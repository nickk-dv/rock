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

struct Scope {
    module: P<Module>,
    declared_symbols: HashMap<InternID, Decl>,
    imported_symbols: HashMap<InternID, Decl>,
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
        for scope in self.module_scopes.iter_mut() {
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
        for scope in self.module_scopes.iter_mut() {
            let module = scope.module;
            for decl in module.decls.iter() {
                if let Decl::Import(import_decl) = decl {}
            }
        }
    }
}
