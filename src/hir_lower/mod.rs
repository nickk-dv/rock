mod pass_1;
mod pass_2;
mod pass_3;

use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::ErrorComp;
use crate::hir;
use crate::hir::hir_builder as hb;

pub fn check<'ctx, 'ast, 'hir>(
    ctx: &'ctx CompCtx,
    ast: ast::Ast<'ast>,
) -> Result<hir::Hir<'hir>, Vec<ErrorComp>> {
    let mut hb = hb::HirBuilder::new(ctx, ast);
    pass_1::run(&mut hb);
    pass_2::run(&mut hb);
    pass_3::run(&mut hb);
    hb.finish()
}
