use super::scope::*;
use super::symbol_table::*;
use crate::ast::ast::*;
use crate::ast::span::Span;
use crate::err::error::*;
use crate::mem::*;
use std::collections::HashMap;

pub const ROOT_ID: ScopeID = 0;

pub fn check(ast: P<Ast>) -> Result<(), ()> {
    let mut context = Context::new(ast);
    context.pass_0_create_scopes()?;
    context.pass_1_check_main_proc();
    context.pass_2_check_namesets(); //@duplicates are not removed from lists
    context.pass_3_process_imports();
    context.report_errors()
}

pub(super) struct Context {
    ast: P<Ast>,
    scopes: Vec<Scope>,
    errors: Vec<Error>,
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

#[derive(Copy, Clone, PartialEq)]
enum ImportTaskStatus {
    Unresolved,
    SourceNotFound,
    Resolved,
}

struct Conflit<T> {
    data: T,
    from_id: ScopeID,
    import_span: Option<Span>,
}

impl<T> Conflit<T> {
    fn new(data: T, from_id: ScopeID, import_span: Option<Span>) -> Self {
        Self {
            data,
            from_id,
            import_span,
        }
    }
}

fn visibily_private(visibility: Visibility, scope_id: ScopeID) -> bool {
    visibility == Visibility::Private && scope_id != ROOT_ID
}

fn visibily_public(visibility: Visibility, scope_id: ScopeID) -> bool {
    visibility == Visibility::Public || scope_id == ROOT_ID
}

impl Context {
    fn new(ast: P<Ast>) -> Self {
        Self {
            ast,
            scopes: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn err(&mut self, error: Error) {
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

    pub fn get_scope(&self, id: ScopeID) -> &Scope {
        //@unsafe { self.scopes.get_unchecked(scope_id as usize) }
        self.scopes.get(id as usize).unwrap()
    }

    pub fn get_scope_mut(&mut self, id: ScopeID) -> &mut Scope {
        //@unsafe { self.scopes.get_unchecked_mut(scope_id as usize) }
        self.scopes.get_mut(id as usize).unwrap()
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
                let scope = Self::create_scope(
                    self.scopes.len() as ScopeID,
                    module.copy(),
                    None,
                    &mut tasks,
                );
                file_scope_map.insert(scope.module.file.path.clone(), scope.id);
                self.scopes.push(scope);
            }
            None => {
                self.err(Error::check_no_src(CheckError::ParseMainFileMissing));
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
                    self.err(
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
                    self.err(
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
                        self.err(
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
                        self.err(
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

            let scope = Self::create_scope(
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
        id: ScopeID,
        module: P<Module>,
        parent: Option<ScopeID>,
        tasks: &mut Vec<ScopeTreeTask>,
    ) -> Scope {
        let mut scope = Scope::new(id, module.copy(), parent);

        for decl in scope.module.decls {
            match decl {
                Decl::Mod(mod_decl) => {
                    if let Some(existing) = scope.declared.add_mod(mod_decl) {
                        scope.error(
                            Error::check(
                                CheckError::RedefinitionMod,
                                scope.md(),
                                mod_decl.name.span,
                            )
                            .context(scope.md(), existing.name.span, "already defined here")
                            .into(),
                        );
                    } else {
                        tasks.push(ScopeTreeTask {
                            parent: module.copy(),
                            parent_id: scope.id,
                            mod_decl,
                        });
                    }
                }
                Decl::Proc(proc_decl) => {
                    //@temp 0 id
                    if let Some(existing) = scope.declared.add_proc(proc_decl, 0) {
                        scope.error(
                            Error::check(
                                CheckError::RedefinitionProc,
                                scope.md(),
                                proc_decl.name.span,
                            )
                            .context(scope.md(), existing.decl.name.span, "already defined here")
                            .into(),
                        );
                    }
                }
                Decl::Enum(enum_decl) => {
                    if let Some(existing) = scope.declared.add_enum(enum_decl) {
                        scope.error(
                            Error::check(
                                CheckError::RedefinitionType,
                                scope.md(),
                                enum_decl.name.span,
                            )
                            .context(scope.md(), existing.name().span, "already defined here")
                            .into(),
                        );
                    }
                }
                Decl::Struct(struct_decl) => {
                    //@temp 0 id
                    if let Some(existing) = scope.declared.add_struct(struct_decl, 0) {
                        scope.error(
                            Error::check(
                                CheckError::RedefinitionType,
                                scope.md(),
                                struct_decl.name.span,
                            )
                            .context(scope.md(), existing.name().span, "already defined here")
                            .into(),
                        );
                    }
                }
                Decl::Global(global_decl) => {
                    if let Some(existing) = scope.declared.add_global(global_decl) {
                        scope.error(
                            Error::check(
                                CheckError::RedefinitionGlobal,
                                scope.md(),
                                global_decl.name.span,
                            )
                            .context(scope.md(), existing.decl.name.span, "already defined here")
                            .into(),
                        );
                    }
                }
                Decl::Import(..) => {}
            }
        }

        scope
    }

    fn pass_1_check_main_proc(&mut self) {
        let main_id = match self.ast.intern_pool.get_id_if_exists("main".as_bytes()) {
            Some(id) => id,
            None => {
                self.err(Error::check_no_src(CheckError::MainProcMissing));
                return;
            }
        };
        let scope = self.get_scope_mut(ROOT_ID);
        let main_proc = match scope.declared.get_proc(main_id) {
            Some(data) => data.decl,
            None => {
                self.err(Error::check_no_src(CheckError::MainProcMissing));
                return;
            }
        };
        if main_proc.is_variadic {
            scope.err(CheckError::MainProcVariadic, main_proc.name.span);
        }
        if main_proc.block.is_none() {
            scope.err(CheckError::MainProcExternal, main_proc.name.span);
        }
        if !main_proc.params.is_empty() {
            scope.err(CheckError::MainProcHasParams, main_proc.name.span);
        }
        if let Some(tt) = main_proc.return_type {
            if tt.pointer_level == 0 && matches!(tt.kind, TypeKind::Basic(BasicType::S32)) {
                return;
            }
        }
        scope.err(CheckError::MainProcWrongRetType, main_proc.name.span);
    }

    fn pass_2_check_namesets(&mut self) {
        for scope in self.scopes.iter_mut() {
            let md = scope.md();
            let errors = &mut scope.errors; //@this resolved borrowing issues of calling error() on scope

            for data in scope.declared.proc_values() {
                let mut name_set = HashMap::<InternID, Ident>::new();
                for param in data.decl.params.iter() {
                    if let Some(existing) = name_set.get(&param.name.id) {
                        errors.push(
                            Error::check(
                                CheckError::ProcParamRedefinition,
                                md.copy(),
                                param.name.span,
                            )
                            .context(md.copy(), existing.span, "already defined here")
                            .into(),
                        );
                    } else {
                        name_set.insert(param.name.id, param.name);
                    }
                }
            }

            for data in scope.declared.enum_values() {
                let mut name_set = HashMap::<InternID, Ident>::new();
                for variant in data.decl.variants.iter() {
                    if let Some(existing) = name_set.get(&variant.name.id) {
                        errors.push(
                            Error::check(
                                CheckError::EnumVariantRedefinition,
                                md.copy(),
                                variant.name.span,
                            )
                            .context(md.copy(), existing.span, "already defined here")
                            .into(),
                        );
                    } else {
                        name_set.insert(variant.name.id, variant.name);
                    }
                }
            }

            for data in scope.declared.struct_values() {
                let mut name_set = HashMap::<InternID, Ident>::new();
                for field in data.decl.fields.iter() {
                    if let Some(existing) = name_set.get(&field.name.id) {
                        errors.push(
                            Error::check(
                                CheckError::StructFieldRedefinition,
                                md.copy(),
                                field.name.span,
                            )
                            .context(md.copy(), existing.span, "already defined here")
                            .into(),
                        );
                    } else {
                        name_set.insert(field.name.id, field.name);
                    }
                }
            }
        }
    }

    fn pass_3_process_imports(&mut self) {
        for scope_id in 0..self.scopes.len() as ScopeID {
            let mut import_tasks = Vec::new();
            let mut were_resolved = 0;

            let scope = self.get_scope(scope_id);
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
                    self.scope_import_task_run(scope_id, task);
                }

                let resolved_count = import_tasks
                    .iter()
                    .filter(|task| task.status == ImportTaskStatus::Resolved)
                    .count();
                if resolved_count <= were_resolved {
                    for task in import_tasks.iter_mut() {
                        if task.status == ImportTaskStatus::SourceNotFound {
                            self.get_scope_mut(scope_id).err(
                                CheckError::ModuleNotFoundInScope,
                                task.import.module_access.names.first().unwrap().span,
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

    fn scope_import_task_run(&mut self, scope_id: ScopeID, task: &mut ImportTask) {
        if task.status == ImportTaskStatus::Resolved {
            return;
        }

        if task.import.module_access.modifier == ModuleAccessModifier::None {
            let first = task.import.module_access.names.first().unwrap();
            if !self.scope_in_scope_mod_exists(scope_id, first) {
                task.status = ImportTaskStatus::SourceNotFound;
                return;
            }
        }
        task.status = ImportTaskStatus::Resolved;

        let from_id = match self.scope_resolve_module_access(scope_id, task.import.module_access) {
            Some(id) => id,
            None => return,
        };

        if from_id == scope_id {
            let scope = self.get_scope_mut(scope_id);
            scope.err(CheckError::ImportFromItself, task.import.span);
            return;
        }

        match task.import.target {
            ImportTarget::AllSymbols => {
                let scope = self.get_scope_mut(scope_id);
                let duplicate = scope
                    .glob_imports
                    .iter()
                    .find(|wildcard| wildcard.from_id == from_id);
                match duplicate {
                    Some(existing) => scope.error(
                        Error::check(
                            CheckError::ImportWildcardExists,
                            scope.md(),
                            task.import.span,
                        )
                        .context(scope.md(), existing.import_span, "existing import")
                        .into(),
                    ),
                    None => scope.glob_imports.push(GlobImport {
                        from_id,
                        import_span: task.import.span,
                    }),
                }
            }
            ImportTarget::Symbol(name) => {
                self.scope_import_symbol(scope_id, from_id, name);
            }
            ImportTarget::SymbolList(symbol_list) => {
                for name in symbol_list {
                    self.scope_import_symbol(scope_id, from_id, name);
                }
            }
        }
    }

    fn scope_in_scope_mod_exists(&self, scope_id: ScopeID, name: Ident) -> bool {
        let scope = self.get_scope(scope_id);
        if scope.declared.get_mod(name.id).is_some() {
            return true;
        }
        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if from_scope.declared.get_mod(name.id).is_some() {
                return true;
            }
        }
        for glob_import in scope.glob_imports.iter() {
            let from_scope = self.get_scope(glob_import.from_id);
            if from_scope.declared.get_mod(name.id).is_some() {
                return true;
            }
        }
        return false;
    }

    fn scope_import_symbol(&mut self, scope_id: ScopeID, from_id: ScopeID, name: Ident) {
        let scope = self.get_scope(scope_id);
        let from_scope = self.get_scope(from_id);

        let mut some_exists = false;
        let mut private_symbols = Vec::new();

        if let Some(mod_decl) = from_scope.declared.get_mod(name.id) {
            if visibily_private(mod_decl.visibility, from_id) {
                private_symbols.push(mod_decl.name);
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.declared.get_proc(name.id) {
            if visibily_private(data.decl.visibility, from_id) {
                private_symbols.push(data.decl.name);
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.declared.get_type(name.id) {
            if visibily_private(data.visibility(), from_id) {
                private_symbols.push(data.name());
            } else {
                some_exists = true;
            }
        } else if let Some(data) = from_scope.declared.get_global(name.id) {
            if visibily_private(data.decl.visibility, from_id) {
                private_symbols.push(data.decl.name);
            } else {
                some_exists = true;
            }
        }

        if !some_exists {
            let mut error = Error::check(CheckError::ImportSymbolNotDefined, scope.md(), name.span);
            for name in private_symbols {
                error = error.context(from_scope.md(), name.span, "found this private symbol");
            }
            let scope = self.get_scope_mut(scope_id);
            scope.error(error.into());
            return;
        }

        if let Some(existing) = scope.symbol_imports.get(&name.id) {
            let error = Error::check(
                CheckError::ImportSymbolAlreadyImported,
                scope.md(),
                name.span,
            )
            .context(scope.md(), existing.name.span, "existing symbol import")
            .into();
            let scope = self.get_scope_mut(scope_id);
            scope.error(error);
            return;
        }

        let scope = self.get_scope_mut(scope_id);
        scope
            .symbol_imports
            .insert(name.id, SymbolImport { from_id, name });
    }

    fn scope_find_proc(
        &mut self,
        scope_id: ScopeID,
        module_access: ModuleAccess,
        name: Ident,
    ) -> Option<ProcData> {
        if module_access.modifier == ModuleAccessModifier::None && module_access.names.is_empty() {
            return self.scope_get_in_scope_proc(scope_id, name);
        }
        let from_scope = match self.scope_resolve_module_access(scope_id, module_access) {
            Some(id) => self.get_scope(id),
            None => return None,
        };
        if let Some(data) = from_scope.declared.get_proc(name.id) {
            Some(data)
        } else {
            let scope = self.get_scope_mut(scope_id);
            scope.err(CheckError::ProcNotDeclaredInPath, name.span);
            None
        }
    }

    fn scope_find_type(
        &mut self,
        scope_id: ScopeID,
        module_access: ModuleAccess,
        name: Ident,
    ) -> Option<TypeData> {
        if module_access.modifier == ModuleAccessModifier::None && module_access.names.is_empty() {
            return self.scope_get_in_scope_type(scope_id, name);
        }
        let from_scope = match self.scope_resolve_module_access(scope_id, module_access) {
            Some(id) => self.get_scope(id),
            None => return None,
        };
        if let Some(data) = from_scope.declared.get_type(name.id) {
            Some(data)
        } else {
            let scope = self.get_scope_mut(scope_id);
            scope.err(CheckError::TypeNotDeclaredInPath, name.span);
            None
        }
    }

    //@report usages of self id module path as redundant
    //maybe still return a valid result after
    fn scope_resolve_module_access(
        &mut self,
        scope_id: ScopeID,
        module_access: ModuleAccess,
    ) -> Option<ScopeID> {
        let target_id = match module_access.modifier {
            ModuleAccessModifier::None => {
                let first = match module_access.names.first() {
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
                        scope.err(CheckError::ModuleFileReportedMissing, first.span);
                        return None;
                    }
                }
            }
            ModuleAccessModifier::Super => {
                let scope = self.get_scope_mut(scope_id);
                if let Some(parent) = scope.parent {
                    parent
                } else {
                    scope.err(
                        CheckError::SuperUsedFromRootModule,
                        module_access.modifier_span,
                    );
                    return None;
                }
            }
            ModuleAccessModifier::Package => ROOT_ID,
        };

        let mut target = self.get_scope(target_id);
        let mut skip_first = module_access.modifier == ModuleAccessModifier::None;

        for name in module_access.names {
            if skip_first {
                skip_first = false;
                continue;
            }
            let mod_decl = match target.declared.get_mod(name.id) {
                Some(mod_decl) => mod_decl,
                None => {
                    let scope = self.get_scope_mut(scope_id);
                    scope.err(CheckError::ModuleNotDeclaredInPath, name.span);
                    return None;
                }
            };
            if visibily_private(mod_decl.visibility, target.id) {
                let scope = self.get_scope_mut(scope_id);
                scope.err(CheckError::ModuleIsPrivate, name.span);
                return None;
            }
            target = match mod_decl.id {
                Some(id) => self.get_scope_mut(id),
                None => {
                    let scope = self.get_scope_mut(scope_id);
                    scope.err(CheckError::ModuleFileReportedMissing, name.span);
                    return None;
                }
            };
        }

        Some(target.id)
    }

    fn scope_get_in_scope_mod(&mut self, scope_id: ScopeID, name: Ident) -> Option<P<ModDecl>> {
        let scope = self.get_scope(scope_id);
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<P<ModDecl>>>::new();

        if let Some(data) = scope.declared.get_mod(name.id) {
            unique = Some(Conflit::new(data, scope_id, None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(mod_decl) = from_scope.declared.get_mod(name.id) {
                if visibily_public(mod_decl.visibility, import.from_id) {
                    let conflict = Conflit::new(mod_decl, import.from_id, Some(import.name.span));
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
            if let Some(mod_decl) = from_scope.declared.get_mod(name.id) {
                if visibily_public(mod_decl.visibility, import.from_id) {
                    let conflict = Conflit::new(mod_decl, import.from_id, Some(import.import_span));
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
                    scope.err(CheckError::ModuleNotFoundInScope, name.span);
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
                let from_scope = self.get_scope(conflict.from_id);
                error = error
                    .context(scope.md(), import_span, "from this import:")
                    .context(
                        from_scope.md(),
                        conflict.data.name.span,
                        "conflict with this module",
                    )
            } else {
                error = error.context(
                    scope.md(),
                    conflict.data.name.span,
                    "conflict with this declared module",
                );
            }
        }

        let scope = self.get_scope_mut(scope_id);
        scope.error(error.into());
        None
    }

    fn scope_get_in_scope_proc(&mut self, scope_id: ScopeID, name: Ident) -> Option<ProcData> {
        let scope = self.get_scope(scope_id);
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<ProcData>>::new();

        if let Some(data) = scope.declared.get_proc(name.id) {
            unique = Some(Conflit::new(data, scope_id, None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.declared.get_proc(name.id) {
                if visibily_public(data.decl.visibility, import.from_id) {
                    let conflict = Conflit::new(data, import.from_id, Some(import.name.span));
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
            if let Some(data) = from_scope.declared.get_proc(name.id) {
                if visibily_public(data.decl.visibility, import.from_id) {
                    let conflict = Conflit::new(data, import.from_id, Some(import.import_span));
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
                    scope.err(CheckError::ProcNotFoundInScope, name.span);
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
                let from_scope = self.get_scope(conflict.from_id);
                error = error
                    .context(scope.md(), import_span, "from this import:")
                    .context(
                        from_scope.md(),
                        conflict.data.decl.name.span,
                        "conflict with this procedure",
                    )
            } else {
                error = error.context(
                    scope.md(),
                    conflict.data.decl.name.span,
                    "conflict with this declared procedure",
                );
            }
        }

        let scope = self.get_scope_mut(scope_id);
        scope.error(error.into());
        None
    }

    fn scope_get_in_scope_type(&mut self, scope_id: ScopeID, name: Ident) -> Option<TypeData> {
        let scope = self.get_scope(scope_id);
        let mut unique = None;
        let mut conflicts = Vec::<Conflit<TypeData>>::new();

        if let Some(data) = scope.declared.get_type(name.id) {
            unique = Some(Conflit::new(data, scope_id, None));
        }

        if let Some(import) = scope.symbol_imports.get(&name.id) {
            let from_scope = self.get_scope(import.from_id);
            if let Some(data) = from_scope.declared.get_type(name.id) {
                if visibily_public(data.visibility(), import.from_id) {
                    let conflict = Conflit::new(data, import.from_id, Some(import.name.span));
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
            if let Some(data) = from_scope.declared.get_type(name.id) {
                if visibily_public(data.visibility(), import.from_id) {
                    let conflict = Conflit::new(data, import.from_id, Some(import.import_span));
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
                    scope.err(CheckError::TypeNotFoundInScope, name.span);
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
                let from_scope = self.get_scope(conflict.from_id);
                error = error
                    .context(scope.md(), import_span, "from this import:")
                    .context(
                        from_scope.md(),
                        conflict.data.name().span,
                        "conflict with this type name",
                    )
            } else {
                error = error.context(
                    scope.md(),
                    conflict.data.name().span,
                    "conflict with this declared type name",
                );
            }
        }

        let scope = self.get_scope_mut(scope_id);
        scope.error(error.into());
        None
    }
}
