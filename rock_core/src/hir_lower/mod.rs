mod check_directive;
mod check_path;
mod constant;
mod context;
mod match_check;
mod pass_1;
mod pass_2;
mod pass_3;
mod pass_5;
mod pass_6;

use crate::hir;
use crate::session::Session;
use context::HirCtx;

pub fn check<'hir>(session: &mut Session) -> Result<hir::Hir<'hir>, ()> {
    let mut ctx = HirCtx::new(session);
    pass_1::populate_scopes(&mut ctx);
    pass_2::resolve_imports(&mut ctx);
    pass_3::process_items(&mut ctx);
    constant::resolve_const_dependencies(&mut ctx);
    pass_5::typecheck_procedures(&mut ctx);
    pass_6::finalize_checks(&mut ctx);
    ctx.finish()
}
