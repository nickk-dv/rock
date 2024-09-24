mod attr_check;
mod constant;
mod context;
mod match_check;
mod pass_1;
mod pass_2;
mod pass_3;
mod pass_5;
mod pass_6;
mod proc_scope;

use crate::error::ResultComp;
use crate::hir;
use crate::session::Session;
use context::HirCtx;

pub fn check<'hir, 'ast, 's_ref>(session: &'s_ref Session<'ast>) -> ResultComp<hir::Hir<'hir>> {
    let mut ctx = HirCtx::new(session);
    pass_1::populate_scopes(&mut ctx);
    pass_2::resolve_imports(&mut ctx);
    pass_3::process_items(&mut ctx);
    constant::resolve_const_dependencies(&mut ctx);
    pass_5::typecheck_procedures(&mut ctx);
    pass_6::check_entry_point(&mut ctx);
    ctx.hir_emit()
}
