use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::SourceLoc;
use crate::err::error_new::{CompError, ErrorContext, Message};
use crate::hir::hir;
use crate::hir::hir_temp;

// @some structures are temporarily used to construct final Hir
// 1. scope creation from ast decl scopes (map of declarations)
// 2. imports from ast decl scopes        (proccess use declarations)
// 3. ? constants in decls
// 4. create "real" Hir scopes & add resolved variants of declarations

pub fn check<'ast, 'hir>(
    ctx: &CompCtx,
    ast: ast::Ast<'ast>,
) -> Result<hir::Hir<'hir>, Vec<CompError>> {
    let mut hir = hir::Hir::new();
    let mut hir_temp = hir_temp::HirTemp::new(ast);
    hir_pass_make_scope_tree(&mut hir_temp, ctx);
    Ok(hir)
}

fn hir_pass_make_scope_tree(hir_temp: &mut hir_temp::HirTemp, ctx: &CompCtx) {
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct ScopeTreeTask {
        module_id: ast::ModuleID,
        parent: Option<hir_temp::ModID>,
    }

    let mut module_map = HashMap::<&PathBuf, ast::ModuleID>::new();
    let mut taken_module_map = HashMap::<&PathBuf, SourceLoc>::new();
    let mut task_queue = Vec::<ScopeTreeTask>::new();

    for (idx, module) in hir_temp.modules().enumerate() {
        module_map.insert(&ctx.file(module.file_id).path, ast::ModuleID(idx as u32));
    }

    // @test is remporary path for the root, lib not supported
    // @lib is not supported yet, base on context
    let root_path: PathBuf = ["test", "main.lang"].iter().collect();
    match module_map.remove(&root_path) {
        Some(module_id) => task_queue.push(ScopeTreeTask {
            module_id,
            parent: None,
        }),
        None => {
            eprintln!("no root module found, add src/main.lang"); //@report
            return;
        }
    }

    while let Some(task) = task_queue.pop() {
        // ...
    }
}
