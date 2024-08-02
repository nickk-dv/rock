use super::context::{Codegen, ProcCodegen};
use super::emit_expr;
use rock_core::hir;

pub fn codegen_block<'c>(cg: &Codegen, proc_cg: &mut ProcCodegen<'c>, block: hir::Block<'c>) {
    proc_cg.block_enter();
    for stmt in block.stmts {
        codegen_stmt(cg, proc_cg, *stmt);
    }
    if !cg.insert_bb_terminated() {
        let defer_range = proc_cg.last_defer_blocks();
        codegen_defer_blocks(cg, proc_cg, defer_range);
    }
    proc_cg.block_exit();
}

fn codegen_stmt<'c>(cg: &Codegen, proc_cg: &mut ProcCodegen<'c>, stmt: hir::Stmt<'c>) {
    match stmt {
        hir::Stmt::Break => codegen_break(cg, proc_cg),
        hir::Stmt::Continue => codegen_continue(cg, proc_cg),
        hir::Stmt::Return(expr) => codegen_return(cg, proc_cg, expr),
        hir::Stmt::Defer(block) => proc_cg.add_defer_block(*block),
        hir::Stmt::Loop(loop_) => codegen_loop(cg, proc_cg, loop_),
        hir::Stmt::Local(local_id) => codegen_local(cg, proc_cg, local_id),
        hir::Stmt::Assign(assign) => codegen_assign(cg, proc_cg, assign),
        hir::Stmt::ExprSemi(expr) => codegen_expr_semi(cg, proc_cg, expr),
        hir::Stmt::ExprTail(expr) => codegen_expr_tail(cg, proc_cg, expr),
    }
}

fn codegen_break(cg: &Codegen, proc_cg: &mut ProcCodegen) {
    let (loop_info, defer_range) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, defer_range);
    cg.build.br(loop_info.break_bb);
}

fn codegen_continue(cg: &Codegen, proc_cg: &mut ProcCodegen) {
    let (loop_info, defer_range) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, defer_range);
    cg.build.br(loop_info.continue_bb);
}

fn codegen_return(cg: &Codegen, proc_cg: &mut ProcCodegen, expr: Option<&hir::Expr>) {
    let defer_range = proc_cg.all_defer_blocks();
    codegen_defer_blocks(cg, proc_cg, defer_range);

    if let Some(expr) = expr {
        unimplemented!();
    } else {
        cg.build.ret_void();
    }
}

fn codegen_defer_blocks(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen,
    defer_range: std::ops::Range<usize>,
) {
    if defer_range.is_empty() {
        return;
    }
    for block_idx in defer_range {
        let defer_bb = cg.append_bb(proc_cg, "defer_bb");
        cg.build.br(defer_bb);
        cg.build.position_at_end(defer_bb);
        let block = proc_cg.defer_block(block_idx);
        codegen_block(cg, proc_cg, block);
    }
    let exit_bb = cg.append_bb(proc_cg, "defer_exit");
    cg.build.br(exit_bb);
    cg.build.position_at_end(exit_bb);
}

fn codegen_loop<'c>(cg: &Codegen, proc_cg: &mut ProcCodegen<'c>, loop_: &hir::Loop<'c>) {
    let entry_bb = cg.append_bb(proc_cg, "loop_entry");
    let body_bb = cg.append_bb(proc_cg, "loop_body");
    let exit_bb = cg.append_bb(proc_cg, "loop_exit");
    proc_cg.set_next_loop_info(exit_bb, entry_bb);

    match loop_.kind {
        hir::LoopKind::Loop => {
            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);
            cg.build.br(body_bb);

            cg.build.position_at_end(body_bb);
            codegen_block(cg, proc_cg, loop_.block);
            cg.build_br_no_term(entry_bb);
        }
        hir::LoopKind::While { cond } => {
            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);
            let cond = emit_expr::codegen_expr_value(cg, proc_cg, cond);
            cg.build.cond_br(cond, body_bb, exit_bb);

            cg.build.position_at_end(body_bb);
            codegen_block(cg, proc_cg, loop_.block);
            cg.build_br_no_term(entry_bb);
        }
        hir::LoopKind::ForLoop {
            local_id,
            cond,
            assign,
        } => {
            codegen_local(cg, proc_cg, local_id);
            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);
            let cond = emit_expr::codegen_expr_value(cg, proc_cg, cond);
            cg.build.cond_br(cond, body_bb, exit_bb);

            cg.build.position_at_end(body_bb);
            codegen_block(cg, proc_cg, loop_.block);
            if !cg.insert_bb_terminated() {
                codegen_assign(cg, proc_cg, assign);
            }
            cg.build_br_no_term(entry_bb);
        }
    }
}

fn codegen_local(cg: &Codegen, proc_cg: &mut ProcCodegen, local_id: hir::LocalID) {
    unimplemented!();
}

fn codegen_assign(cg: &Codegen, proc_cg: &mut ProcCodegen, assign: &hir::Assign) {
    unimplemented!();
}

fn codegen_expr_semi(cg: &Codegen, proc_cg: &mut ProcCodegen, expr: &hir::Expr) {
    unimplemented!();
}

fn codegen_expr_tail(cg: &Codegen, proc_cg: &mut ProcCodegen, expr: &hir::Expr) {
    unimplemented!();
}
