use super::symbol_table::*;
use crate::ast::{ast::*, span::Span};
use crate::err::check_err::*;
use crate::mem::*;
use std::collections::HashMap;

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
    let mut context = Context::new(ast);
    context.pass_0_create_scopes();
    context.pass_1_process_declarations();
    context.pass_2_check_main_proc();
    context.pass_3_check_decl_namesets();
    context.pass_4_process_imports();
    context.report_errors()
}

struct Context<'ast> {
    ast: &'ast mut Ast,
    scopes: Vec<Scope>,
    errors: Vec<CheckError>,
}

struct Scope {
    module: P<Module>,
    errors: Vec<Error>,
    declared: SymbolTable,
    imported: SymbolTable,
    wildcards: Vec<WildcardImport>,
}

#[derive(Copy, Clone)]
struct WildcardImport {
    from_id: SourceID,
    import_span: Span,
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

    fn get_scope(&self, scope_id: SourceID) -> &Scope {
        unsafe { self.scopes.get_unchecked(scope_id as usize) }
    }

    fn get_scope_mut(&mut self, scope_id: SourceID) -> &mut Scope {
        unsafe { self.scopes.get_unchecked_mut(scope_id as usize) }
    }

    //@its possible to have parser output already linear vector of modules
    // which will streamline the work with scopes and prevent any possible
    // issues with current module tree representation
    fn pass_0_create_scopes(&mut self) {
        self.create_scopes(self.ast.package.root);
    }

    fn create_scopes(&mut self, module: P<Module>) {
        self.scopes.push(Scope::new(module));
        for submodule in module.submodules.iter() {
            self.create_scopes(submodule);
        }
    }

    //@note if later stages will operate on list of decls duplicates must be removed from those checks
    fn pass_1_process_declarations(&mut self) {
        for scope in self.scopes.iter_mut() {
            for decl in scope.module.decls.iter() {
                match decl {
                    Decl::Mod(mod_decl) => {
                        if let Err(existing) = scope.declared.add_mod(mod_decl, scope.id()) {
                            scope.err(CheckError::ModRedefinition, mod_decl.name.span);
                            scope.err_info(existing.0.name.span, "already defined here");
                        }
                    }
                    Decl::Proc(proc_decl) => {
                        if let Err(existing) = scope.declared.add_proc(proc_decl, scope.id()) {
                            scope.err(CheckError::ProcRedefinition, proc_decl.name.span);
                            scope.err_info(existing.0.name.span, "already defined here");
                        }
                    }
                    Decl::Enum(enum_decl) => {
                        let tt = TypeSymbol::Enum(enum_decl);
                        if let Err(existing) = scope.declared.add_type(tt, scope.id()) {
                            scope.err(CheckError::TypeRedefinition, enum_decl.name.span);
                            scope.err_info(existing.0.name().span, "already defined here");
                        }
                    }
                    Decl::Struct(struct_decl) => {
                        let tt = TypeSymbol::Struct(struct_decl);
                        if let Err(existing) = scope.declared.add_type(tt, scope.id()) {
                            scope.err(CheckError::TypeRedefinition, struct_decl.name.span);
                            scope.err_info(existing.0.name().span, "already defined here");
                        }
                    }
                    Decl::Global(global_decl) => {
                        if let Err(existing) = scope.declared.add_global(global_decl, scope.id()) {
                            scope.err(CheckError::GlobalRedefinition, global_decl.name.span);
                            scope.err_info(existing.0.name.span, "already defined here");
                        }
                    }
                    Decl::Import(..) => {}
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
            for decl in scope.module.decls.iter() {
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
        for scope_id in 0..self.scopes.len() as SourceID {
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

    fn scope_import_task_collect(&mut self, scope_id: SourceID) -> Vec<ImportTask> {
        let scope = self.get_scope_mut(scope_id);
        let mut import_tasks = Vec::new();

        for decl in scope.module.decls.iter() {
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

    fn scope_import_task_run(&mut self, scope_id: SourceID, task: &mut ImportTask) {
        if task.status == ImportTaskStatus::Resolved {
            return;
        }

        let mut from_id = 0;

        if task.import.module_access.modifier == ModuleAccessModifier::None {
            let first_name = task.import.module_access.names.first().unwrap();
            //@mod publicity not considered (fine for same package access)
            //@origin might conflit with any wilcard imported / imported module
            if let Some(mod_decl) = self.get_scope(scope_id).declared.get_mod(first_name.id) {
                from_id = mod_decl.0.source;
            } else {
                task.status = ImportTaskStatus::SourceNotFound;
                return;
            }
        }

        from_id = match task.import.module_access.modifier {
            ModuleAccessModifier::None => from_id,
            ModuleAccessModifier::Super => self.get_scope(scope_id).module.parent.unwrap().source,
            ModuleAccessModifier::Package => 0,
        };

        task.status = ImportTaskStatus::Resolved;
        let mut skip_first = task.import.module_access.modifier == ModuleAccessModifier::None;

        for name in task.import.module_access.names.iter() {
            if skip_first {
                skip_first = false;
                continue;
            }

            //@mod publicity not considered (fine for same package access)
            //@non first modules taken from declared table without any possible conflits
            match self.get_scope(from_id).declared.get_mod(name.id) {
                Some(mod_decl) => from_id = mod_decl.0.source,
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
                let mut duplicate: Option<WildcardImport> = None;
                for wildcard in scope.wildcards.iter() {
                    if wildcard.from_id == from_id {
                        duplicate = Some(*wildcard);
                        continue;
                    }
                }
                match duplicate {
                    Some(existing) => {
                        scope.err(CheckError::ImportWildcardExists, task.import.span);
                        scope.err_info(existing.import_span, "existing import");
                    }
                    None => {
                        scope.wildcards.push(WildcardImport {
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
                for name in symbol_list.iter() {
                    self.scope_import_symbol(scope_id, from_id, name);
                }
            }
        }
    }

    fn scope_import_symbol(&mut self, scope_id: SourceID, from_id: SourceID, name: Ident) {
        //symbol being imported must be public + uniquely defined in source module, else its ambiguous
        //conflit might arise from symbol thats already defined or imported or wilcard public declared from same group

        let from_scope = self.get_scope(from_id);
        let symbol = match from_scope.declared.get_public_unique(name.id) {
            Ok(symbol_option) => match symbol_option {
                Some(symbol) => symbol,
                None => {
                    let all_private = from_scope.declared.get_all_private(name.id);
                    let scope = self.get_scope_mut(scope_id);
                    scope.err(CheckError::ImportPublicSymbolNotFound, name.span);
                    for private in all_private.iter() {
                        scope.err_info_external(
                            private.name().span,
                            from_id,
                            "found this private symbol",
                        );
                    }
                    return;
                }
            },
            Err(conflits) => {
                let scope = self.get_scope_mut(scope_id);
                scope.err(CheckError::ImportPublicSymbolsConflit, name.span);
                for conflit in conflits.iter() {
                    scope.err_info_external(conflit.name().span, from_id, "confliting symbol");
                }
                return;
            }
        };

        //match self.get_scope(from_id).get_declared(symbol.id) {
        //    None => {
        //        let scope = self.get_scope_mut(scope_id);
        //        scope.err(CheckError::ImportSymbolNotDefined, symbol.span);
        //    }
        //    Some(decl) => {
        //        let scope = self.get_scope_mut(scope_id);

        //        if let Decl::Mod(mod_decl) = decl {
        //            if mod_decl.source == scope_id {
        //                scope.err(CheckError::ImportItself, symbol.span);
        //                return;
        //            }
        //        }
        //        if decl.is_private() {
        //            scope.err(CheckError::ImportSymbolIsPrivate, symbol.span);
        //            return;
        //        }
        //        if let Some(existing) = scope.get_declared(symbol.id) {
        //            scope.err(CheckError::ImportSymbolAlreadyDefined, symbol.span);
        //            return;
        //        }
        //        if let Some(existing) = scope.get_imported(symbol.id) {
        //            scope.err(CheckError::ImporySymbolAlreadyImported, symbol.span);
        //            return;
        //        }
        //        scope.imported_symbols.insert(symbol.id, decl);
        //    }
    }
}

impl Scope {
    fn new(module: P<Module>) -> Self {
        Self {
            module,
            errors: Vec::new(),
            declared: SymbolTable::new(),
            imported: SymbolTable::new(),
            wildcards: Vec::new(),
        }
    }

    fn id(&self) -> SourceID {
        self.module.source
    }

    fn err(&mut self, error: CheckError, span: Span) {
        self.errors.push(Error::new(error, self.id(), span));
    }

    fn err_info(&mut self, span: Span, marker: &'static str) {
        let info = ErrorInfo {
            source: self.id(),
            span,
            marker,
        };
        unsafe {
            self.errors.last_mut().unwrap_unchecked().info.push(info);
        }
    }

    fn err_info_external(&mut self, span: Span, source: SourceID, marker: &'static str) {
        let info = ErrorInfo {
            source,
            span,
            marker,
        };
        unsafe {
            self.errors.last_mut().unwrap_unchecked().info.push(info);
        }
    }
}

impl Decl {
    fn get_name(&self) -> Option<Ident> {
        match self {
            Decl::Mod(mod_decl) => Some(mod_decl.name),
            Decl::Proc(proc_decl) => Some(proc_decl.name),
            Decl::Enum(enum_decl) => Some(enum_decl.name),
            Decl::Struct(struct_decl) => Some(struct_decl.name),
            Decl::Global(global_decl) => Some(global_decl.name),
            _ => None,
        }
    }

    fn is_private(&self) -> bool {
        // @mod decls always return false, this is valid for same package, but not for dependencies
        match self {
            Decl::Mod(..) => false,
            Decl::Proc(proc_decl) => proc_decl.visibility == Visibility::Private,
            Decl::Enum(enum_decl) => enum_decl.visibility == Visibility::Private,
            Decl::Struct(struct_decl) => struct_decl.visibility == Visibility::Private,
            Decl::Global(global_decl) => global_decl.visibility == Visibility::Private,
            Decl::Import(..) => false,
        }
    }
}
