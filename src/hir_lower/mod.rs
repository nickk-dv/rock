mod pass_1;
mod pass_2;

use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::ErrorComp;
use crate::hir;
use crate::hir::hir_builder;

pub fn check<'ctx, 'ast, 'hir>(
    ctx: &'ctx CompCtx,
    ast: ast::Ast<'ast>,
) -> Result<hir::Hir<'hir>, Vec<ErrorComp>> {
    let mut hir_builder = hir_builder::HirBuilder::new(ctx, ast);
    let mut errors = Vec::new();

    errors.extend(pass_1::run(&mut hir_builder));
    errors.extend(pass_2::run(&mut hir_builder));

    if errors.is_empty() {
        Ok(hir_builder.finish())
    } else {
        Err(errors)
    }
}
