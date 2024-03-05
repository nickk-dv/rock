mod pass_1;

use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::CompError;
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
    let hir = hir::Hir::new();
    let mut hir_temp = hir_temp::HirTemp::new(ast);
    pass_1::run_scope_tree_gen(ctx, &mut hir_temp);
    Ok(hir)
}
