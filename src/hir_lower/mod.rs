mod pass_1;

use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::ErrorComp;
use crate::hir;
use crate::hir::hir_builder as hb;

pub fn check<'a, 'ast, 'hir: 'ast>(
    ctx: &'a CompCtx,
    ast: ast::Ast<'ast>,
) -> Result<hir::Hir<'hir>, Vec<ErrorComp>> {
    let mut hir_temp = hb::HirBuilder::new(ctx, ast);
    let mut errors = Vec::new();

    errors.extend(pass_1::run(&mut hir_temp));

    if errors.is_empty() {
        Ok(hir_temp.finish())
    } else {
        Err(errors)
    }
}
