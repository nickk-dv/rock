use super::context::{Codegen, Expect, ProcCodegen};
use super::emit_expr;
use crate::llvm;
use rock_core::hir;

pub fn codegen_block<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    block: hir::Block<'c>,
) {
    proc_cg.block_enter();
    for stmt in block.stmts {
        codegen_stmt(cg, proc_cg, expect, *stmt);
    }
    if !cg.insert_bb_terminated() {
        let defer_range = proc_cg.last_defer_blocks();
        codegen_defer_blocks(cg, proc_cg, defer_range);
    }
    proc_cg.block_exit();
}

fn codegen_stmt<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    stmt: hir::Stmt<'c>,
) {
    match stmt {
        hir::Stmt::Break => codegen_break(cg, proc_cg),
        hir::Stmt::Continue => codegen_continue(cg, proc_cg),
        hir::Stmt::Return(expr) => codegen_return(cg, proc_cg, expr),
        hir::Stmt::Defer(block) => proc_cg.add_defer_block(*block),
        hir::Stmt::Loop(loop_) => codegen_loop(cg, proc_cg, loop_),
        hir::Stmt::Local(local_id) => codegen_local(cg, proc_cg, local_id),
        hir::Stmt::Assign(assign) => codegen_assign(cg, proc_cg, assign),
        hir::Stmt::ExprSemi(expr) => codegen_expr_semi(cg, proc_cg, expr),
        hir::Stmt::ExprTail(expr) => codegen_expr_tail(cg, proc_cg, expect, expr),
    }
}

fn codegen_break<'c>(cg: &Codegen<'c>, proc_cg: &mut ProcCodegen<'c>) {
    let (loop_info, defer_range) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, defer_range);
    cg.build.br(loop_info.break_bb);
}

fn codegen_continue<'c>(cg: &Codegen<'c>, proc_cg: &mut ProcCodegen<'c>) {
    let (loop_info, defer_range) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, defer_range);
    cg.build.br(loop_info.continue_bb);
}

fn codegen_return<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: Option<&hir::Expr<'c>>,
) {
    let defer_range = proc_cg.all_defer_blocks();
    codegen_defer_blocks(cg, proc_cg, defer_range);

    let value = if let Some(expr) = expr {
        emit_expr::codegen_expr_value_opt(cg, proc_cg, expr)
    } else {
        None
    };
    cg.build.ret(value);
}

fn codegen_defer_blocks<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
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
        codegen_block(cg, proc_cg, Expect::Value(None), block);
    }
    let exit_bb = cg.append_bb(proc_cg, "defer_exit");
    cg.build.br(exit_bb);
    cg.build.position_at_end(exit_bb);
}

fn codegen_loop<'c>(cg: &Codegen<'c>, proc_cg: &mut ProcCodegen<'c>, loop_: &hir::Loop<'c>) {
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
            codegen_block(cg, proc_cg, Expect::Value(None), loop_.block);
            cg.build_br_no_term(entry_bb);
        }
        hir::LoopKind::While { cond } => {
            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);
            let cond = emit_expr::codegen_expr_value(cg, proc_cg, cond);
            cg.build.cond_br(cond, body_bb, exit_bb);

            cg.build.position_at_end(body_bb);
            codegen_block(cg, proc_cg, Expect::Value(None), loop_.block);
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
            codegen_block(cg, proc_cg, Expect::Value(None), loop_.block);
            if !cg.insert_bb_terminated() {
                codegen_assign(cg, proc_cg, assign);
            }
            cg.build_br_no_term(entry_bb);
        }
    }

    cg.build.position_at_end(exit_bb);
}

fn codegen_local<'c>(cg: &Codegen<'c>, proc_cg: &mut ProcCodegen<'c>, local_id: hir::LocalID) {
    let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
    let local_ptr = proc_cg.local_ptrs[local_id.index()];

    if let Some(expr) = local.value {
        emit_expr::codegen_expr_store(cg, proc_cg, expr, local_ptr);
    } else {
        //@can be unsafe on complex types, enums, re-design local init rules
        let local_ty = cg.ty(local.ty);
        let zero_init = llvm::const_all_zero(local_ty);
        cg.build.store(zero_init, local_ptr);
    }
}

fn codegen_assign<'c>(cg: &Codegen<'c>, proc_cg: &mut ProcCodegen<'c>, assign: &hir::Assign<'c>) {
    let lhs_ptr = emit_expr::codegen_expr_pointer(cg, proc_cg, assign.lhs);

    match assign.op {
        hir::AssignOp::Assign => {
            emit_expr::codegen_expr_store(cg, proc_cg, assign.rhs, lhs_ptr);
        }
        hir::AssignOp::Bin(op) => {
            let lhs_ty = cg.ty(assign.lhs_ty);
            let lhs_val = cg.build.load(lhs_ty, lhs_ptr, "load_val");
            let rhs_val = emit_expr::codegen_expr_value(cg, proc_cg, assign.rhs);
            let bin_val = emit_expr::codegen_binary_op(cg, op, lhs_val, rhs_val);
            cg.build.store(bin_val, lhs_ptr);
        }
    }
}

fn codegen_expr_semi<'c>(cg: &Codegen<'c>, proc_cg: &mut ProcCodegen<'c>, expr: &hir::Expr<'c>) {
    let _ = emit_expr::codegen_expr_value_opt(cg, proc_cg, expr);
}

fn codegen_expr_tail<'c>(
    cg: &Codegen<'c>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    expr: &hir::Expr<'c>,
) {
    emit_expr::codegen_expr_tail(cg, proc_cg, expect, expr);
}
