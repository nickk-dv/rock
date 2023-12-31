use crate::ast::{ast::*, span::Span};
use crate::err::check_err::CheckError;
use crate::mem::*;
use std::collections::{HashMap, HashSet};

pub fn check(ast: &mut Ast) -> Result<(), ()> {
    let mut context = Context {
        ast,
        errors: Vec::new(),
        module_scopes: Vec::new(),
        external_procs: HashMap::new(),
    };

    context.pass_0_populate_scopes();
    context.pass_1_check_decls();
    context.pass_2_check_main_proc();
    context.pass_3_import_symbols();

    if context.errors.is_empty() {
        Ok(())
    } else {
        context.report_errors();
        Err(())
    }
}

struct Context<'ast> {
    ast: &'ast mut Ast,
    errors: Vec<Error>,
    module_scopes: Vec<Scope>,
    external_procs: HashMap<InternID, P<ProcDecl>>,
}

fn decl_is_private(decl: Decl) -> bool {
    // @mod decls always return false, this is valid for same package, but not for dependencies
    match decl {
        Decl::Mod(mod_decl) => false,
        Decl::Proc(proc_decl) => proc_decl.visibility == Visibility::Private,
        Decl::Enum(enum_decl) => enum_decl.visibility == Visibility::Private,
        Decl::Struct(struct_decl) => struct_decl.visibility == Visibility::Private,
        Decl::Global(global_decl) => global_decl.visibility == Visibility::Private,
        Decl::Import(..) => false,
    }
}

fn decl_get_span(decl: Decl) -> Span {
    match decl {
        Decl::Mod(mod_decl) => mod_decl.name.span,
        Decl::Proc(proc_decl) => proc_decl.name.span,
        Decl::Enum(enum_decl) => enum_decl.name.span,
        Decl::Struct(struct_decl) => struct_decl.name.span,
        Decl::Global(global_decl) => global_decl.name.span,
        Decl::Import(import_decl) => import_decl.span,
    }
}

struct Scope {
    module: P<Module>,
    declared_symbols: HashMap<InternID, Decl>,
    imported_symbols: HashMap<InternID, Decl>,
}

impl Scope {
    fn find_declared_module(&self, id: InternID) -> Option<P<ModDecl>> {
        match self.declared_symbols.get(&id) {
            Some(Decl::Mod(mod_decl)) => Some(*mod_decl),
            _ => None,
        }
    }

    fn find_declared_symbol(&self, id: InternID) -> Option<Decl> {
        match self.declared_symbols.get(&id) {
            Some(decl) => Some(*decl),
            _ => None,
        }
    }
}

struct Error {
    error: CheckError,
    no_souce: bool,
    source: SourceID,
    span: Span,
}

impl Error {
    pub fn new(error: CheckError, source: SourceID, span: Span) -> Self {
        Self {
            error,
            no_souce: false,
            source,
            span,
        }
    }

    pub fn new_no_context(error: CheckError) -> Self {
        Self {
            error,
            no_souce: true,
            source: 0,
            span: Span::new(1, 1),
        }
    }
}

impl<'ast> Context<'ast> {
    fn report_errors(&self) {
        for err in self.errors.iter() {
            crate::err::report::err(self.ast, err.error, err.no_souce, err.source, err.span);
        }
        println!("");
    }

    fn pass_0_populate_scopes(&mut self) {
        self.add_module_scope(self.ast.package.root);
    }

    fn add_module_scope(&mut self, module: P<Module>) {
        let scope = self.create_scope_from_module(module);
        self.module_scopes.push(scope);
        for submodule in module.submodules.iter() {
            self.add_module_scope(submodule);
        }
    }

    fn create_scope_from_module(&mut self, module: P<Module>) -> Scope {
        let mut declared_symbols = HashMap::new();

        for decl in module.decls.iter() {
            let symbol = match decl {
                Decl::Mod(mod_decl) => Some(mod_decl.name),
                Decl::Proc(proc_decl) => Some(proc_decl.name),
                Decl::Enum(enum_decl) => Some(enum_decl.name),
                Decl::Struct(struct_decl) => Some(struct_decl.name),
                Decl::Global(global_decl) => Some(global_decl.name),
                _ => None,
            };
            if let Some(name) = symbol {
                if declared_symbols.contains_key(&name.id) {
                    self.errors.push(Error::new(
                        CheckError::SymbolRedefinition,
                        module.source,
                        name.span,
                    ));
                } else {
                    declared_symbols.insert(name.id, decl);

                    if let Decl::Proc(proc_decl) = decl {
                        if proc_decl.block.is_none() {
                            if self.external_procs.contains_key(&name.id) {
                                self.errors.push(Error::new(
                                    CheckError::ExternalProcRedefinition,
                                    module.source,
                                    name.span,
                                ));
                            } else {
                                self.external_procs.insert(name.id, proc_decl);
                            }
                        }
                    }
                }
            }
        }

        Scope {
            module,
            declared_symbols,
            imported_symbols: HashMap::new(),
        }
    }

    fn pass_1_check_decls(&mut self) {
        for scope in self.module_scopes.iter() {
            let module = scope.module;
            for decl in module.decls.iter() {
                match decl {
                    Decl::Proc(proc_decl) => {
                        let mut name_set = HashSet::new();
                        for param in proc_decl.params.iter() {
                            if name_set.contains(&param.name.id) {
                                self.errors.push(Error::new(
                                    CheckError::ProcParamRedefinition,
                                    module.source,
                                    param.name.span,
                                ));
                            } else {
                                name_set.insert(param.name.id);
                            }
                        }
                    }
                    Decl::Enum(enum_decl) => {
                        let mut name_set = HashSet::new();
                        for variant in enum_decl.variants.iter() {
                            if name_set.contains(&variant.name.id) {
                                self.errors.push(Error::new(
                                    CheckError::EnumVariantRedefinition,
                                    module.source,
                                    variant.name.span,
                                ));
                            } else {
                                name_set.insert(variant.name.id);
                            }
                        }
                    }
                    Decl::Struct(struct_decl) => {
                        let mut name_set = HashSet::new();
                        for field in struct_decl.fields.iter() {
                            if name_set.contains(&field.name.id) {
                                self.errors.push(Error::new(
                                    CheckError::StructFieldRedefinition,
                                    module.source,
                                    field.name.span,
                                ));
                            } else {
                                name_set.insert(field.name.id);
                            }
                        }
                    }
                    _ => {}
                };
            }
        }
    }

    fn pass_2_check_main_proc(&mut self) {
        let main_id = self.ast.intern_pool.get_id_if_exists("main".as_bytes());
        let root_id: SourceID = 0;
        let root = self.module_scopes.get(root_id as usize).unwrap(); //@err internal?

        if let Some(id) = main_id {
            if root.declared_symbols.contains_key(&id) {
                let main_decl = root.declared_symbols.get(&id);
                match main_decl {
                    Some(decl) => match decl {
                        Decl::Proc(proc_decl) => {
                            //@check 0 input args
                            if proc_decl.is_variadic {
                                self.errors.push(Error::new(
                                    CheckError::MainProcVariadic,
                                    root_id,
                                    proc_decl.name.span,
                                ));
                            }
                            if proc_decl.block.is_none() {
                                self.errors.push(Error::new(
                                    CheckError::MainProcExternal,
                                    root_id,
                                    proc_decl.name.span,
                                ));
                            }
                            match proc_decl.return_type {
                                Some(tt) => match tt.kind {
                                    TypeKind::Basic(basic) => {
                                        if basic != BasicType::S32 {
                                            self.errors.push(Error::new(
                                                CheckError::MainProcWrongRetType,
                                                root_id,
                                                proc_decl.name.span,
                                            ));
                                        }
                                    }
                                    _ => {
                                        self.errors.push(Error::new(
                                            CheckError::MainProcWrongRetType,
                                            root_id,
                                            proc_decl.name.span,
                                        ));
                                    }
                                },
                                None => {
                                    self.errors.push(Error::new(
                                        CheckError::MainProcWrongRetType,
                                        root_id,
                                        proc_decl.name.span,
                                    ));
                                }
                            }
                        }
                        _ => {}
                    },
                    None => {}
                }
            } else {
                self.errors
                    .push(Error::new_no_context(CheckError::MainProcMissing));
            }
        } else {
            self.errors
                .push(Error::new_no_context(CheckError::MainProcMissing));
        }
    }

    fn pass_3_import_symbols(&mut self) {
        for scope in self.module_scopes.iter() {
            let module = scope.module;
            let mut imports = Vec::new();

            for decl in module.decls.iter() {
                if let Decl::Import(import_decl) = decl {
                    if import_decl.module_access.names.is_empty()
                        && import_decl.module_access.modifier == ModuleAccessModifier::None
                    {
                        self.errors.push(Error::new(
                            CheckError::ImportModuleAccessMissing,
                            module.source,
                            import_decl.span,
                        ));
                    } else {
                        imports.push(import_decl);
                    }
                }
            }

            let mut i = 0;
            while i < imports.len() {
                let import = unsafe { *imports.get_unchecked(i) };
                if import.module_access.modifier == ModuleAccessModifier::None {
                    i += 1;
                    continue;
                } else {
                    imports.remove(i);
                }

                if import.module_access.modifier == ModuleAccessModifier::Super {
                    if module.parent.is_none() {
                        self.errors.push(Error::new(
                            CheckError::SuperUsedFromRootModule,
                            module.source,
                            import.span,
                        ));
                        continue;
                    }
                }

                let mut import_scope =
                    if import.module_access.modifier == ModuleAccessModifier::Super {
                        &self.module_scopes[scope.module.parent.unwrap().source as usize]
                    } else {
                        &self.module_scopes[0]
                    };

                for name in import.module_access.names.iter() {
                    match import_scope.find_declared_module(name.id) {
                        Some(mod_decl) => {
                            println!("core has this id: {}", mod_decl.source);
                            import_scope = &self.module_scopes[mod_decl.source as usize];
                        }
                        None => {
                            self.errors.push(Error::new(
                                CheckError::ModuleNotDefined,
                                module.source,
                                name.span,
                            ));
                            continue;
                        }
                    }
                }

                if import_scope.module.source == scope.module.source {
                    self.errors.push(Error::new(
                        CheckError::ImportFromItself,
                        module.source,
                        import.span,
                    ));
                    continue;
                }

                match import.target {
                    ImportTarget::AllSymbols => todo!(),
                    ImportTarget::Module(symbol) => {
                        match import_scope.find_declared_symbol(symbol.id) {
                            Some(decl) => {
                                if let Decl::Mod(mod_decl) = decl {
                                    if mod_decl.source == module.source {
                                        self.errors.push(Error::new(
                                            CheckError::ImportItself,
                                            module.source,
                                            symbol.span,
                                        ));
                                    }
                                    //@need to exit here and not check visibility etc
                                }

                                if decl_is_private(decl) {
                                    self.errors.push(Error::new(
                                        CheckError::ImportSymbolIsPrivate,
                                        module.source,
                                        symbol.span,
                                    ));
                                    //@support another span with cyan underline and optional marker text
                                    //self.errors.push(Error::new(
                                    //    CheckError::ImportSymbolIsPrivate,
                                    //    import_scope.module.source,
                                    //    decl_get_span(decl),
                                    //));
                                } else {
                                    match scope.declared_symbols.get(&symbol.id) {
                                        Some(sym) => {
                                            self.errors.push(Error::new(
                                                CheckError::ImportSymbolAlreadyDefined,
                                                module.source,
                                                symbol.span,
                                            ));
                                        }
                                        None => {
                                            //@borrow checker scope.declared_symbols.insert(symbol.id, decl);
                                        }
                                    }
                                }
                            }
                            None => {
                                self.errors.push(Error::new(
                                    CheckError::ImportSymbolNotDefined,
                                    module.source,
                                    symbol.span,
                                ));
                            }
                        }
                    }
                    ImportTarget::SymbolList(symbols) => {
                        for symbol in symbols.iter() {
                            match import_scope.find_declared_symbol(symbol.id) {
                                Some(decl) => {
                                    if let Decl::Mod(mod_decl) = decl {
                                        if mod_decl.source == module.source {
                                            self.errors.push(Error::new(
                                                CheckError::ImportItself,
                                                module.source,
                                                symbol.span,
                                            ));
                                            continue;
                                        }
                                    }

                                    if decl_is_private(decl) {
                                        self.errors.push(Error::new(
                                            CheckError::ImportSymbolIsPrivate,
                                            module.source,
                                            symbol.span,
                                        ));
                                        //@support another span with cyan underline and optional marker text
                                        //self.errors.push(Error::new(
                                        //    CheckError::ImportSymbolIsPrivate,
                                        //    import_scope.module.source,
                                        //    decl_get_span(decl),
                                        //));
                                    } else {
                                        match scope.declared_symbols.get(&symbol.id) {
                                            Some(sym) => {
                                                self.errors.push(Error::new(
                                                    CheckError::ImportSymbolAlreadyDefined,
                                                    module.source,
                                                    symbol.span,
                                                ));
                                            }
                                            None => {
                                                //@borrow checker scope.declared_symbols.insert(symbol.id, decl);
                                            }
                                        }
                                    }
                                }
                                None => {
                                    self.errors.push(Error::new(
                                        CheckError::ImportSymbolNotDefined,
                                        module.source,
                                        symbol.span,
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            //process all imports
        }
    }
}
