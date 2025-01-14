use super::context::{Codegen, Expect};
use super::emit_expr;
use crate::llvm;
use rock_core::hir;

pub fn codegen_block<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, block: hir::Block<'c>) {
    cg.proc.block_enter();
    for stmt in block.stmts {
        codegen_stmt(cg, expect, *stmt);
    }
    cg.proc.block_exit();
}

fn codegen_stmt<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, stmt: hir::Stmt<'c>) {
    match stmt {
        hir::Stmt::Break => codegen_break(cg),
        hir::Stmt::Continue => codegen_continue(cg),
        hir::Stmt::Return(expr) => codegen_return(cg, expr),
        hir::Stmt::Loop(block) => codegen_loop(cg, block),
        hir::Stmt::Local(local) => codegen_local(cg, local),
        hir::Stmt::Discard(value) => codegen_discard(cg, value),
        hir::Stmt::Assign(assign) => codegen_assign(cg, assign),
        hir::Stmt::ExprSemi(expr) => codegen_expr_semi(cg, expr),
        hir::Stmt::ExprTail(expr) => codegen_expr_tail(cg, expect, expr),
    }
}

fn codegen_break(cg: &mut Codegen) {
    let loop_info = cg.proc.last_loop_info();
    cg.build.br(loop_info.break_bb);
}

fn codegen_continue(cg: &mut Codegen) {
    let loop_info = cg.proc.last_loop_info();
    cg.build.br(loop_info.continue_bb);
}

fn codegen_return<'c>(cg: &mut Codegen<'c, '_, '_>, expr: Option<&hir::Expr<'c>>) {
    let value =
        if let Some(expr) = expr { emit_expr::codegen_expr_value_opt(cg, expr) } else { None };
    cg.build.ret(value);
}

fn codegen_loop<'c>(cg: &mut Codegen<'c, '_, '_>, block: &hir::Block<'c>) {
    let entry_bb = cg.append_bb("loop_entry");
    let body_bb = cg.append_bb("loop_body");
    let exit_bb = cg.append_bb("loop_exit");
    cg.proc.set_next_loop_info(exit_bb, entry_bb);

    cg.build.br(entry_bb);
    cg.build.position_at_end(entry_bb);
    cg.build.br(body_bb);
    cg.build.position_at_end(body_bb);
    codegen_block(cg, Expect::Value(None), *block);
    cg.build_br_no_term(entry_bb);
    cg.build.position_at_end(exit_bb);
}

fn codegen_local<'c>(cg: &mut Codegen<'c, '_, '_>, local: &hir::Local<'c>) {
    let var = cg.hir.proc_data(cg.proc.proc_id).variable(local.var_id);
    let var_ptr = cg.proc.variable_ptrs[local.var_id.index()];

    match local.init {
        hir::LocalInit::Init(expr) => {
            emit_expr::codegen_expr_store(cg, expr, var_ptr);
        }
        hir::LocalInit::Zeroed => {
            let zero_init = llvm::const_zeroed(cg.ty(var.ty));
            cg.build.store(zero_init, var_ptr);
        }
        hir::LocalInit::Undefined => {}
    }
}

//@generates not needed load for tail values
// values should be fully ignored and not stored anywhere
fn codegen_discard<'c>(cg: &mut Codegen<'c, '_, '_>, value: Option<&hir::Expr<'c>>) {
    if let Some(expr) = value {
        emit_expr::codegen_expr_value_opt(cg, expr);
    }
}

fn codegen_assign<'c>(cg: &mut Codegen<'c, '_, '_>, assign: &hir::Assign<'c>) {
    let lhs_ptr = emit_expr::codegen_expr_pointer(cg, assign.lhs);

    match assign.op {
        hir::AssignOp::Assign => {
            emit_expr::codegen_expr_store(cg, assign.rhs, lhs_ptr);
        }
        hir::AssignOp::Bin(op) => {
            let lhs_ty = cg.ty(assign.lhs_ty);
            let lhs_val = cg.build.load(lhs_ty, lhs_ptr, "load_val");
            let rhs_val = emit_expr::codegen_expr_value(cg, assign.rhs);
            let bin_val = emit_expr::codegen_binary_op(cg, op, lhs_val, rhs_val);
            cg.build.store(bin_val, lhs_ptr);
        }
    }
}

fn codegen_expr_semi<'c>(cg: &mut Codegen<'c, '_, '_>, expr: &hir::Expr<'c>) {
    let _ = emit_expr::codegen_expr_value_opt(cg, expr);
}

fn codegen_expr_tail<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, expr: &hir::Expr<'c>) {
    emit_expr::codegen_expr_tail(cg, expect, expr);
}
