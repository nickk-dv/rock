use crate::ast::ast::*;
use crate::mem::P;

pub fn check(ast: &Ast) {
    check_module(ast, ast.package.root);
}

fn check_module(ast: &Ast, module: P<Module>) {
    println!(
        "checking module: {}",
        ast.files
            .get(module.source as usize)
            .unwrap()
            .path
            .to_string_lossy()
    );

    for submodule in module.submodules.iter() {
        check_module(ast, submodule);
    }
}
