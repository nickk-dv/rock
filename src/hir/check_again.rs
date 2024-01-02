use crate::ast::{ast::*, span::Span};
use crate::err::check_err::CheckError;
use crate::mem::*;
use std::borrow::Borrow;
use std::collections::HashMap;

//@note external proc uniqueness check will be delayed to after hir creation

pub fn check(ast: &mut Ast) -> Result<(), ()> {
    let mut context = Context::new(ast);
    context.pass_0_create_scopes();
    context.pass_1_add_declared_symbols();
    context.pass_2_check_main_proc();
    context.pass_3_check_decl_namesets();
    context.pass_4_import_symbols();
    context.report_errors()
}

struct Context<'ast> {
    ast: &'ast mut Ast,
    errors: Vec<Error>,
    scopes: Vec<Scope>,
}

struct Scope {
    module: P<Module>,
    errors: Vec<Error>,
    declared_symbols: HashMap<InternID, Decl>,
    imported_symbols: HashMap<InternID, Decl>,
}

struct Error {
    error: CheckError,
    no_context: bool,
    source: SourceID,
    span: Span,
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
            errors: Vec::new(),
            scopes: Vec::new(),
        }
    }

    fn err(&mut self, error: CheckError) {
        self.errors.push(Error::new_no_context(error));
    }

    fn report_errors(&self) -> Result<(), ()> {
        for err in self.errors.iter() {
            crate::err::report::err(self.ast, err.error, err.no_context, err.source, err.span);
        }
        for scope in self.scopes.iter() {
            for err in scope.errors.iter() {
                crate::err::report::err(self.ast, err.error, err.no_context, err.source, err.span);
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

    fn pass_0_create_scopes(&mut self) {
        self.create_scopes(self.ast.package.root);
    }

    fn create_scopes(&mut self, module: P<Module>) {
        self.scopes.push(Scope::new(module));
        for submodule in module.submodules.iter() {
            self.create_scopes(submodule);
        }
    }

    fn pass_1_add_declared_symbols(&mut self) {
        for scope_id in 0..self.scopes.len() as SourceID {
            self.scope_add_declared_symbol(scope_id);
        }
    }

    fn scope_add_declared_symbol(&mut self, scope_id: SourceID) {
        let scope = self.get_scope_mut(scope_id);
        for decl in scope.module.decls.iter() {
            if let Some(name) = decl.get_name() {
                if let Some(existing) = scope.declared_symbols.get(&name.id) {
                    scope.err(CheckError::SymbolRedefinition, name.span);
                } else {
                    scope.declared_symbols.insert(name.id, decl);
                }
            }
        }
    }

    //@ lib / exe package type is not considered, main is always required
    // root has id = 0 currently, id scheme might change with
    // dependencies in ast or module tree repr. changes
    fn pass_2_check_main_proc(&mut self) {
        if let Some(main_id) = self.ast.intern_pool.get_id_if_exists("main".as_bytes()) {
            let root_scope = self.get_scope(0);
            if let Some(main_proc) = root_scope.find_declared_proc(main_id) {
                self.scope_check_main_proc(0, main_proc);
            } else {
                self.err(CheckError::MainProcMissing);
            }
        } else {
            self.err(CheckError::MainProcMissing);
        }
    }

    fn scope_check_main_proc(&mut self, scope_id: SourceID, main_proc: P<ProcDecl>) {
        let scope = self.get_scope_mut(scope_id);
        if main_proc.is_variadic {
            scope.err(CheckError::MainProcVariadic, main_proc.name.span);
        }
        if main_proc.block.is_none() {
            scope.err(CheckError::MainProcExternal, main_proc.name.span);
        }
        if !main_proc.params.is_empty() {
            scope.err(CheckError::MainProcHasParams, main_proc.name.span);
        }
        let mut ret_type_valid = false;
        if let Some(tt) = main_proc.return_type {
            if tt.pointer_level == 0 {
                if let TypeKind::Basic(BasicType::S32) = tt.kind {
                    ret_type_valid = true;
                }
            }
        }
        if !ret_type_valid {
            scope.err(CheckError::MainProcWrongRetType, main_proc.name.span);
        }
    }

    fn pass_3_check_decl_namesets(&mut self) {
        for scope_id in 0..self.scopes.len() as SourceID {
            self.scope_check_decl_namesets(scope_id);
        }
    }

    fn scope_check_decl_namesets(&mut self, scope_id: SourceID) {
        let scope = self.get_scope_mut(scope_id);
        for decl in scope.module.decls.iter() {
            match decl {
                Decl::Proc(proc_decl) => {
                    let mut name_set = HashMap::<InternID, Ident>::new();
                    for param in proc_decl.params.iter() {
                        if let Some(existing) = name_set.get(&param.name.id) {
                            scope.err(CheckError::ProcParamRedefinition, param.name.span);
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
                        } else {
                            name_set.insert(field.name.id, field.name);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn pass_4_import_symbols(&mut self) {
        for scope_id in 0..self.scopes.len() as SourceID {
            self.scope_import_symbols(scope_id);
        }
    }

    fn scope_create_import_tasks(&mut self, scope_id: SourceID) -> Vec<ImportTask> {
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

    fn scope_import_symbols(&mut self, scope_id: SourceID) {
        let mut import_tasks = self.scope_create_import_tasks(scope_id);

        while import_tasks
            .iter()
            .any(|task| task.status != ImportTaskStatus::Resolved)
        {
            for task in import_tasks.iter_mut() {
                if task.status == ImportTaskStatus::Resolved {
                    continue;
                }

                let mut from_source_id = 0;

                if task.import.module_access.modifier == ModuleAccessModifier::None {
                    let first_name = task.import.module_access.names.first().unwrap();
                    if let Some(from_id) = self.get_scope(scope_id).find_module(first_name.id) {
                        from_source_id = from_id;
                    } else {
                        task.status = ImportTaskStatus::SourceNotFound;
                        task.status = ImportTaskStatus::Resolved; //@temp
                        continue;
                    }
                }

                task.status = ImportTaskStatus::Resolved;

                from_source_id = match task.import.module_access.modifier {
                    ModuleAccessModifier::None => from_source_id,
                    ModuleAccessModifier::Super => {
                        self.get_scope(scope_id).module.parent.unwrap().source
                    }
                    ModuleAccessModifier::Package => 0,
                };

                let mut skip_first =
                    task.import.module_access.modifier == ModuleAccessModifier::None;

                let mut success = true;
                for name in task.import.module_access.names.iter() {
                    if skip_first {
                        skip_first = false;
                        continue;
                    }

                    let from_scope = self.get_scope(from_source_id);
                    match from_scope.find_declared_module(name.id) {
                        Some(from_id) => from_source_id = from_id,
                        None => {
                            self.get_scope_mut(scope_id)
                                .err(CheckError::ModuleNotDefined, name.span);
                            success = false;
                            break;
                        }
                    }
                }

                if !success {
                    continue;
                }

                if from_source_id == scope_id {
                    self.get_scope_mut(scope_id)
                        .err(CheckError::ImportFromItself, task.import.span);
                    continue;
                }

                match task.import.target {
                    ImportTarget::AllSymbols => {}
                    ImportTarget::Module(symbol) => {
                        match self
                            .get_scope(from_source_id)
                            .declared_symbols
                            .get(&symbol.id)
                        {
                            None => {
                                self.get_scope_mut(scope_id)
                                    .err(CheckError::ImportSymbolNotDefined, symbol.span);
                            }
                            Some(decl) => {
                                if let Decl::Mod(mod_decl) = decl {
                                    if mod_decl.source == scope_id {
                                        self.get_scope_mut(scope_id)
                                            .err(CheckError::ImportItself, symbol.span);
                                        continue;
                                    }
                                }
                                if decl.is_private() {
                                    self.get_scope_mut(scope_id)
                                        .err(CheckError::ImportSymbolIsPrivate, symbol.span);
                                    continue;
                                }

                                if let Some(existing) =
                                    self.get_scope(scope_id).declared_symbols.get(&symbol.id)
                                {
                                    self.get_scope_mut(scope_id)
                                        .err(CheckError::ImportSymbolAlreadyDefined, symbol.span);
                                    continue;
                                }

                                if let Some(existing) = self
                                    .get_scope_mut(scope_id)
                                    .imported_symbols
                                    .get(&symbol.id)
                                {
                                    self.get_scope_mut(scope_id)
                                        .err(CheckError::ImportSymbolAlreadyDefined, symbol.span);
                                    continue;
                                }

                                self.get_scope_mut(from_source_id)
                                    .add_imported(symbol.id, *decl);
                            }
                        }
                    }
                    ImportTarget::SymbolList(symbol_list) => {}
                }
            }
        }
    }
}

impl Scope {
    fn new(module: P<Module>) -> Self {
        Self {
            module,
            errors: Vec::new(),
            declared_symbols: HashMap::new(),
            imported_symbols: HashMap::new(),
        }
    }

    fn err(&mut self, error: CheckError, span: Span) {
        self.errors
            .push(Error::new(error, self.module.source, span));
    }

    fn find_declared_proc(&self, id: InternID) -> Option<P<ProcDecl>> {
        match self.declared_symbols.get(&id) {
            Some(Decl::Proc(proc_decl)) => Some(*proc_decl),
            _ => None,
        }
    }

    fn find_declared_module(&self, id: InternID) -> Option<SourceID> {
        match self.declared_symbols.get(&id) {
            Some(Decl::Mod(mod_decl)) => return Some(mod_decl.source),
            _ => None,
        }
    }

    fn find_module(&self, id: InternID) -> Option<SourceID> {
        match self.declared_symbols.get(&id) {
            Some(Decl::Mod(mod_decl)) => return Some(mod_decl.source),
            _ => {}
        }
        match self.imported_symbols.get(&id) {
            Some(Decl::Mod(mod_decl)) => Some(mod_decl.source),
            _ => None,
        }
    }

    fn add_imported(&mut self, id: InternID, decl: Decl) {
        self.imported_symbols.insert(id, decl);
    }
}

impl Error {
    pub fn new(error: CheckError, source: SourceID, span: Span) -> Self {
        Self {
            error,
            no_context: false,
            source,
            span,
        }
    }

    pub fn new_no_context(error: CheckError) -> Self {
        Self {
            error,
            no_context: true,
            source: 0,
            span: Span::new(1, 1),
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
            Decl::Mod(mod_decl) => false,
            Decl::Proc(proc_decl) => proc_decl.visibility == Visibility::Private,
            Decl::Enum(enum_decl) => enum_decl.visibility == Visibility::Private,
            Decl::Struct(struct_decl) => struct_decl.visibility == Visibility::Private,
            Decl::Global(global_decl) => global_decl.visibility == Visibility::Private,
            Decl::Import(..) => false,
        }
    }
}
