use super::scope::*;
use super::symbol_table::*;
use crate::ast::ast::*;
use crate::ast::span::Span;
use crate::err::error::*;
use crate::mem::*;
use std::collections::HashMap;

const ROOT_ID: ScopeID = 0;

pub fn check(ast: P<Ast>) -> Result<(), ()> {
    let mut context = Context::new(ast);
    context.pass_0_create_scopes()?;
    context.pass_1_check_main_proc();
    context.pass_2_check_namesets(); //@duplicates are not removed from lists
    context.pass_3_process_imports();
    context.report_errors()
}

struct Context {
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

    fn get_scope(&self, id: ScopeID) -> &Scope {
        //@unsafe { self.scopes.get_unchecked(scope_id as usize) }
        self.scopes.get(id as usize).unwrap()
    }

    fn get_scope_mut(&mut self, id: ScopeID) -> &mut Scope {
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
        let mut scope = Scope {
            id,
            module: module.copy(),
            parent,
            declared: SymbolTable::new(),
            imported: SymbolTable::new(),
            wildcards: Vec::new(),
            errors: Vec::new(),
            declared2: SymbolTable2::new(),
        };

        for decl in scope.module.decls {
            match decl {
                Decl::Mod(mod_decl) => {
                    if let Some(existing) = scope.declared2.add_mod(mod_decl) {
                        scope.error(
                            Error::check(
                                CheckError::RedefinitionMod,
                                scope.md(),
                                mod_decl.name.span,
                            )
                            .context(scope.md(), existing.decl.name.span, "already defined here")
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
                    if let Some(existing) = scope.declared2.add_proc(proc_decl, 0) {
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
                    if let Some(existing) = scope.declared2.add_enum(enum_decl) {
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
                    if let Some(existing) = scope.declared2.add_struct(struct_decl, 0) {
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
                    if let Some(existing) = scope.declared2.add_global(global_decl) {
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
            Some(proc_decl) => proc_decl.0,
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

            for data in scope.declared2.proc_values() {
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

            for data in scope.declared2.enum_values() {
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

            for data in scope.declared2.struct_values() {
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
                    .wildcards
                    .iter()
                    .find(|wildcard| wildcard.from_id == from_id)
                    .copied();
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
                    None => scope.wildcards.push(Wildcard {
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

    fn scope_import_symbol(&mut self, scope_id: ScopeID, from_id: ScopeID, name: Ident) {
        let from_scope = self.get_scope_mut(from_id);
        let symbol = match from_scope.declared.get(name.id) {
            Some(symbol) => symbol,
            None => {
                let scope = self.get_scope_mut(scope_id);
                scope.err(CheckError::ImportSymbolNotDefined, name.span);
                return;
            }
        };

        let scope = self.get_scope_mut(scope_id);
        if let Symbol::Mod(mod_decl) = symbol {
            if let Some(id) = mod_decl.0.id {
                if id == scope_id {
                    scope.err(CheckError::ImportItself, name.span);
                    return;
                }
            }
        }

        if symbol.visibility() == Visibility::Private && from_id != ROOT_ID {
            let error: Error =
                Error::check(CheckError::ImportSymbolIsPrivate, scope.md(), name.span)
                    .context(
                        self.get_scope(from_id).md(),
                        symbol.name().span,
                        "this private symbol",
                    )
                    .into();
            self.get_scope_mut(scope_id).error(error);
            return;
        }

        if let Some(existing) = scope.declared.get(name.id) {
            scope.error(
                Error::check(
                    CheckError::ImportSymbolAlreadyDefined,
                    scope.md(),
                    name.span,
                )
                .context(scope.md(), existing.name().span, "already defined here")
                .into(),
            );
            return;
        }
        if let Err(..) = scope.imported.add(symbol) {
            scope.err(CheckError::ImporySymbolAlreadyImported, name.span);
        }
    }

    fn scope_resolve_module_access(
        &mut self,
        scope_id: ScopeID,
        module_access: ModuleAccess,
    ) -> Option<ScopeID> {
        let target_id = match module_access.modifier {
            ModuleAccessModifier::None => {
                let first = match module_access.names.first() {
                    Some(name) => name,
                    None => return None,
                };
                let mod_decl = match self.scope_get_in_scope_mod(first, scope_id) {
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
                    scope.err(CheckError::SuperUsedFromRootModule, Span::new(0, 0)); //@no modifier span is available
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
            if mod_decl.0.visibility == Visibility::Private && target.id != ROOT_ID {
                let scope = self.get_scope_mut(scope_id);
                scope.err(CheckError::ModuleIsPrivate, name.span);
                return None;
            }
            target = match mod_decl.0.id {
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

    fn scope_in_scope_mod_exists(&self, scope_id: ScopeID, name: Ident) -> bool {
        let scope = self.get_scope(scope_id);
        if scope.declared.get_mod(name.id).is_some() {
            return true;
        }
        if scope.imported.get_mod(name.id).is_some() {
            return true;
        }
        for wildcard in scope.wildcards.iter() {
            let from_scope = self.get_scope(wildcard.from_id);
            if from_scope.declared.get_mod(name.id).is_some() {
                return true;
            }
        }
        return false;
    }

    fn scope_get_in_scope_mod(&mut self, name: Ident, scope_id: ScopeID) -> Option<P<ModDecl>> {
        let mut unique = self.get_scope(scope_id).declared.get_mod(name.id);
        let mut conflits = Vec::new();

        //@declared and imported cannot conflit in practice, since its already checked on import resolution
        //this step can be simplified
        if let Some(mod_decl) = self.get_scope(scope_id).imported.get_mod(name.id) {
            if let Some(..) = unique {
                conflits.push((mod_decl.0, mod_decl.1, None)); //@imported span info doesnt exist
            } else {
                unique = Some(mod_decl);
            }
        }

        //@another issue unique doesnt retain wildcard span
        //once its added to conflits this info is lost
        for wildcard in self.get_scope(scope_id).wildcards.iter() {
            if let Some(mod_decl) = self.get_scope(wildcard.from_id).declared.get_mod(name.id) {
                if mod_decl.0.visibility == Visibility::Private && wildcard.from_id != ROOT_ID {
                    continue;
                }
                if let Some(..) = unique {
                    conflits.push((mod_decl.0, mod_decl.1, Some(wildcard.import_span)));
                } else {
                    unique = Some(mod_decl);
                }
            }
        }

        if conflits.is_empty() {
            if let Some(mod_decl) = unique {
                return Some(mod_decl.0);
            } else {
                let scope = self.get_scope_mut(scope_id);
                scope.err(CheckError::ModuleNotFoundInScope, name.span);
                return None;
            }
        } else {
            let source = self.get_scope(scope_id).md();
            let mut error = Error::check(CheckError::ModuleSymbolConflit, source.copy(), name.span);

            //@span info lost as mentioned earlier
            if let Some(mod_decl) = unique {
                conflits.push((mod_decl.0, mod_decl.1, None));
            }

            for conflit in conflits.iter() {
                match conflit.2 {
                    Some(import_span) => {
                        //@those with span assumed to come from wildcard imports
                        error = error
                            .context(
                                self.get_scope(conflit.1).md(),
                                conflit.0.name.span,
                                "conflits with this module",
                            )
                            .context(source.copy(), import_span, "from this import")
                    }
                    None => {
                        //@no import source
                        //@no distinct marker for declared / imported
                        error = error.context(
                            self.get_scope(conflit.1).md(),
                            conflit.0.name.span,
                            "conflits with this declared / imported module",
                        );
                    }
                }
            }
            let scope = self.get_scope_mut(scope_id);
            scope.error(error.into());
            return None;
        }
    }
}
