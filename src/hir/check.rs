use super::scope::*;
use super::symbol_table::*;
use crate::ast::ast::*;
use crate::err::check_err::*;
use crate::mem::*;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn check_ast(mut ast: Ast) -> Result<(), ()> {
    let mut file_map = HashMap::new();
    for module in ast.modules.iter() {
        if let Some(existing) = file_map.insert(module.file.path.clone(), module.copy()) {
            println!("Multiple modules share same file! {:?}", existing.file.path);
        }
    }

    let mut context = Context::new(&mut ast);
    let mut root_path = PathBuf::new();
    root_path.push("test"); //@src
    root_path.push("main.lang"); //@or lib.lang

    if let Some(existing) = file_map.get(&root_path) {
        let root_module = existing.copy();
    } else {
        context.err(CheckError::ParseMainFileMissing);
        return context.report_errors();
    }

    return context.report_errors();
}

//@note external proc uniqueness check will be delayed to after hir creation

//@note resolve symbol based on 4 classes: Mod / Proc / Type / GlobalVar
// one unique symbol per that class must exist in scope
// meaning: declared, imported and public wilcard imported symbols

//@note query for GlobalVar in scope to prevent local vars shadowing them

/*
@design ideas testing:
- have per symbol kind tables in scope
: issue: how will importing symbols work then?
: solution: find symbol in each table, report if its ambiguous
- need separate imported tables to carry the source information to be able to know the source
*/

pub fn check(ast: &mut Ast) -> Result<(), ()> {
    return Ok(());
    let mut context = Context::new(ast);
    context.pass_0_create_scopes();
    context.pass_1_process_declarations();
    //@broken (no scopes added 0 doesnt exist) context.pass_2_check_main_proc();
    context.pass_3_check_decl_namesets();
    context.pass_4_process_imports();
    context.pass_5_testing();
    context.report_errors()
}

struct Context<'ast> {
    ast: &'ast mut Ast,
    scopes: Vec<Scope>,
    errors: Vec<CheckError>,
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

impl<'ast> Context<'ast> {
    fn new(ast: &'ast mut Ast) -> Self {
        Self {
            ast,
            scopes: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn err(&mut self, error: CheckError) {
        self.errors.push(error);
    }

    fn report_errors(&self) -> Result<(), ()> {
        for err in self.errors.iter() {
            crate::err::report::err_no_context(*err);
        }
        for scope in self.scopes.iter() {
            for err in scope.errors.iter() {
                crate::err::report::err(self.ast, err);
            }
        }
        if crate::err::report::did_error() {
            println!("");
            Err(())
        } else {
            Ok(())
        }
    }

    fn get_scope(&self, scope_id: ModuleID) -> &Scope {
        //@unsafe { self.scopes.get_unchecked(scope_id as usize) }
        self.scopes.get(scope_id as usize).unwrap()
    }

    fn get_scope_mut(&mut self, scope_id: ModuleID) -> &mut Scope {
        //@unsafe { self.scopes.get_unchecked_mut(scope_id as usize) }
        self.scopes.get_mut(scope_id as usize).unwrap()
    }

    //@its possible to have parser output already linear vector of modules
    // which will streamline the work with scopes and prevent any possible
    // issues with current module tree representation
    fn pass_0_create_scopes(&mut self) {
        //self.create_scopes(self.ast.package.root);
    }

    fn create_scopes(&mut self, module: P<Module>) {
        //self.scopes.push(Scope::new(module));
        //for submodule in module.submodules.iter() {
        //    self.create_scopes(submodule);
        //}
    }

    //@note if later stages will operate on list of decls duplicates must be removed from those checks
    fn pass_1_process_declarations(&mut self) {
        for scope in self.scopes.iter_mut() {
            for decl in scope.module.decls {
                if let Some(symbol) = Symbol::from_decl(decl, scope.id()) {
                    if let Err(existing) = scope.declared.add(symbol) {
                        scope.err(CheckError::SymbolRedefinition, symbol.name().span);
                        scope.err_info(existing.name().span, "already defined here");
                    }
                }
            }
        }
    }

    //@note lib / exe package type is not considered, main is always required to exist
    // root id = 0
    fn pass_2_check_main_proc(&mut self) {
        let main_id = match self.ast.intern_pool.get_id_if_exists("main".as_bytes()) {
            Some(id) => id,
            None => {
                self.err(CheckError::MainProcMissing);
                return;
            }
        };
        let scope = self.get_scope_mut(0);
        let main_proc = match scope.declared.get_proc(main_id) {
            Some(proc_decl) => proc_decl.0,
            None => {
                self.err(CheckError::MainProcMissing);
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

    //@perf creating map might not be ideal, clearing big maps can cause issues too
    // needs to be properly measured on big codebases, if this step is performance issue
    fn pass_3_check_decl_namesets(&mut self) {
        for scope in self.scopes.iter_mut() {
            for decl in scope.module.decls {
                match decl {
                    Decl::Proc(proc_decl) => {
                        let mut name_set = HashMap::<InternID, Ident>::new();
                        for param in proc_decl.params.iter() {
                            if let Some(existing) = name_set.get(&param.name.id) {
                                scope.err(CheckError::ProcParamRedefinition, param.name.span);
                                scope.err_info(existing.span, "already defined here");
                            } else {
                                name_set.insert(param.name.id, param.name);
                            }
                        }
                    }
                    Decl::Enum(enum_decl) => {
                        let mut name_set = HashMap::<InternID, Ident>::new();
                        for variant in enum_decl.variants.iter() {
                            if let Some(existing) = name_set.get(&variant.name.id) {
                                scope.err(CheckError::EnumVariantRedefinition, variant.name.span);
                                scope.err_info(existing.span, "already defined here");
                            } else {
                                name_set.insert(variant.name.id, variant.name);
                            }
                        }
                    }
                    Decl::Struct(struct_decl) => {
                        let mut name_set = HashMap::<InternID, Ident>::new();
                        for field in struct_decl.fields.iter() {
                            if let Some(existing) = name_set.get(&field.name.id) {
                                scope.err(CheckError::StructFieldRedefinition, field.name.span);
                                scope.err_info(existing.span, "already defined here");
                            } else {
                                name_set.insert(field.name.id, field.name);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn pass_4_process_imports(&mut self) {
        for scope_id in 0..self.scopes.len() as ModuleID {
            let mut import_tasks = self.scope_import_task_collect(scope_id);
            let mut were_resolved = 0;

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
                            task.status = ImportTaskStatus::Resolved;
                        }
                    }
                } else {
                    were_resolved = resolved_count;
                }
            }
        }
    }

    fn scope_import_task_collect(&mut self, scope_id: ModuleID) -> Vec<ImportTask> {
        let scope = self.get_scope_mut(scope_id);
        let mut import_tasks = Vec::new();

        for decl in scope.module.decls {
            if let Decl::Import(import) = decl {
                if import.module_access.names.is_empty()
                    && import.module_access.modifier == ModuleAccessModifier::None
                {
                    scope.err(CheckError::ImportModuleAccessMissing, import.span);
                    continue;
                }
                if import.module_access.modifier == ModuleAccessModifier::Super
                    && scope.module.parent.is_none()
                {
                    scope.err(CheckError::SuperUsedFromRootModule, import.span);
                    continue;
                }
                import_tasks.push(ImportTask {
                    import,
                    status: ImportTaskStatus::Unresolved,
                });
            }
        }
        import_tasks
    }

    fn scope_import_task_run(&mut self, scope_id: ModuleID, task: &mut ImportTask) {
        if task.status == ImportTaskStatus::Resolved {
            return;
        }

        let mut from_id = match task.import.module_access.modifier {
            ModuleAccessModifier::None => {
                //@mod publicity not considered (fine for same package access)
                // find and check conflits in declared / import / wildcards instead of just declared
                let first_name = task.import.module_access.names.first().unwrap();
                if let Some(mod_decl) = self.get_scope(scope_id).declared.get_mod(first_name.id) {
                    mod_decl.0.id
                } else {
                    task.status = ImportTaskStatus::SourceNotFound;
                    return;
                }
            }
            ModuleAccessModifier::Super => self.get_scope(scope_id).module.parent.unwrap(),
            ModuleAccessModifier::Package => 0,
        };

        task.status = ImportTaskStatus::Resolved;
        let mut skip_first = task.import.module_access.modifier == ModuleAccessModifier::None;

        for name in task.import.module_access.names {
            if skip_first {
                skip_first = false;
                continue;
            }
            //@mod publicity not considered (fine for same package access)
            match self.get_scope(from_id).declared.get_mod(name.id) {
                Some(mod_decl) => from_id = mod_decl.0.id,
                None => {
                    self.get_scope_mut(scope_id)
                        .err(CheckError::ModuleNotDeclaredInPath, name.span);
                    return;
                }
            }
        }

        if from_id == scope_id {
            self.get_scope_mut(scope_id)
                .err(CheckError::ImportFromItself, task.import.span);
            return;
        }

        match task.import.target {
            ImportTarget::AllSymbols => {
                let scope = self.get_scope_mut(scope_id);
                let mut duplicate = None;
                for wildcard in scope.wildcards.iter() {
                    if wildcard.from_id == from_id {
                        duplicate = Some(*wildcard);
                        break;
                    }
                }
                match duplicate {
                    Some(existing) => {
                        scope.err(CheckError::ImportWildcardExists, task.import.span);
                        scope.err_info(existing.import_span, "existing import");
                    }
                    None => {
                        scope.wildcards.push(Wildcard {
                            from_id,
                            import_span: task.import.span,
                        });
                    }
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

    fn scope_import_symbol(&mut self, scope_id: ModuleID, from_id: ModuleID, name: Ident) {
        //symbol being imported must be public + uniquely defined in source module, else its ambiguous
        //conflit might arise from symbol thats already defined or imported or wilcard public declared from same group

        match self.get_scope(from_id).declared.get(name.id) {
            None => {
                let scope = self.get_scope_mut(scope_id);
                scope.err(CheckError::ImportSymbolNotDefined, name.span);
            }
            Some(symbol) => {
                let scope = self.get_scope_mut(scope_id);
                if let Symbol::Mod(mod_decl) = symbol {
                    if mod_decl.0.id == scope_id {
                        scope.err(CheckError::ImportItself, name.span);
                        return;
                    }
                }
                if symbol.visibility() == Visibility::Private {
                    scope.err(CheckError::ImportSymbolIsPrivate, name.span);
                    scope.err_info_external(symbol.name().span, from_id, "this private symbol");
                    return;
                }
                if let Some(existing) = scope.declared.get(name.id) {
                    scope.err(CheckError::ImportSymbolAlreadyDefined, name.span);
                    scope.err_info(existing.name().span, "already defined here");
                    return;
                }
                if let Err(existing) = scope.imported.add(symbol) {
                    scope.err(CheckError::ImporySymbolAlreadyImported, name.span);
                }
            }
        }
    }

    fn pass_5_testing(&mut self) {
        for scope_id in 0..self.scopes.len() as ModuleID {
            for decl in self.get_scope(scope_id).module.decls {
                let proc_decl = if let Decl::Proc(proc_decl) = decl {
                    proc_decl
                } else {
                    continue;
                };
                for param in proc_decl.params.iter() {
                    let tt = param.tt;
                    match tt.kind {
                        TypeKind::Basic(_) => {}
                        TypeKind::Custom(custom) => {
                            if !custom.module_access.names.is_empty() {
                                let val = self.scope_get_in_scope_mod(
                                    custom.module_access.names.first().unwrap(),
                                    0,
                                );
                            }
                        }
                        TypeKind::ArraySlice(_) => {}
                        TypeKind::ArrayStatic(_) => {}
                    }
                }
            }
        }
    }

    fn scope_get_in_scope_mod(
        &mut self,
        name: Ident,
        scope_id: ModuleID,
    ) -> Result<(P<ModDecl>, ModuleID), ()> {
        let mut unique = self.get_scope(scope_id).declared.get_mod(name.id);
        let mut conflits = Vec::new();

        if let Some(mod_decl) = self.get_scope(scope_id).imported.get_mod(name.id) {
            if let Some(..) = unique {
                conflits.push((mod_decl.0, mod_decl.1, None));
            } else {
                unique = Some(mod_decl);
            }
        }

        for wildcard in self.get_scope(scope_id).wildcards.iter() {
            if let Some(mod_decl) = self.get_scope(wildcard.from_id).declared.get_mod(name.id) {
                if mod_decl.0.visibility == Visibility::Private {
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
                Ok(mod_decl)
            } else {
                let scope = self.get_scope_mut(scope_id);
                scope.err(CheckError::ModuleNotFoundInScope, name.span);
                Err(())
            }
        } else {
            let scope = self.get_scope_mut(scope_id);
            scope.err(CheckError::ModuleSymbolConflit, name.span);
            for conflit in conflits.iter() {
                match conflit.2 {
                    Some(import_span) => {
                        scope.err_info(import_span, "from this import");
                        scope.err_info_external(conflit.0.name.span, conflit.1, "this symbol");
                    }
                    None => {
                        scope.err_info_external(
                            conflit.0.name.span,
                            conflit.1,
                            "this symbol is imported",
                        );
                    }
                }
            }
            Err(())
        }
    }
}
