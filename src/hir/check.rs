use crate::ast::{ast::*, span::Span};
use crate::mem::*;
use std::collections::HashMap;

pub fn check(ast: &mut Ast) -> Result<(), ()> {
    let mut context = Context {
        ast,
        errors: Vec::new(),
        module_scopes: Vec::new(),
    };

    check_decl_pass(&mut context);

    if context.errors.is_empty() {
        Ok(())
    } else {
        context.report_errors();
        Err(())
    }
}

fn check_decl_pass(context: &mut Context) {
    check_module(context, context.ast.package.root);
}

fn check_module(context: &mut Context, module: P<Module>) {
    println!(
        "checking module: {}",
        context
            .ast
            .files
            .get(module.source as usize)
            .unwrap()
            .path
            .to_string_lossy()
    );

    let scope = scope_resolve_declared_symbols(context, module);
    context.module_scopes.push(scope);

    for submodule in module.submodules.iter() {
        check_module(context, submodule);
    }
}

struct Error {
    source: SourceID,
    span: Span,
}

struct Context<'ast> {
    ast: &'ast mut Ast,
    errors: Vec<Error>,
    module_scopes: Vec<Scope>,
}

impl<'ast> Context<'ast> {
    fn err(&mut self, source: SourceID, span: Span) {
        self.errors.push(Error { source, span });
    }

    fn report_errors(&self) {
        for err in self.errors.iter() {
            crate::err::report::err(self.ast, err.source, err.span);
        }
    }
}

struct Scope {
    module: P<Module>,
    declared_symbols: HashMap<InternID, Decl>,
    imported_symbols: HashMap<InternID, Decl>,
}

fn scope_resolve_declared_symbols(context: &mut Context, module: P<Module>) -> Scope {
    let mut declared_symbols = HashMap::new();

    for decl in module.decls.iter() {
        let symbol = match decl {
            Decl::Mod(..) => None,
            Decl::Proc(proc_decl) => Some(proc_decl.name),
            Decl::Enum(enum_decl) => Some(enum_decl.name),
            Decl::Struct(struct_decl) => Some(struct_decl.name),
            Decl::Global(global_decl) => Some(global_decl.name),
            Decl::Import(..) => None,
        };
        match symbol {
            Some(ident) => {
                if declared_symbols.contains_key(&ident.id) {
                    context.err(module.source, ident.span);
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
