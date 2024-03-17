pub mod hir_builder;
mod pass_1;
mod pass_2;
mod pass_3;
mod pass_4;
mod pass_5;

use crate::ast::ast;
use crate::ast::CompCtx;
use crate::error::ErrorComp;
use crate::hir;
use hir_builder as hb;

pub fn check<'ctx, 'ast, 'hir>(
    ctx: &'ctx mut CompCtx,
    ast: ast::Ast<'ast>,
) -> Result<hir::Hir<'hir>, Vec<ErrorComp>> {
    let mut hb = hb::HirBuilder::new(ctx, ast);
    hb.ctx_mut().timing_start();
    hb.ctx_mut().timing_start();
    pass_1::run(&mut hb);
    hb.ctx_mut().timing_end("hir lower pass1 `module tree`");
    hb.ctx_mut().timing_start();
    pass_2::run(&mut hb);
    hb.ctx_mut().timing_end("hir lower pass2 `use resolve`");
    hb.ctx_mut().timing_start();
    pass_3::run(&mut hb);
    hb.ctx_mut().timing_end("hir lower pass3 `decl process`");
    hb.ctx_mut().timing_start();
    pass_4::run(&mut hb);
    hb.ctx_mut().timing_end("hir lower pass4 `const resolve`");
    hb.ctx_mut().timing_start();
    pass_5::run(&mut hb);
    hb.ctx_mut().timing_end("hir lower pass5 `typecheck`");
    hb.ctx_mut().timing_end("hir lower passes");

    let bytes = hb.arena().mem_usage();
    hb.ctx_mut()
        .add_trace(format!("arena hir: mem usage {bytes} bytes",));
    hb.finish()
}
