use crate::ast::{ast::*, span::Span};
use crate::mem::*;
use std::collections::{HashMap, HashSet};

pub fn check(ast: &mut Ast) -> Result<(), ()> {
    let mut context = Context {
        ast,
        errors: Vec::new(),
        module_scopes: Vec::new(),
    };

    context.pass_0_populate_scopes();
    context.pass_1_check_decls();

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
}

struct Scope {
    module: P<Module>,
    declared_symbols: HashMap<InternID, Decl>,
    imported_symbols: HashMap<InternID, Decl>,
}

struct Error {
    source: SourceID,
    span: Span,
}

impl Error {
    pub fn new(source: SourceID, span: Span) -> Self {
        Self { source, span }
    }
}

impl<'ast> Context<'ast> {
    fn report_errors(&self) {
        for err in self.errors.iter() {
            crate::err::report::err(self.ast, err.source, err.span);
        }
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
                Decl::Proc(proc_decl) => Some(proc_decl.name),
                Decl::Enum(enum_decl) => Some(enum_decl.name),
                Decl::Struct(struct_decl) => Some(struct_decl.name),
                Decl::Global(global_decl) => Some(global_decl.name),
                _ => None,
            };
            match symbol {
                Some(ident) => {
                    if declared_symbols.contains_key(&ident.id) {
                        self.errors.push(Error::new(module.source, ident.span));
                    } else {
                        declared_symbols.insert(ident.id, decl);
                    }
                }
                None => {
                    continue;
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
                                self.errors.push(Error::new(module.source, param.name.span));
                            } else {
                                name_set.insert(param.name.id);
                            }
                        }
                    }
                    Decl::Enum(enum_decl) => {
                        let mut name_set = HashSet::new();
                        for variant in enum_decl.variants.iter() {
                            if name_set.contains(&variant.name.id) {
                                self.errors
                                    .push(Error::new(module.source, variant.name.span));
                            } else {
                                name_set.insert(variant.name.id);
                            }
                        }
                    }
                    Decl::Struct(struct_decl) => {
                        let mut name_set = HashSet::new();
                        for field in struct_decl.fields.iter() {
                            if name_set.contains(&field.name.id) {
                                self.errors.push(Error::new(module.source, field.name.span));
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
}
