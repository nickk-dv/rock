mod hir_builder;
mod pass_1;
mod pass_2;
mod pass_3;
mod pass_4;
mod pass_5;

use crate::ast;
use crate::error::ErrorComp;
use crate::hir;
use crate::session::Session;
use hir_builder as hb;

pub fn check<'ctx, 'ast, 'hir>(
    session: &Session,
    ast: ast::Ast<'ast>,
) -> Result<hir::Hir<'hir>, Vec<ErrorComp>> {
    let mut hb = hb::HirBuilder::new(session, ast);
    pass_1::run(&mut hb);
    pass_2::run(&mut hb);
    pass_3::run(&mut hb);
    pass_4::run(&mut hb);
    pass_5::run(&mut hb);
    hb.finish()
}
