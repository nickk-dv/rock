use super::scope::*;
use crate::ast::ast::*;
use crate::ast::span::Span;
use crate::ast::visit;
use crate::err::error::*;
use crate::mem::*;
use std::collections::HashMap;

//@conflits in scope can be wrong if symbol / glob reference the same symbol
//@warn or hard error on access that points to self, how would that behave with imports (import from self error), can it be generalized?
//@report usages of self id module path as redundant?

pub fn check(ast: P<Ast>) -> Result<(), ()> {
    let est_scope_count = ast.modules.len();
    let block_size = std::mem::size_of::<Scope>() * est_scope_count;
    let mut arena = Arena::new(block_size);
    let mut context = arena.alloc::<Context>();
    *context = Context::new(ast, arena);

    context.pass_0_create_scopes()?;
    context.pass_1_check_main_proc();
    context.pass_2_check_namesets();
    context.pass_3_process_imports();
    context.pass_4_type_resolve();
    context.pass_5_check_globals();
    context.pass_6_check_control_flow();

    let mut ir_gen = IRGen::new(context.copy());
    ir_gen.emit_ir();

    //let result = context.report_errors();
    context.manual_drop();
    return Err(());
}

pub struct Context {
    ast: P<Ast>,
    arena: Arena,
    errors: Drop<Vec<Error>>,
    scopes: Drop<Vec<P<Scope>>>,
    curr_scope: P<Scope>, //@hack for visitor
    procs: Drop<Vec<(P<Module>, ProcData)>>,
    enums: Drop<Vec<EnumData>>,
    structs: Drop<Vec<StructData>>,
    globals: Drop<Vec<GlobalData>>,
}

impl ManualDrop for P<Context> {
    fn manual_drop(mut self) {
        unsafe {
            for scope in self.scopes.iter() {
                scope.copy().manual_drop();
            }
            Drop::drop(&mut self.errors);
            Drop::drop(&mut self.scopes);
            Drop::drop(&mut self.procs);
            Drop::drop(&mut self.enums);
            Drop::drop(&mut self.structs);
            Drop::drop(&mut self.globals);
            self.arena.manual_drop();
        }
    }
}

impl Context {
    fn new(ast: P<Ast>, arena: Arena) -> Self {
        Self {
            ast,
            arena,
            errors: Drop::new(Vec::new()),
            scopes: Drop::new(Vec::new()),
            curr_scope: P::null(),
            procs: Drop::new(Vec::new()),
            enums: Drop::new(Vec::new()),
            structs: Drop::new(Vec::new()),
            globals: Drop::new(Vec::new()),
        }
    }
}

pub const ROOT_ID: ScopeID = 0;

macro_rules! scope_error {
    ($scope:expr, $error:expr) => {
        let mut scope_copy = $scope.copy();
        scope_copy.errors.push($error.into());
    };
}

fn visibility_private(vis: Visibility, scope_id: ScopeID) -> bool {
    vis == Visibility::Private && scope_id != ROOT_ID
}

fn visibility_public(vis: Visibility, scope_id: ScopeID) -> bool {
    vis == Visibility::Public || scope_id == ROOT_ID
}

struct ScopeTreeTask {
    parent: P<Module>,
    parent_id: ScopeID,
    mod_decl: P<ModDecl>,
}

struct ImportTask {
    import: P<ImportDecl>,
    status: ImportTaskStatus,
}

#[derive(PartialEq)]
enum ImportTaskStatus {
    Unresolved,
    SourceNotFound,
    Resolved,
}

struct Conflit<T> {
    data: T,
    from_scope: P<Scope>,
    import_span: Option<Span>,
}

impl<T> Conflit<T> {
    fn new(data: T, from_scope: P<Scope>, import_span: Option<Span>) -> Self {
        Self {
            data,
            from_scope,
            import_span,
        }
    }
}

impl visit::MutVisit for Context {
    fn visit_custom_type(&mut self, custom_type: P<CustomType>) {
        let tt = self.scope_find_type(
            self.curr_scope.copy(),
            custom_type.module_path,
            custom_type.name,
        );
    }

    fn visit_struct_init(&mut self, struct_init: P<StructInit>) {
        if let Some(name) = struct_init.name {
            let tt = self.scope_find_type(self.curr_scope.copy(), struct_init.module_path, name);
        }
    }

    fn visit_proc_call(&mut self, proc_call: P<ProcCall>) {
        self.scope_find_proc(
            self.curr_scope.copy(),
            proc_call.module_path,
            proc_call.name,
        );
    }
}

impl Context {
    fn error(&mut self, error: Error) {
        self.errors.push(error);
    }

    fn report_errors(&self) -> Result<(), ()> {
        use crate::err::report;
        for err in self.errors.iter() {
            report::report(err);
        }
        for scope in self.scopes.iter() {
            for err in scope.errors.iter() {
                report::report(err);
            }
        }
        report::err_status(())
    }

    pub fn get_scope(&self, id: ScopeID) -> P<Scope> {
        self.scopes.get(id as usize).unwrap().copy()
    }

    fn pass_0_create_scopes(&mut self) -> Result<(), ()> {
        let mut tasks = Vec::new();
        let mut file_module_map = HashMap::new();
        let mut file_scope_map = HashMap::new();

        for module in self.ast.modules.iter() {
            file_module_map.insert(module.file.path.clone(), module.copy());
        }

        let mut root_path = std::path::PathBuf::new();
        root_path.push("test"); //@src
        root_path.push("main.lang"); //@lib.lang, without main req.

        match file_module_map.get(&root_path) {
            Some(module) => {
                let scope = self.create_scope(
                    self.scopes.len() as ScopeID,
                    module.copy(),
                    None,
                    &mut tasks,
                );
                file_scope_map.insert(scope.module.file.path.clone(), scope.id);
                self.scopes.push(scope);
            }
            None => {
                self.error(Error::check_no_src(CheckError::ParseMainFileMissing));
                return self.report_errors();
            }
        }

        while let Some(mut task) = tasks.pop() {
            let source = &task.parent.file.source;
            let mod_name = task.mod_decl.name.span.str(source);
            let mut path_1 = task.parent.file.path.clone();
            let mut path_2 = task.parent.file.path.clone();
            path_1.pop();
            path_1.push(format!("{}.lang", mod_name));
            path_2.pop();
            path_2.push(mod_name);
            path_2.push("mod.lang");

            let module = match (file_module_map.get(&path_1), file_module_map.get(&path_2)) {
                (None, None) => {
                    self.error(
                        Error::check(
                            CheckError::ParseModBothPathsMissing,
                            task.parent,
                            task.mod_decl.name.span,
                        )
                        .info(format!("{:?}", path_1))
                        .info(format!("{:?}", path_2))
                        .into(),
                    );
                    continue;
                }
                (Some(..), Some(..)) => {
                    self.error(
                        Error::check(
                            CheckError::ParseModBothPathsExist,
                            task.parent,
                            task.mod_decl.name.span,
                        )
                        .info(format!("{:?}", path_1))
                        .info(format!("{:?}", path_2))
                        .into(),
                    );
                    continue;
                }
                (Some(module), None) => match file_scope_map.get(&path_1) {
                    Some(..) => {
                        //@store where this path was declared? mod_decl + scope_id
                        //@change err message
                        self.error(
                            Error::check(
                                CheckError::ParseModCycle,
                                task.parent,
                                task.mod_decl.name.span,
                            )
                            .info(format!("{:?}", path_1))
                            .into(),
                        );
                        continue;
                    }
                    None => module.copy(),
                },
                (None, Some(module)) => match file_scope_map.get(&path_2) {
                    Some(..) => {
                        //@store where this path was declared? mod_decl + scope_id
                        //@change err message
                        self.error(
                            Error::check(
                                CheckError::ParseModCycle,
                                task.parent,
                                task.mod_decl.name.span,
                            )
                            .info(format!("{:?}", path_2))
                            .into(),
                        );
                        continue;
                    }
                    None => module.copy(),
                },
            };

            let scope = self.create_scope(
                self.scopes.len() as ScopeID,
                module,
                Some(task.parent_id),
                &mut tasks,
            );
            task.mod_decl.id = Some(scope.id);
            file_scope_map.insert(scope.module.file.path.clone(), scope.id);
            self.scopes.push(scope);
        }

        Ok(())
    }

    fn create_scope(
        &mut self,
        id: ScopeID,
        module: P<Module>,
        parent: Option<ScopeID>,
        tasks: &mut Vec<ScopeTreeTask>,
    ) -> P<Scope> {
        let mut scope = self.arena.alloc::<Scope>();
        *scope = Scope::new(id, module.copy(), parent);

        macro_rules! redefinition_error {
            ($check_error:expr, $span:expr, $existing_span:expr) => {
                scope_error!(
                    scope,
                    Error::check($check_error, scope.md(), $span).context(
                        "already defined here",
                        scope.md(),
                        $existing_span
                    )
                );
            };
        }

        for decl in scope.module.decls {
            match decl {
                Decl::Mod(mod_decl) => {
                    if let Err(existing) = scope.add_mod(mod_decl) {
                        redefinition_error!(
                            CheckError::RedefinitionMod,
                            mod_decl.name.span,
                            existing.name.span
                        );
                    } else {
                        tasks.push(ScopeTreeTask {
                            parent: module.copy(),
                            parent_id: scope.id,
                            mod_decl,
                        });
                    }
                }
                Decl::Proc(decl) => {
                    let id = self.procs.len() as ProcID;
                    if let Err(existing) = scope.add_proc(decl, 0) {
                        redefinition_error!(
                            CheckError::RedefinitionProc,
                            decl.name.span,
                            existing.decl.name.span
                        );
                    } else {
                        self.procs.push((scope.md(), ProcData { decl, id }));
                    }
                }
                Decl::Enum(decl) => {
                    let id = self.enums.len() as EnumID;
                    if let Err(existing) = scope.add_enum(decl, id) {
                        redefinition_error!(
                            CheckError::RedefinitionProc,
                            decl.name.span,
                            existing.name().span
                        );
                    } else {
                        self.enums.push(EnumData { decl, id });
                    }
                }
                Decl::Struct(decl) => {
                    let id = self.structs.len() as StructID;
                    if let Err(existing) = scope.add_struct(decl, 0) {
                        redefinition_error!(
                            CheckError::RedefinitionType,
                            decl.name.span,
                            existing.name().span
                        );
                    } else {
                        self.structs.push(StructData { decl, id });
                    }
                }
                Decl::Global(decl) => {
                    let id = self.globals.len() as GlobalID;
                    if let Err(existing) = scope.add_global(decl, 0) {
                        redefinition_error!(
                            CheckError::RedefinitionGlobal,
                            decl.name.span,
                            existing.decl.name.span
                        );
                    } else {
                        self.globals.push(GlobalData { decl, id });
                    }
                }
                Decl::Import(..) => {}
                Decl::Impl(..) => {}
            }
        }

        return scope;
    }

    fn pass_1_check_main_proc(&mut self) {
        let main_id = match self.ast.intern_pool.get_id_if_exists("main".as_bytes()) {
            Some(id) => id,
            None => {
                self.error(Error::check_no_src(CheckError::MainProcMissing));
                return;
            }
        };
        let mut scope = self.get_scope(ROOT_ID);
        let main = match scope.get_proc(main_id) {
            Some(data) => data.decl,
            None => {
                self.error(Error::check_no_src(CheckError::MainProcMissing));
                return;
            }
        };
        if main.is_variadic {
            scope.error(CheckError::MainProcVariadic, main.name.span);
        }
        if main.block.is_none() {
            scope.error(CheckError::MainProcExternal, main.name.span);
        }
        if !main.params.is_empty() {
            scope.error(CheckError::MainProcHasParams, main.name.span);
        }
        if let Some(tt) = main.return_type {
            if tt.pointer_level == 0 && matches!(tt.kind, TypeKind::Basic(BasicType::S32)) {
                return;
            }
        }
        scope.error(CheckError::MainProcWrongRetType, main.name.span);
    }

    //@duplicates are not removed
    fn pass_2_check_namesets(&self) {
        for scope in self.scopes.iter() {
            let scope = scope.copy();
            let mut name_set = HashMap::<InternID, Span>::new();

            macro_rules! redefinition_check {
                ($element_list:expr, $check_error:expr) => {
                    if $element_list.is_empty() {
                        continue;
                    }
                    name_set.clear();
                    for element in $element_list.iter() {
                        if let Some(existing) = name_set.get(&element.name.id) {
                            scope_error!(
                                scope,
                                Error::check($check_error, scope.md(), element.name.span).context(
                                    "already defined here",
                                    scope.md(),
                                    *existing
                                )
                            );
                        } else {
                            name_set.insert(element.name.id, element.name.span);
                        }
                    }
                };
            }

            for decl in scope.module.decls {
                match decl {
                    Decl::Proc(proc_decl) => {
                        redefinition_check!(proc_decl.params, CheckError::ProcParamRedefinition);
                    }
                    Decl::Enum(enum_decl) => {
                        redefinition_check!(
                            enum_decl.variants,
                            CheckError::EnumVariantRedefinition
                        );
                    }
                    Decl::Struct(struct_decl) => {
                        redefinition_check!(
                            struct_decl.fields,
                            CheckError::StructFieldRedefinition
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    fn pass_3_process_imports(&self) {
        for scope in self.scopes.iter() {
            let mut import_tasks = Vec::new();
            let mut were_resolved = 0;

            for decl in scope.module.decls {
                if let Decl::Import(import) = decl {
                    import_tasks.push(ImportTask {
                        import,
                        status: ImportTaskStatus::Unresolved,
                    });
                }
            }

            while import_tasks
                .iter()
                .any(|task| task.status != ImportTaskStatus::Resolved)
            {
                for task in import_tasks.iter_mut() {
                    self.import_task_run(scope.copy(), task);
                }

                let resolved_count = import_tasks
                    .iter()
                    .filter(|task| task.status == ImportTaskStatus::Resolved)
                    .count();
                if resolved_count <= were_resolved {
                    for task in import_tasks.iter_mut() {
                        if task.status == ImportTaskStatus::SourceNotFound {
                            scope_error!(
                                scope,
                                Error::check(
                                    CheckError::ModuleNotFoundInScope,
                                    scope.md(),
                                    task.import.module_path.names.first().unwrap().span,
                                )
                            );
                        }
                    }
                    break;
                } else {
                    were_resolved = resolved_count;
                }
            }
        }
    }

    fn import_task_run(&self, mut scope: P<Scope>, task: &mut ImportTask) {
        if task.status == ImportTaskStatus::Resolved {
            return;
        }

        if task.import.module_path.kind == ModulePathKind::None {
            let first = task.import.module_path.names.first().unwrap();
            if !self.scope_in_scope_mod_exists(scope.copy(), first) {
                task.status = ImportTaskStatus::SourceNotFound;
                return;
            }
        }
        task.status = ImportTaskStatus::Resolved;

        let from_scope = match self.scope_resolve_module_path(scope.copy(), task.import.module_path)
        {
            Some(from_scope) => from_scope,
            None => return,
        };

        if from_scope.id == scope.id {
            scope.error(CheckError::ImportFromItself, task.import.span);
            return;
        }

        match task.import.target {
            ImportTarget::AllSymbols => {
                let import = GlobImport {
                    from_id: from_scope.id,
                    import_span: task.import.span,
                };
                if let Err(existing) = scope.add_glob_import(import) {
                    scope_error!(
                        scope,
                        Error::check(CheckError::ImportGlobExists, scope.md(), task.import.span,)
                            .context("existing import", scope.md(), existing.import_span)
                    );
                }
            }
            ImportTarget::Symbol(name) => {
                self.scope_import_symbol(scope, from_scope, name);
            }
            ImportTarget::SymbolList(symbol_list) => {
                for name in symbol_list {
                    self.scope_import_symbol(scope.copy(), from_scope.copy(), name);
                }
            }
        }
    }

    fn scope_in_scope_mod_exists(&self, scope: P<Scope>, name: Ident) -> bool {
        if scope.get_mod(name.id).is_some() {
            return true;
        }
        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if from_scope.get_mod(name.id).is_some() {
                return true;
            }
        }
        for import in scope.glob_imports.iter() {
            let from_scope = self.get_scope(import.from_id);
            if from_scope.get_mod(name.id).is_some() {
                return true;
            }
        }
        return false;
    }

    fn scope_import_symbol(&self, mut scope: P<Scope>, from_scope: P<Scope>, name: Ident) {
        let mut some_exists = false;
        let mut private_symbols = Vec::new();

        if let Some(mod_decl) = from_scope.get_mod(name.id) {
            if visibility_private(mod_decl.vis, from_scope.id) {
                private_symbols.push(mod_decl.name);
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.get_proc(name.id) {
            if visibility_private(data.decl.vis, from_scope.id) {
                private_symbols.push(data.decl.name);
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.get_type(name.id) {
            if visibility_private(data.vis(), from_scope.id) {
                private_symbols.push(data.name());
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.get_global(name.id) {
            if visibility_private(data.decl.vis, from_scope.id) {
                private_symbols.push(data.decl.name);
            } else {
                some_exists = true;
            }
        }

        if !some_exists {
            let mut error = Error::check(CheckError::ImportSymbolNotDefined, scope.md(), name.span);
            for name in private_symbols {
                error = error.context("found this private symbol", from_scope.md(), name.span);
            }
            scope_error!(scope, error);
            return;
        }

        let import = SymbolImport {
            from_id: from_scope.id,
            name,
        };
        if let Err(existing) = scope.add_symbol_import(import) {
            scope_error!(
                scope,
                Error::check(
                    CheckError::ImportSymbolAlreadyImported,
                    scope.md(),
                    name.span,
                )
                .context("existing symbol import", scope.md(), existing.name.span)
            );
        }
    }

    fn pass_4_type_resolve(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            self.curr_scope = self.get_scope(scope_id);
            let module = self.curr_scope.module.copy();
            visit::visit_module_with(self, module);
        }
    }

    fn scope_find_proc(
        &mut self,
        mut scope: P<Scope>,
        module_path: ModulePath,
        name: Ident,
    ) -> Option<ProcData> {
        if module_path.kind == ModulePathKind::None && module_path.names.is_empty() {
            return self.scope_get_in_scope_proc(scope, name);
        }
        let from_scope = match self.scope_resolve_module_path(scope.copy(), module_path) {
            Some(from_scope) => from_scope,
            None => return None,
        };
        let data = match from_scope.get_proc(name.id) {
            Some(data) => data,
            None => {
                scope.error(CheckError::ProcNotDeclaredInPath, name.span);
                return None;
            }
        };
        if visibility_private(data.decl.vis, from_scope.id) {
            scope_error!(
                scope,
                Error::check(CheckError::ProcIsPrivate, scope.md(), name.span).context(
                    "defined here",
                    from_scope.md(),
                    data.decl.name.span
                )
            );
            return None;
        }
        return Some(data);
    }

    fn scope_find_type(
        &mut self,
        mut scope: P<Scope>,
        module_path: ModulePath,
        name: Ident,
    ) -> Option<TypeData> {
        if module_path.kind == ModulePathKind::None && module_path.names.is_empty() {
            return self.scope_get_in_scope_type(scope, name);
        }
        let from_scope = match self.scope_resolve_module_path(scope.copy(), module_path) {
            Some(from_scope) => from_scope,
            None => return None,
        };
        let data = match from_scope.get_type(name.id) {
            Some(data) => data,
            None => {
                scope.error(CheckError::TypeNotDeclaredInPath, name.span);
                return None;
            }
        };
        if visibility_private(data.vis(), from_scope.id) {
            scope_error!(
                scope,
                Error::check(CheckError::TypeIsPrivate, scope.md(), name.span).context(
                    "defined here",
                    from_scope.md(),
                    data.name().span
                )
            );
            return None;
        }
        return Some(data);
    }

    fn scope_find_global(
        &mut self,
        mut scope: P<Scope>,
        module_path: ModulePath,
        name: Ident,
    ) -> Option<GlobalData> {
        if module_path.kind == ModulePathKind::None && module_path.names.is_empty() {
            return self.scope_get_in_scope_global(scope, name);
        }
        let from_scope = match self.scope_resolve_module_path(scope.copy(), module_path) {
            Some(from_scope) => from_scope,
            None => return None,
        };
        let data = match from_scope.get_global(name.id) {
            Some(data) => data,
            None => {
                scope.error(CheckError::GlobalNotDeclaredInPath, name.span);
                return None;
            }
        };
        if visibility_private(data.decl.vis, from_scope.id) {
            scope_error!(
                scope,
                Error::check(CheckError::GlobalIsPrivate, scope.md(), name.span).context(
                    "defined here",
                    from_scope.md(),
                    data.decl.name.span
                )
            );
            return None;
        }
        return Some(data);
    }

    fn scope_resolve_module_path(
        &self,
        mut scope: P<Scope>,
        module_path: ModulePath,
    ) -> Option<P<Scope>> {
        let mut target = match module_path.kind {
            ModulePathKind::None => {
                let first = match module_path.names.first() {
                    Some(name) => name,
                    None => return Some(scope),
                };
                let mod_decl = match self.scope_get_in_scope_mod(scope.copy(), first) {
                    Some(mod_decl) => mod_decl,
                    None => return None,
                };
                match mod_decl.id {
                    Some(id) => self.get_scope(id),
                    None => {
                        scope.error(CheckError::ModuleFileReportedMissing, first.span);
                        return None;
                    }
                }
            }
            ModulePathKind::Super => {
                if let Some(parent) = scope.parent {
                    self.get_scope(parent)
                } else {
                    scope.error(CheckError::SuperUsedFromRootModule, module_path.kind_span);
                    return None;
                }
            }
            ModulePathKind::Package => self.get_scope(ROOT_ID),
        };

        let mut skip_first = module_path.kind == ModulePathKind::None;
        for name in module_path.names {
            if skip_first {
                skip_first = false;
                continue;
            }
            let mod_decl = match target.get_mod(name.id) {
                Some(mod_decl) => mod_decl,
                None => {
                    scope.error(CheckError::ModuleNotDeclaredInPath, name.span);
                    return None;
                }
            };
            if visibility_private(mod_decl.vis, target.id) {
                scope_error!(
                    scope,
                    Error::check(CheckError::ModuleIsPrivate, scope.md(), name.span).context(
                        "defined here",
                        target.md(),
                        mod_decl.name.span
                    )
                );
                return None;
            }
            target = match mod_decl.id {
                Some(id) => self.get_scope(id),
                None => {
                    scope.error(CheckError::ModuleFileReportedMissing, name.span);
                    return None;
                }
            };
        }

        return Some(target);
    }

    fn scope_get_in_scope_mod(&self, mut scope: P<Scope>, name: Ident) -> Option<P<ModDecl>> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<P<ModDecl>>>::new();

        if let Some(data) = scope.get_mod(name.id) {
            unique = Some(Conflit::new(data, scope.copy(), None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(mod_decl) = from_scope.get_mod(name.id) {
                if visibility_public(mod_decl.vis, import.from_id) {
                    let conflict = Conflit::new(mod_decl, from_scope, Some(import.name.span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        for import in scope.glob_imports.iter() {
            let from_scope = self.get_scope(import.from_id);
            if let Some(mod_decl) = from_scope.get_mod(name.id) {
                if visibility_public(mod_decl.vis, import.from_id) {
                    let conflict = Conflit::new(mod_decl, from_scope, Some(import.import_span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        if conflicts.is_empty() {
            return match unique {
                Some(conflict) => Some(conflict.data),
                None => {
                    scope.error(CheckError::ModuleNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(CheckError::ModuleSymbolConflict, scope.md(), name.span);
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md(), import_span)
                    .context(
                        "conflict with this module",
                        conflict.from_scope.md(),
                        conflict.data.name.span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared module",
                    scope.md(),
                    conflict.data.name.span,
                );
            }
        }
        scope_error!(scope, error);
        return None;
    }

    fn scope_get_in_scope_proc(&mut self, mut scope: P<Scope>, name: Ident) -> Option<ProcData> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<ProcData>>::new();

        if let Some(data) = scope.get_proc(name.id) {
            unique = Some(Conflit::new(data, scope.copy(), None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_proc(name.id) {
                if visibility_public(data.decl.vis, import.from_id) {
                    let conflict = Conflit::new(data, from_scope, Some(import.name.span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        for import in scope.glob_imports.iter() {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_proc(name.id) {
                if visibility_public(data.decl.vis, import.from_id) {
                    let conflict = Conflit::new(data, from_scope, Some(import.import_span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        if conflicts.is_empty() {
            return match unique {
                Some(conflict) => Some(conflict.data),
                None => {
                    scope.error(CheckError::ProcNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(CheckError::ProcSymbolConflict, scope.md(), name.span);
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md(), import_span)
                    .context(
                        "conflict with this procedure",
                        conflict.from_scope.md(),
                        conflict.data.decl.name.span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared procedure",
                    scope.md(),
                    conflict.data.decl.name.span,
                );
            }
        }
        scope.errors.push(error.into());
        return None;
    }

    fn scope_get_in_scope_type(&mut self, mut scope: P<Scope>, name: Ident) -> Option<TypeData> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<TypeData>>::new();

        if let Some(data) = scope.get_type(name.id) {
            unique = Some(Conflit::new(data, scope.copy(), None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_type(name.id) {
                if visibility_public(data.vis(), import.from_id) {
                    let conflict = Conflit::new(data, from_scope, Some(import.name.span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        for import in scope.glob_imports.iter() {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_type(name.id) {
                if visibility_public(data.vis(), import.from_id) {
                    let conflict = Conflit::new(data, from_scope, Some(import.import_span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        if conflicts.is_empty() {
            return match unique {
                Some(conflict) => Some(conflict.data),
                None => {
                    scope.error(CheckError::TypeNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(CheckError::TypeSymbolConflict, scope.md(), name.span);
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md(), import_span)
                    .context(
                        "conflict with this type name",
                        conflict.from_scope.md(),
                        conflict.data.name().span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared type name",
                    scope.md(),
                    conflict.data.name().span,
                );
            }
        }
        scope.errors.push(error.into());
        return None;
    }

    fn scope_get_in_scope_global(
        &mut self,
        mut scope: P<Scope>,
        name: Ident,
    ) -> Option<GlobalData> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<GlobalData>>::new();

        if let Some(data) = scope.get_global(name.id) {
            unique = Some(Conflit::new(data, scope.copy(), None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_global(name.id) {
                if visibility_public(data.decl.vis, import.from_id) {
                    let conflict = Conflit::new(data, from_scope, Some(import.name.span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        for import in scope.glob_imports.iter() {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_global(name.id) {
                if visibility_public(data.decl.vis, import.from_id) {
                    let conflict = Conflit::new(data, from_scope, Some(import.import_span));
                    if unique.is_none() {
                        unique = Some(conflict);
                    } else {
                        conflicts.push(conflict);
                    }
                }
            }
        }

        if conflicts.is_empty() {
            return match unique {
                Some(conflict) => Some(conflict.data),
                None => {
                    scope.error(CheckError::GlobalNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(CheckError::GlobalSymbolConflict, scope.md(), name.span);
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md(), import_span)
                    .context(
                        "conflict with this global constant",
                        conflict.from_scope.md(),
                        conflict.data.decl.name.span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared global constant",
                    scope.md(),
                    conflict.data.decl.name.span,
                );
            }
        }
        scope.errors.push(error.into());
        return None;
    }

    fn pass_5_check_globals(&mut self) {
        //@duplicate decls are still checked here
        for scope_id in 0..self.scopes.len() as ScopeID {
            let scope = self.get_scope(scope_id);
            for decl in scope.module.decls {
                if let Decl::Global(global) = decl {
                    self.scope_check_global_expr(scope.copy(), global.expr.0);
                }
            }
        }
    }

    fn scope_check_global_expr(&mut self, scope: P<Scope>, expr: Expr) {
        match expr.kind {
            ExprKind::Var(var) => {
                //access not walked
                let global = self.scope_find_global(scope, var.module_path, var.name);
            }
            ExprKind::Enum(..) => {}   //todo
            ExprKind::Cast(..) => {}   //todo
            ExprKind::Sizeof(..) => {} //todo
            ExprKind::Literal(..) => {}
            ExprKind::ProcCall(proc_call) => {
                for expr in proc_call.input {
                    self.scope_check_global_expr(scope.copy(), expr);
                }
            }
            ExprKind::ArrayInit(array_init) => {
                for expr in array_init.input {
                    self.scope_check_global_expr(scope.copy(), expr);
                }
            }
            ExprKind::StructInit(struct_init) => {
                for expr in struct_init.input {
                    self.scope_check_global_expr(scope.copy(), expr);
                }
            }
            ExprKind::UnaryExpr(un) => self.scope_check_global_expr(scope.copy(), un.rhs),
            ExprKind::BinaryExpr(bin) => {
                self.scope_check_global_expr(scope.copy(), bin.lhs);
                self.scope_check_global_expr(scope, bin.rhs);
            }
        }
    }

    fn pass_6_check_control_flow(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            let scope = self.get_scope(scope_id);
            for decl in scope.module.decls {
                if let Decl::Proc(proc_decl) = decl {
                    if let Some(block) = proc_decl.block {
                        self.scope_check_control_flow(scope.copy(), block, false, false, false);
                    }
                }
            }
        }
    }

    fn scope_check_control_flow(
        &mut self,
        mut scope: P<Scope>,
        block: P<Block>,
        in_loop: bool,
        in_defer: bool,
        mut is_unreachable: bool,
    ) {
        //@todo very bloated code
        //@todo ban continue / break in defer blocks for outside loops
        //@todo ban return in defer blocks
        let mut terminated = false;
        let mut term_span: Span = Span::new(0, 0);
        let mut term_errored = is_unreachable;
        for stmt in block.stmts {
            if terminated && !term_errored {
                term_errored = true;
                scope_error!(
                    scope,
                    Error::check(CheckError::UnreachableStatement, scope.md(), stmt.span).context(
                        "any code following this statement is unreachable",
                        scope.md(),
                        term_span
                    )
                );
            }
            match stmt.kind {
                StmtKind::If(if_) => {
                    self.scope_check_control_flow(
                        scope.copy(),
                        if_.block,
                        in_loop,
                        in_defer,
                        is_unreachable,
                    );
                    let mut curr_else = if_.else_;
                    while let Some(else_) = curr_else {
                        match else_ {
                            Else::If(if_) => {
                                self.scope_check_control_flow(
                                    scope.copy(),
                                    if_.block,
                                    in_loop,
                                    in_defer,
                                    is_unreachable,
                                );
                                curr_else = if_.else_;
                            }
                            Else::Block(block) => {
                                self.scope_check_control_flow(
                                    scope.copy(),
                                    block,
                                    in_loop,
                                    in_defer,
                                    is_unreachable,
                                );
                                curr_else = None;
                            }
                        }
                    }
                }
                StmtKind::For(for_) => {
                    self.scope_check_control_flow(
                        scope.copy(),
                        for_.block,
                        true,
                        in_defer,
                        is_unreachable,
                    );
                }
                StmtKind::Block(block) => {
                    self.scope_check_control_flow(
                        scope.copy(),
                        block,
                        in_loop,
                        in_defer,
                        is_unreachable,
                    );
                }
                StmtKind::Defer(defer) => {
                    if in_defer {
                        scope.error(CheckError::DeferNested, stmt.span);
                    }
                    self.scope_check_control_flow(
                        scope.copy(),
                        defer,
                        in_loop,
                        true,
                        is_unreachable,
                    );
                }
                StmtKind::Break => {
                    if !in_loop {
                        scope.error(CheckError::BreakOutsideLoop, stmt.span);
                    } else {
                        terminated = true;
                        term_span = stmt.span;
                        is_unreachable = true;
                    }
                }
                StmtKind::Switch(..) => {}
                StmtKind::Return(..) => {
                    terminated = true;
                    term_span = stmt.span;
                    is_unreachable = true;
                }
                StmtKind::Continue => {
                    if !in_loop {
                        scope.error(CheckError::ContinueOutsideLoop, stmt.span);
                    } else {
                        terminated = true;
                        term_span = stmt.span;
                        is_unreachable = true;
                    }
                }
                StmtKind::VarDecl(..) => {}
                StmtKind::VarAssign(..) => {}
                StmtKind::ProcCall(..) => {}
            }
        }
    }
}

use super::ir::*;

struct IRGen {
    context: P<Context>,
    ir_buf: Vec<Inst>,
    val_count: u32,
}

impl IRGen {
    fn new(context: P<Context>) -> Self {
        Self {
            context,
            ir_buf: Vec::new(),
            val_count: 0,
        }
    }

    fn add_inst(&mut self, inst: Inst) {
        self.ir_buf.push(inst);
    }

    fn emit_val(&mut self) -> u32 {
        let id = self.val_count;
        self.val_count += 1;
        self.add_inst(Inst::Value(id));
        return id;
    }

    fn emit_ir(&mut self) {
        for v in self.context.copy().globals.iter() {
            self.emit_global_decl(v.decl);
        }
        pretty_print(&self.ir_buf);
    }

    fn emit_global_decl(&mut self, global_decl: P<GlobalDecl>) {
        self.emit_expr(global_decl.expr.0);
    }

    fn emit_proc_decl(&mut self, proc_decl: P<ProcDecl>) {}

    fn emit_expr(&mut self, expr: Expr) -> u32 {
        match expr.kind {
            ExprKind::Var(_) => todo!(),
            ExprKind::Enum(_) => todo!(),
            ExprKind::Cast(_) => todo!(),
            ExprKind::Sizeof(_) => todo!(),
            ExprKind::Literal(lit) => self.emit_literal(lit),
            ExprKind::ProcCall(proc_call) => self.emit_proc_call(proc_call),
            ExprKind::ArrayInit(array_init) => self.emit_array_init(array_init),
            ExprKind::StructInit(struct_init) => self.emit_struct_init(struct_init),
            ExprKind::UnaryExpr(un) => self.emit_un_expr(un),
            ExprKind::BinaryExpr(bin) => self.emit_bin_expr(bin),
        }
    }

    fn emit_literal(&mut self, lit: Literal) -> u32 {
        let val = self.emit_val();
        let inst = match lit {
            Literal::Null => Inst::Null,
            Literal::Bool(v) => Inst::Bool(v),
            Literal::Uint(v, t) => Inst::UInt(v, t),
            Literal::Float(v, t) => Inst::Float(v, t),
            Literal::Char(v) => Inst::Char(v),
            Literal::String => todo!("string lit inst not implemented"),
        };
        self.add_inst(inst);
        return val;
    }

    fn emit_proc_call(&mut self, proc_call: P<ProcCall>) -> u32 {
        let mut input = Vec::new();
        for expr in proc_call.input {
            input.push(self.emit_expr(expr));
        }

        let val = self.emit_val();
        self.add_inst(Inst::Call {
            argc: input.len() as u32,
            id: None, //@id is not stored in ast
        });
        for val in input {
            self.add_inst(Inst::Value(val))
        }
        return val;
    }

    fn emit_array_init(&mut self, array_init: P<ArrayInit>) -> u32 {
        let mut input = Vec::new();
        for expr in array_init.input {
            input.push(self.emit_expr(expr));
        }

        let val = self.emit_val();
        self.add_inst(Inst::ArrayInit {
            argc: input.len() as u32,
        });
        for val in input {
            self.add_inst(Inst::Value(val))
        }
        return val;
    }

    fn emit_struct_init(&mut self, struct_init: P<StructInit>) -> u32 {
        let mut input = Vec::new();
        for expr in struct_init.input {
            input.push(self.emit_expr(expr));
        }

        let val = self.emit_val();
        self.add_inst(Inst::StructInit {
            argc: input.len() as u32,
            id: None, //@id is not stored in ast
        });
        for val in input {
            self.add_inst(Inst::Value(val))
        }
        return val;
    }

    fn emit_un_expr(&mut self, un: P<UnaryExpr>) -> u32 {
        let val_rhs = self.emit_expr(un.rhs);
        let val = self.emit_val();
        let inst = match un.op {
            UnaryOp::Minus => Inst::Neg,
            UnaryOp::BitNot => Inst::BitNot,
            UnaryOp::LogicNot => Inst::LogicNot,
            UnaryOp::AddressOf => Inst::Addr,
            UnaryOp::Dereference => Inst::Deref,
        };
        self.add_inst(inst);
        self.add_inst(Inst::Value(val_rhs));
        return val;
    }

    fn emit_bin_expr(&mut self, bin: P<BinaryExpr>) -> u32 {
        let val_lhs = self.emit_expr(bin.lhs);
        let val_rhs = self.emit_expr(bin.rhs);
        let val = self.emit_val();
        let inst = match bin.op {
            BinaryOp::LogicAnd => Inst::LogicAnd,
            BinaryOp::LogicOr => Inst::LogicOr,
            BinaryOp::Less => Inst::CmpLT,
            BinaryOp::Greater => Inst::CmpGT,
            BinaryOp::LessEq => Inst::CmpLEQ,
            BinaryOp::GreaterEq => Inst::CmpGEQ,
            BinaryOp::IsEq => Inst::CmpEQ,
            BinaryOp::NotEq => Inst::CmpNEQ,
            BinaryOp::Plus => Inst::Add,
            BinaryOp::Minus => Inst::Sub,
            BinaryOp::Times => Inst::Mul,
            BinaryOp::Div => Inst::Div,
            BinaryOp::Mod => Inst::Rem,
            BinaryOp::BitAnd => Inst::BitAnd,
            BinaryOp::BitOr => Inst::BitOr,
            BinaryOp::BitXor => Inst::BitXor,
            BinaryOp::Shl => Inst::Shl,
            BinaryOp::Shr => Inst::Shr,
        };
        self.add_inst(inst);
        self.add_inst(Inst::Value(val_lhs));
        self.add_inst(Inst::Value(val_rhs));
        return val;
    }
}
