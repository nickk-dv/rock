mod attr_check;
mod constant;
mod context;
mod errors;
mod pass_1;
mod pass_2;
mod pass_3;
mod pass_5;
mod pass_6;
mod proc_scope;

use crate::ast;
use crate::error::ResultComp;
use crate::hir;
use crate::session::Session;
use context::HirCtx;

pub fn check<'hir, 'ast: 'hir>(
    ast: ast::Ast<'ast>,
    session: &Session,
) -> ResultComp<hir::Hir<'hir>> {
    let mut ctx = HirCtx::new(ast, session);
    pass_1::populate_scopes(&mut ctx, session);
    pass_2::resolve_imports(&mut ctx, session);
    pass_3::process_items(&mut ctx);
    constant::resolve_const_dependencies(&mut ctx);
    pass_5::typecheck_procedures(&mut ctx);
    pass_6::check_entry_point(&mut ctx, session);
    ctx.hir_emit()
}
