use super::scope::*;
use crate::ast::ast::*;
use crate::ast::intern::*;
use crate::ast::parser::CompCtx;
use crate::ast::span::Span;
use crate::ast::visit;
use crate::err::error::*;
use crate::mem::*;
use std::collections::HashMap;

//@conflits in scope can be wrong if symbol / glob reference the same symbol
//@warn or hard error on access that points to self, how would that behave with imports (import from self error), can it be generalized?
//@report usages of self id module path as redundant?

pub fn check(mut ast: Ast, ctx: &CompCtx) -> Result<(), ()> {
    // 14192 original ast size
    // 14632 // 440 / 8 = 55 extra indir after moving Expr -> P<Expr>

    //@memory prof
    eprintln!("ast arenas mem-usage: {}", ast.arena.memory_usage());

    let mut context = Context::new(&mut ast, ctx);

    context.pass_0_create_scopes()?;
    context.pass_1_check_main_proc();
    context.pass_2_check_namesets();
    context.pass_3_process_imports();
    context.pass_4_type_resolve();
    context.pass_6_check_control_flow();
    context.pass_7_check_proc();

    return context.report_errors();
}

pub struct Context<'a> {
    ast: &'a mut Ast,
    ctx: &'a CompCtx,
    curr_scope: ScopeID,
    errors: Vec<Error>,
    scopes: Vec<Scope>,
    procs: Vec<(P<Module>, ProcData)>,
    enums: Vec<EnumData>,
    unions: Vec<UnionData>,
    structs: Vec<StructData>,
    globals: Vec<GlobalData>,
}

impl<'a> Context<'a> {
    fn new(ast: &'a mut Ast, ctx: &'a CompCtx) -> Self {
        Self {
            ast,
            ctx,
            curr_scope: 0,
            errors: Vec::new(),
            scopes: Vec::new(),
            procs: Vec::new(),
            enums: Vec::new(),
            unions: Vec::new(),
            structs: Vec::new(),
            globals: Vec::new(),
        }
    }
}

pub const ROOT_ID: ScopeID = 0;

macro_rules! scope_error {
    ($self:expr, $scope_id:expr, $error:expr) => {
        $self.get_scope_mut($scope_id).errors.push($error.into());
    };
}

fn visibility_private(vis: Vis, scope_id: ScopeID) -> bool {
    vis == Vis::Private && scope_id != ROOT_ID
}

fn visibility_public(vis: Vis, scope_id: ScopeID) -> bool {
    vis == Vis::Public || scope_id == ROOT_ID
}

struct ScopeTreeTask {
    parent: P<Module>,
    parent_id: ScopeID,
    mod_decl: P<ModuleDecl>,
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
    from_scope: ScopeID,
    import_span: Option<Span>,
}

impl<T> Conflit<T> {
    fn new(data: T, from_scope: ScopeID, import_span: Option<Span>) -> Self {
        Self {
            data,
            from_scope,
            import_span,
        }
    }
}

impl<'a> visit::MutVisit for Context<'a> {
    fn visit_type(&mut self, ty: &mut Type) {
        let custom_type = if let TypeKind::Custom(custom) = ty.kind {
            custom
        } else {
            return;
        };

        let tt = self.scope_find_type(self.curr_scope, custom_type.path, custom_type.name);
        match tt {
            Some(TypeData::Enum(data)) => {
                ty.kind = TypeKind::Enum(data.id);
            }
            Some(TypeData::Union(data)) => {
                ty.kind = TypeKind::Union(data.id);
            }
            Some(TypeData::Struct(data)) => {
                ty.kind = TypeKind::Struct(data.id);
            }
            None => ty.kind = TypeKind::Poison,
        }
    }

    fn visit_struct_init(&mut self, mut struct_init: P<StructInit>) {
        let tt = self.scope_find_type(self.curr_scope, struct_init.path, struct_init.name);
        match tt {
            Some(TypeData::Enum(..)) => {
                //@no source id available after find_type is complete
                // add context when scopes / finding symbols is changed
                struct_init.ty = StructInitResolved::Poison;
                self.get_scope_mut(self.curr_scope)
                    .error(CheckError::StructInitGotEnumType, struct_init.name.span);
            }
            Some(TypeData::Union(data)) => {
                struct_init.ty = StructInitResolved::Union(data.id);
            }
            Some(TypeData::Struct(data)) => {
                struct_init.ty = StructInitResolved::Struct(data.id);
            }
            None => struct_init.ty = StructInitResolved::Poison,
        }
    }

    fn visit_proc_call(&mut self, mut proc_call: P<ProcCall>) {
        println!("finding proc with intern id = {}", proc_call.name.id.0);
        let data = self.scope_find_proc(self.curr_scope, proc_call.path, proc_call.name);
        if let Some(data) = data {
            proc_call.id = Some(data.id);
            println!("proc_call resolved to proc id: {}", data.id);
        } else {
            proc_call.id = None;
        }
    }
}

impl<'a> Context<'a> {
    fn error(&mut self, error: Error) {
        self.errors.push(error);
    }

    fn report_errors(&self) -> Result<(), ()> {
        let handle = &mut std::io::BufWriter::new(std::io::stderr());
        use crate::err::report;
        for err in self.errors.iter() {
            report::report(handle, err, self.ctx);
        }
        for scope in self.scopes.iter() {
            for err in scope.errors.iter() {
                report::report(handle, err, self.ctx);
            }
        }
        report::err_status(())
    }

    pub fn get_scope(&self, id: ScopeID) -> &Scope {
        self.scopes.get(id as usize).unwrap()
    }

    pub fn get_scope_mut(&mut self, id: ScopeID) -> &mut Scope {
        self.scopes.get_mut(id as usize).unwrap()
    }

    fn pass_0_create_scopes(&mut self) -> Result<(), ()> {
        let mut tasks = Vec::new();
        let mut file_module_map = HashMap::new();
        let mut file_scope_map = HashMap::new();

        for module in self.ast.modules.iter() {
            file_module_map.insert(self.ctx.file(module.file_id).path.clone(), module.copy());
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
                file_scope_map.insert(self.ctx.file(scope.module.file_id).path.clone(), scope.id);
                self.scopes.push(scope);
            }
            None => {
                self.error(Error::check_no_src(CheckError::ParseMainFileMissing));
                return self.report_errors();
            }
        }

        while let Some(mut task) = tasks.pop() {
            let source = &self.ctx.file(task.parent.file_id).source;
            let mod_name = task.mod_decl.name.span.slice(source);
            let mut path_1 = self.ctx.file(task.parent.file_id).path.clone();
            let mut path_2 = self.ctx.file(task.parent.file_id).path.clone();
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
                            task.parent.file_id,
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
                            task.parent.file_id,
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
                                task.parent.file_id,
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
                                task.parent.file_id,
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
            file_scope_map.insert(self.ctx.file(scope.module.file_id).path.clone(), scope.id);
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
    ) -> Scope {
        let mut scope = Scope::new(id, module.copy(), parent);

        macro_rules! redefinition_error {
            ($check_error:expr, $span:expr, $existing_span:expr) => {
                scope.errors.push(
                    Error::check($check_error, scope.md().file_id, $span)
                        .context("already defined here", scope.md().file_id, $existing_span)
                        .into(),
                );
            };
        }

        for decl in scope.module.decls {
            match decl {
                Decl::Module(mod_decl) => {
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
                    if let Err(existing) = scope.add_proc(decl, id) {
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
                            CheckError::RedefinitionType,
                            decl.name.span,
                            existing.name().span
                        );
                    } else {
                        self.enums.push(EnumData { decl, id });
                    }
                }
                Decl::Union(decl) => {
                    let id = self.unions.len() as UnionID;
                    if let Err(existing) = scope.add_union(decl, id) {
                        redefinition_error!(
                            CheckError::RedefinitionType,
                            decl.name.span,
                            existing.name().span
                        );
                    } else {
                        self.unions.push(UnionData { decl, id });
                    }
                }
                Decl::Struct(decl) => {
                    let id = self.structs.len() as StructID;
                    if let Err(existing) = scope.add_struct(decl, id) {
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
                    if let Err(existing) = scope.add_global(decl, id) {
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
            }
        }

        return scope;
    }

    fn pass_1_check_main_proc(&mut self) {
        let main_id = match self.ctx.intern().try_get_str_id("main") {
            Some(id) => id,
            None => {
                self.error(Error::check_no_src(CheckError::MainProcMissing));
                return;
            }
        };
        let mut scope = self.get_scope_mut(ROOT_ID);
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
        if let Some(tt) = main.return_ty {
            if tt.ptr.level() == 0 && matches!(tt.kind, TypeKind::Basic(BasicType::S32)) {
                return;
            }
        }
        scope.error(CheckError::MainProcWrongRetType, main.name.span);
    }

    //@duplicates are not removed
    fn pass_2_check_namesets(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            let mut name_set = HashMap::<InternID, Span>::new();

            macro_rules! redefinition_check {
                ($element_list:expr, $check_error:expr) => {
                    if $element_list.is_empty() {
                        continue;
                    }
                    name_set.clear();
                    for element in $element_list.iter() {
                        if let Some(existing) = name_set.get(&element.name.id) {
                            let file_id = self.get_scope(scope_id).md().file_id;
                            scope_error!(
                                self,
                                scope_id,
                                Error::check($check_error, file_id, element.name.span).context(
                                    "already defined here",
                                    file_id,
                                    *existing
                                )
                            );
                        } else {
                            name_set.insert(element.name.id, element.name.span);
                        }
                    }
                };
            }

            let scope = self.get_scope(scope_id);
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

    fn pass_3_process_imports(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            let mut import_tasks = Vec::new();
            let mut were_resolved = 0;

            {
                let scope = self.get_scope(scope_id);
                for decl in scope.module.decls {
                    if let Decl::Import(import) = decl {
                        import_tasks.push(ImportTask {
                            import,
                            status: ImportTaskStatus::Unresolved,
                        });
                    }
                }
            }

            while import_tasks
                .iter()
                .any(|task| task.status != ImportTaskStatus::Resolved)
            {
                for task in import_tasks.iter_mut() {
                    self.import_task_run(scope_id, task);
                }

                let resolved_count = import_tasks
                    .iter()
                    .filter(|task| task.status == ImportTaskStatus::Resolved)
                    .count();
                if resolved_count <= were_resolved {
                    for task in import_tasks.iter_mut() {
                        if task.status == ImportTaskStatus::SourceNotFound {
                            let file_id = self.get_scope(scope_id).md().file_id;
                            scope_error!(
                                self,
                                scope_id,
                                Error::check(
                                    CheckError::ModuleNotFoundInScope,
                                    file_id,
                                    task.import.path.names.first().unwrap().span,
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

    fn import_task_run(&mut self, scope_id: ScopeID, task: &mut ImportTask) {
        if task.status == ImportTaskStatus::Resolved {
            return;
        }

        //@emit good error for empty paths
        if task.import.path.names.is_empty() {
            task.status = ImportTaskStatus::Resolved;
            return;
        }

        if task.import.path.kind == PathKind::None {
            let first = task.import.path.names.first().unwrap();
            if !self.scope_in_scope_mod_exists(scope_id, first) {
                task.status = ImportTaskStatus::SourceNotFound;
                return;
            }
        }
        task.status = ImportTaskStatus::Resolved;

        let from_scope = match self.scope_resolve_module_path(scope_id, task.import.path) {
            Some(from_scope) => from_scope,
            None => return,
        };

        if from_scope == scope_id {
            self.get_scope_mut(scope_id)
                .error(CheckError::ImportFromItself, task.import.span);
            return;
        }

        match task.import.target {
            ImportTarget::GlobAll => {
                let import = GlobImport {
                    from_id: from_scope,
                    import_span: task.import.span,
                };
                if let Err(existing) = self.get_scope_mut(scope_id).add_glob_import(import) {
                    let file_id = self.get_scope(scope_id).md().file_id;
                    scope_error!(
                        self,
                        scope_id,
                        Error::check(CheckError::ImportGlobExists, file_id, task.import.span,)
                            .context("existing import", file_id, existing.import_span)
                    );
                }
            }
            ImportTarget::Symbol(name) => {
                self.scope_import_symbol(scope_id, from_scope, name);
            }
            ImportTarget::SymbolList(symbol_list) => {
                for name in symbol_list {
                    self.scope_import_symbol(scope_id, from_scope, name);
                }
            }
        }
    }

    fn scope_in_scope_mod_exists(&self, scope_id: ScopeID, name: Ident) -> bool {
        let scope = self.get_scope(scope_id);
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

    fn scope_import_symbol(&mut self, scope_id: ScopeID, from_scope_id: ScopeID, name: Ident) {
        let mut some_exists = false;
        let mut private_symbols = Vec::new();
        let from_scope = self.get_scope(from_scope_id);

        if let Some(mod_decl) = from_scope.get_mod(name.id) {
            if visibility_private(mod_decl.vis, from_scope_id) {
                private_symbols.push(mod_decl.name);
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.get_proc(name.id) {
            if visibility_private(data.decl.vis, from_scope_id) {
                private_symbols.push(data.decl.name);
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.get_type(name.id) {
            if visibility_private(data.vis(), from_scope_id) {
                private_symbols.push(data.name());
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.get_global(name.id) {
            if visibility_private(data.decl.vis, from_scope_id) {
                private_symbols.push(data.decl.name);
            } else {
                some_exists = true;
            }
        }

        if !some_exists {
            let file_id = self.get_scope(scope_id).md().file_id;
            let mut error = Error::check(CheckError::ImportSymbolNotDefined, file_id, name.span);
            for name in private_symbols {
                error = error.context(
                    "found this private symbol",
                    from_scope.md().file_id,
                    name.span,
                );
            }
            scope_error!(self, scope_id, error);
            return;
        }

        let import = SymbolImport {
            from_id: from_scope.id,
            name,
        };

        if let Err(existing) = self.get_scope_mut(scope_id).add_symbol_import(import) {
            let file_id = self.get_scope(scope_id).md().file_id;
            scope_error!(
                self,
                scope_id,
                Error::check(CheckError::ImportSymbolAlreadyImported, file_id, name.span,).context(
                    "existing symbol import",
                    file_id,
                    existing.name.span
                )
            );
        }
    }

    fn pass_4_type_resolve(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            self.curr_scope = scope_id;
            let module = self.get_scope(self.curr_scope).module.copy();
            visit::visit_module_with(self, module);
        }
    }

    fn scope_find_proc(&mut self, scope_id: ScopeID, path: Path, name: Ident) -> Option<ProcData> {
        if path.kind == PathKind::None && path.names.is_empty() {
            return self.scope_get_in_scope_proc(scope_id, name);
        }
        let from_scope = match self.scope_resolve_module_path(scope_id, path) {
            Some(from_scope) => from_scope,
            None => return None,
        };
        let data = match self.get_scope(from_scope).get_proc(name.id) {
            Some(data) => data,
            None => {
                self.get_scope_mut(scope_id)
                    .error(CheckError::ProcNotDeclaredInPath, name.span);
                return None;
            }
        };
        if visibility_private(data.decl.vis, from_scope) {
            let file_id = self.get_scope(scope_id).md().file_id;
            let from_file_id = self.get_scope(from_scope).md().file_id;
            scope_error!(
                self,
                scope_id,
                Error::check(CheckError::ProcIsPrivate, file_id, name.span).context(
                    "defined here",
                    from_file_id,
                    data.decl.name.span
                )
            );
            return None;
        }
        return Some(data);
    }

    fn scope_find_type(&mut self, scope_id: ScopeID, path: Path, name: Ident) -> Option<TypeData> {
        if path.kind == PathKind::None && path.names.is_empty() {
            return self.scope_get_in_scope_type(scope_id, name);
        }
        let from_scope = match self.scope_resolve_module_path(scope_id, path) {
            Some(from_scope) => from_scope,
            None => return None,
        };
        let data = match self.get_scope(from_scope).get_type(name.id) {
            Some(data) => data,
            None => {
                let scope = self.get_scope_mut(scope_id);
                scope.error(CheckError::TypeNotDeclaredInPath, name.span);
                return None;
            }
        };
        if visibility_private(data.vis(), from_scope) {
            let file_id = self.get_scope(scope_id).md().file_id;
            let from_file_id = self.get_scope(from_scope).md().file_id;
            scope_error!(
                self,
                scope_id,
                Error::check(CheckError::TypeIsPrivate, file_id, name.span).context(
                    "defined here",
                    from_file_id,
                    data.name().span
                )
            );
            return None;
        }
        return Some(data);
    }

    fn scope_find_global(
        &mut self,
        scope_id: ScopeID,
        path: Path,
        name: Ident,
    ) -> Option<GlobalData> {
        if path.kind == PathKind::None && path.names.is_empty() {
            return self.scope_get_in_scope_global(scope_id, name);
        }
        let from_scope = match self.scope_resolve_module_path(scope_id, path) {
            Some(from_scope) => from_scope,
            None => return None,
        };
        let data = match self.get_scope(from_scope).get_global(name.id) {
            Some(data) => data,
            None => {
                let scope = self.get_scope_mut(scope_id);
                scope.error(CheckError::GlobalNotDeclaredInPath, name.span);
                return None;
            }
        };
        if visibility_private(data.decl.vis, from_scope) {
            let file_id = self.get_scope(scope_id).md().file_id;
            let from_file_id = self.get_scope(from_scope).md().file_id;
            scope_error!(
                self,
                scope_id,
                Error::check(CheckError::GlobalIsPrivate, file_id, name.span).context(
                    "defined here",
                    from_file_id,
                    data.decl.name.span
                )
            );
            return None;
        }
        return Some(data);
    }

    fn scope_resolve_module_path(&mut self, scope_id: ScopeID, path: Path) -> Option<ScopeID> {
        let mut target_id = match path.kind {
            PathKind::None => {
                let first = match path.names.first() {
                    Some(name) => name,
                    None => return Some(scope_id),
                };
                let mod_decl = match self.scope_get_in_scope_mod(scope_id, first) {
                    Some(mod_decl) => mod_decl,
                    None => return None,
                };
                match mod_decl.id {
                    Some(id) => id,
                    None => {
                        let scope = self.get_scope_mut(scope_id);
                        scope.error(CheckError::ModuleFileReportedMissing, first.span);
                        return None;
                    }
                }
            }
            PathKind::Super => {
                let scope = self.get_scope(scope_id);
                if let Some(parent) = scope.parent {
                    parent
                } else {
                    let scope = self.get_scope_mut(scope_id);
                    let span = Span::new(path.span_start, path.span_start + 5);
                    scope.error(CheckError::SuperUsedFromRootModule, span);
                    return None;
                }
            }
            PathKind::Package => ROOT_ID,
        };

        let mut skip_first = path.kind == PathKind::None;
        for name in path.names {
            if skip_first {
                skip_first = false;
                continue;
            }
            let mod_decl = match self.get_scope(target_id).get_mod(name.id) {
                Some(mod_decl) => mod_decl,
                None => {
                    let scope = self.get_scope_mut(scope_id);
                    scope.error(CheckError::ModuleNotDeclaredInPath, name.span);
                    return None;
                }
            };
            if visibility_private(mod_decl.vis, target_id) {
                let file_id = self.get_scope(scope_id).md().file_id;
                let target_file_id = self.get_scope(target_id).md().file_id;
                scope_error!(
                    self,
                    scope_id,
                    Error::check(CheckError::ModuleIsPrivate, file_id, name.span).context(
                        "defined here",
                        target_file_id,
                        mod_decl.name.span
                    )
                );
                return None;
            }
            target_id = match mod_decl.id {
                Some(id) => id,
                None => {
                    let scope = self.get_scope_mut(scope_id);
                    scope.error(CheckError::ModuleFileReportedMissing, name.span);
                    return None;
                }
            };
        }

        return Some(target_id);
    }

    fn scope_get_in_scope_mod(&mut self, scope_id: ScopeID, name: Ident) -> Option<P<ModuleDecl>> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<P<ModuleDecl>>>::new();

        let scope = self.get_scope(scope_id);
        if let Some(data) = scope.get_mod(name.id) {
            unique = Some(Conflit::new(data, scope.id, None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(mod_decl) = from_scope.get_mod(name.id) {
                if visibility_public(mod_decl.vis, import.from_id) {
                    let conflict = Conflit::new(mod_decl, from_scope.id, Some(import.name.span));
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
                    let conflict = Conflit::new(mod_decl, from_scope.id, Some(import.import_span));
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
                    let scope = self.get_scope_mut(scope_id);
                    scope.error(CheckError::ModuleNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(
            CheckError::ModuleSymbolConflict,
            scope.md().file_id,
            name.span,
        );
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md().file_id, import_span)
                    .context(
                        "conflict with this module",
                        self.get_scope(conflict.from_scope).md().file_id,
                        conflict.data.name.span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared module",
                    scope.md().file_id,
                    conflict.data.name.span,
                );
            }
        }
        scope_error!(self, scope_id, error);
        return None;
    }

    fn scope_get_in_scope_proc(&mut self, scope_id: ScopeID, name: Ident) -> Option<ProcData> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<ProcData>>::new();

        let scope = self.get_scope(scope_id);
        if let Some(data) = scope.get_proc(name.id) {
            unique = Some(Conflit::new(data, scope.id, None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_proc(name.id) {
                if visibility_public(data.decl.vis, import.from_id) {
                    let conflict = Conflit::new(data, from_scope.id, Some(import.name.span));
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
                    let conflict = Conflit::new(data, from_scope.id, Some(import.import_span));
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
                    let scope = self.get_scope_mut(scope_id);
                    scope.error(CheckError::ProcNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(
            CheckError::ProcSymbolConflict,
            scope.md().file_id,
            name.span,
        );
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md().file_id, import_span)
                    .context(
                        "conflict with this procedure",
                        self.get_scope(conflict.from_scope).md().file_id,
                        conflict.data.decl.name.span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared procedure",
                    scope.md().file_id,
                    conflict.data.decl.name.span,
                );
            }
        }
        scope_error!(self, scope_id, error);
        return None;
    }

    fn scope_get_in_scope_type(&mut self, scope_id: ScopeID, name: Ident) -> Option<TypeData> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<TypeData>>::new();

        let scope = self.get_scope(scope_id);
        if let Some(data) = scope.get_type(name.id) {
            unique = Some(Conflit::new(data, scope.id, None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_type(name.id) {
                if visibility_public(data.vis(), import.from_id) {
                    let conflict = Conflit::new(data, from_scope.id, Some(import.name.span));
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
                    let conflict = Conflit::new(data, from_scope.id, Some(import.import_span));
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
                    let scope = self.get_scope_mut(scope_id);
                    scope.error(CheckError::TypeNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(
            CheckError::TypeSymbolConflict,
            scope.md().file_id,
            name.span,
        );
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md().file_id, import_span)
                    .context(
                        "conflict with this type name",
                        self.get_scope(conflict.from_scope).md().file_id,
                        conflict.data.name().span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared type name",
                    scope.md().file_id,
                    conflict.data.name().span,
                );
            }
        }
        scope_error!(self, scope_id, error);
        return None;
    }

    fn scope_get_in_scope_global(&mut self, scope_id: ScopeID, name: Ident) -> Option<GlobalData> {
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<GlobalData>>::new();

        let scope = self.get_scope(scope_id);
        if let Some(data) = scope.get_global(name.id) {
            unique = Some(Conflit::new(data, scope.id, None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.get_global(name.id) {
                if visibility_public(data.decl.vis, import.from_id) {
                    let conflict = Conflit::new(data, from_scope.id, Some(import.name.span));
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
                    let conflict = Conflit::new(data, from_scope.id, Some(import.import_span));
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
                    let scope = self.get_scope_mut(scope_id);
                    scope.error(CheckError::GlobalNotFoundInScope, name.span);
                    None
                }
            };
        }
        if let Some(conflict) = unique {
            conflicts.insert(0, conflict);
        }

        let mut error = Error::check(
            CheckError::GlobalSymbolConflict,
            scope.md().file_id,
            name.span,
        );
        for conflict in conflicts.iter() {
            if let Some(import_span) = conflict.import_span {
                error = error
                    .context("from this import:", scope.md().file_id, import_span)
                    .context(
                        "conflict with this global constant",
                        self.get_scope(conflict.from_scope).md().file_id,
                        conflict.data.decl.name.span,
                    )
            } else {
                error = error.context(
                    "conflict with this declared global constant",
                    scope.md().file_id,
                    conflict.data.decl.name.span,
                );
            }
        }
        scope_error!(self, scope_id, error);
        return None;
    }

    fn pass_6_check_control_flow(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            let scope = self.get_scope(scope_id);
            for decl in scope.module.decls {
                if let Decl::Proc(proc_decl) = decl {
                    if let Some(block) = proc_decl.block {
                        self.scope_check_control_flow(scope_id, block, false, false, false);
                    }
                }
            }
        }
    }

    fn scope_check_control_flow(
        &mut self,
        scope_id: ScopeID,
        block: P<Block>,
        in_loop: bool,
        in_defer: bool,
        mut is_unreachable: bool,
    ) {
        //@todo very bloated code
        //@todo ban continue / break in defer blocks for outside loops
        //@todo ban return in defer blocks
        let mut terminated = false;
        let mut term_span: Span = Span::new(0, 0); //@no span
        let mut term_errored = is_unreachable;
        for stmt in block.stmts {
            if terminated && !term_errored {
                term_errored = true;
                let file_id = self.get_scope(scope_id).md().file_id;
                scope_error!(
                    self,
                    scope_id,
                    Error::check(CheckError::UnreachableStatement, file_id, stmt.span).context(
                        "any code following this statement is unreachable",
                        file_id,
                        term_span
                    )
                );
            }
            match stmt.kind {
                /* @is expr StmtKind::If(if_) => {
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
                }*/
                StmtKind::For(for_) => {
                    self.scope_check_control_flow(
                        scope_id,
                        for_.block,
                        true,
                        in_defer,
                        is_unreachable,
                    );
                }
                StmtKind::Defer(defer) => {
                    if in_defer {
                        let scope = self.get_scope_mut(scope_id);
                        scope.error(CheckError::DeferNested, stmt.span);
                    }
                    self.scope_check_control_flow(scope_id, defer, in_loop, true, is_unreachable);
                }
                StmtKind::Break => {
                    if !in_loop {
                        let scope = self.get_scope_mut(scope_id);
                        scope.error(CheckError::BreakOutsideLoop, stmt.span);
                    } else {
                        terminated = true;
                        term_span = stmt.span;
                        is_unreachable = true;
                    }
                }
                StmtKind::Return(..) => {
                    terminated = true;
                    term_span = stmt.span;
                    is_unreachable = true;
                }
                StmtKind::Continue => {
                    if !in_loop {
                        let scope = self.get_scope_mut(scope_id);
                        scope.error(CheckError::ContinueOutsideLoop, stmt.span);
                    } else {
                        terminated = true;
                        term_span = stmt.span;
                        is_unreachable = true;
                    }
                }
                _ => {} //@rejecting all others
            }
        }
    }

    fn pass_7_check_proc(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            for decl in self.get_scope(scope_id).module.decls {
                if let Decl::Proc(proc_decl) = decl {
                    self.scope_check_proc(scope_id, proc_decl);
                }
            }
        }
    }

    fn scope_check_proc(&mut self, scope_id: ScopeID, proc_decl: P<ProcDecl>) {
        let mut proc_scope = ProcScope { vars: Vec::new() };
        if let Some(block) = proc_decl.block {
            self.check_block(scope_id, &mut proc_scope, block);
        }
    }

    fn check_block(&mut self, scope_id: ScopeID, proc_scope: &mut ProcScope, block: P<Block>) {
        for stmt in block.stmts {
            self.check_stmt(scope_id, proc_scope, stmt);
        }
    }

    fn check_stmt(&mut self, scope_id: ScopeID, proc_scope: &mut ProcScope, stmt: Stmt) {
        match stmt.kind {
            StmtKind::Break => {}
            StmtKind::Continue => {}
            StmtKind::For(for_) => {
                self.check_block(scope_id, proc_scope, for_.block);
            }
            StmtKind::Defer(block) => self.check_block(scope_id, proc_scope, block),
            StmtKind::Return(_) => {}
            StmtKind::VarDecl(var_decl) => self.check_var_decl(scope_id, proc_scope, var_decl),
            StmtKind::VarAssign(_) => {}
            StmtKind::ExprSemi(_) => {}
            StmtKind::ExprTail(_) => {}
        }
    }

    fn check_var_decl(
        &mut self,
        scope_id: ScopeID,
        proc_scope: &mut ProcScope,
        var_decl: P<VarDecl>,
    ) {
        if let Some(name) = var_decl.name {
            let file_id = self.get_scope(scope_id).md().file_id;
            match proc_scope.find_var_decl(name.id) {
                Some(existing) => {
                    scope_error!(
                        self,
                        scope_id,
                        Error::check(CheckError::VarLocalAlreadyDeclared, file_id, name.span)
                            .context(
                                "already declared here",
                                file_id,
                                existing.name.unwrap().span //@name exists to get a duplicate
                            )
                    );
                }
                None => {
                    proc_scope.vars.push(var_decl);
                }
            }
        }
    }
}

struct ProcScope {
    vars: Vec<P<VarDecl>>,
}

impl ProcScope {
    fn find_var_decl(&self, id: InternID) -> Option<P<VarDecl>> {
        for var in self.vars.iter() {
            if let Some(name) = var.name {
                if name.id == id {
                    return Some(*var);
                }
            }
        }
        None
    }
}
