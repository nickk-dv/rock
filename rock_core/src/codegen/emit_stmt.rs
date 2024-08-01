use super::context::{Codegen, ProcCodegen};
use super::emit_expr::{
    codegen_bin_op, codegen_expr, codegen_expr_value, codegen_expr_value_optional,
};
use crate::ast;
use crate::hir;
use crate::id_impl;
use inkwell::types;
use inkwell::values;

pub fn codegen_block<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    block: hir::Block<'ctx>,
    kind: BlockKind<'ctx>,
) {
    proc_cg.enter_block();

    for stmt in block.stmts {
        match *stmt {
            hir::Stmt::Break => codegen_break(cg, proc_cg),
            hir::Stmt::Continue => codegen_continue(cg, proc_cg),
            hir::Stmt::Return(expr) => codegen_return(cg, proc_cg, expr),
            hir::Stmt::Defer(block) => proc_cg.push_defer_block(*block),
            hir::Stmt::Loop(loop_) => codegen_loop(cg, proc_cg, loop_),
            hir::Stmt::Local(local_id) => codegen_local(cg, proc_cg, local_id),
            hir::Stmt::Assign(assign) => codegen_assign(cg, proc_cg, assign),
            hir::Stmt::ExprSemi(expr) => codegen_expr_semi(cg, proc_cg, expr),
            hir::Stmt::ExprTail(expr) => codegen_expr_tail(cg, proc_cg, expr, kind),
        }
    }

    if !cg.insert_bb_has_term() {
        codegen_defer_blocks(cg, proc_cg, &proc_cg.last_defer_blocks());
    }
    proc_cg.exit_block();
}

fn codegen_break<'ctx>(cg: &Codegen<'ctx>, proc_cg: &mut ProcCodegen<'ctx>) {
    let (loop_info, defer_blocks) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, &defer_blocks);
    cg.build_br(loop_info.break_bb);
}

fn codegen_continue<'ctx>(cg: &Codegen<'ctx>, proc_cg: &mut ProcCodegen<'ctx>) {
    let (loop_info, defer_blocks) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, &defer_blocks);
    cg.build_br(loop_info.continue_bb);
}

fn codegen_return<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: Option<&'ctx hir::Expr<'ctx>>,
) {
    codegen_defer_blocks(cg, proc_cg, &proc_cg.all_defer_blocks());
    if let Some(expr) = expr {
        let value = codegen_expr_value_optional(cg, proc_cg, expr);
        cg.build_ret(value);
    } else {
        cg.build_ret(None);
    }
}

fn codegen_defer_blocks<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    defer_blocks: &[hir::Block<'ctx>],
) {
    if defer_blocks.is_empty() {
        return;
    }
    for block in defer_blocks.iter().copied().rev() {
        let defer_bb = cg.append_bb(proc_cg, "defer_block");
        cg.build_br(defer_bb);
        cg.position_at_end(defer_bb);
        codegen_block(cg, proc_cg, block, BlockKind::TailIgnore);
    }
    let exit_bb = cg.append_bb(proc_cg, "defer_exit");
    cg.build_br(exit_bb);
    cg.position_at_end(exit_bb);
}

fn codegen_loop<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    loop_: &'ctx hir::Loop<'ctx>,
) {
    let entry_bb = cg.append_bb(proc_cg, "loop_entry");
    let body_bb = cg.append_bb(proc_cg, "loop_body");
    let exit_bb = cg.append_bb(proc_cg, "loop_exit");
    proc_cg.set_next_loop_info(exit_bb, entry_bb);

    match loop_.kind {
        hir::LoopKind::Loop => {
            cg.build_br(entry_bb);
            cg.position_at_end(entry_bb);
            cg.build_br(body_bb);

            cg.position_at_end(body_bb);
            codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);
            cg.build_br_no_term(entry_bb);
        }
        hir::LoopKind::While { cond } => {
            cg.build_br(entry_bb);
            cg.position_at_end(entry_bb);
            let cond = codegen_expr_value(cg, proc_cg, cond);
            cg.build_cond_br(cond, body_bb, exit_bb);

            cg.position_at_end(body_bb);
            codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);
            cg.build_br_no_term(entry_bb);
        }
        hir::LoopKind::ForLoop {
            local_id,
            cond,
            assign,
        } => {
            codegen_local(cg, proc_cg, local_id);

            cg.build_br(entry_bb);
            cg.position_at_end(entry_bb);
            let cond = codegen_expr_value(cg, proc_cg, cond);
            cg.build_cond_br(cond, body_bb, exit_bb);

            cg.position_at_end(body_bb);
            codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);
            if !cg.insert_bb_has_term() {
                codegen_assign(cg, proc_cg, assign);
            }
            cg.build_br_no_term(entry_bb);
        }
    }

    cg.position_at_end(exit_bb);
}

//@variables without value expression are always zero initialized 03.06.24
// theres no way to detect potentially uninitialized variables during analysis
// this is the most problematic when dealing with slices, ptr's, proc ptr's, or types with them
fn codegen_local<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    local_id: hir::LocalID,
) {
    let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
    let local_ptr = proc_cg.local_vars[local_id.index()];

    if let Some(expr) = local.value {
        let init_value = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailStore(local_ptr));
        if let Some(value) = init_value {
            cg.builder.build_store(local_ptr, value).unwrap();
        }
    } else {
        let zero_value = cg.type_into_basic(local.ty).const_zero();
        cg.builder.build_store(local_ptr, zero_value).unwrap();
    }
}

fn codegen_assign<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    assign: &hir::Assign<'ctx>,
) {
    let lhs_ptr = codegen_expr(cg, proc_cg, true, assign.lhs, BlockKind::TailIgnore)
        .expect("value")
        .into_pointer_value();

    match assign.op {
        hir::AssignOp::Assign => {
            let init_value = codegen_expr(
                cg,
                proc_cg,
                false,
                assign.rhs,
                BlockKind::TailStore(lhs_ptr),
            );
            if let Some(value) = init_value {
                cg.builder.build_store(lhs_ptr, value).unwrap();
            }
        }
        hir::AssignOp::Bin(op) => {
            let lhs_ty = cg.type_into_basic(assign.lhs_ty);
            let lhs_value = cg.builder.build_load(lhs_ty, lhs_ptr, "load_val").unwrap();
            let init_value = codegen_expr_value(cg, proc_cg, assign.rhs);

            let bin_value = codegen_bin_op(cg, op, lhs_value, init_value);
            cg.builder.build_store(lhs_ptr, bin_value).unwrap();
        }
    }
}

fn codegen_expr_semi<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: &'ctx hir::Expr<'ctx>,
) {
    let _ = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailIgnore);
}

#[derive(Copy, Clone)]
pub enum BlockKind<'ctx> {
    TailIgnore,
    TailAlloca(TailAllocaID),
    TailStore(values::PointerValue<'ctx>),
}

id_impl!(TailAllocaID);
#[derive(Copy, Clone)]
pub enum TailAllocaStatus<'ctx> {
    NoValue,
    WithValue(values::PointerValue<'ctx>, types::BasicTypeEnum<'ctx>),
}

fn codegen_expr_tail<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: &'ctx hir::Expr<'ctx>,
    kind: BlockKind<'ctx>,
) {
    match kind {
        BlockKind::TailIgnore => {
            let _ = codegen_expr(cg, proc_cg, false, expr, kind);
        }
        BlockKind::TailAlloca(alloca_id) => {
            let tail_value = codegen_expr(cg, proc_cg, false, expr, kind);

            if let Some(value) = tail_value {
                match proc_cg.tail_alloca[alloca_id.index()] {
                    TailAllocaStatus::NoValue => {
                        let temp_ptr =
                            cg.entry_insert_alloca(proc_cg, value.get_type(), "temp_tail");
                        cg.builder.build_store(temp_ptr, value).unwrap();

                        let status = &mut proc_cg.tail_alloca[alloca_id.index()];
                        *status = TailAllocaStatus::WithValue(temp_ptr, value.get_type());
                    }
                    TailAllocaStatus::WithValue(temp_ptr, _) => {
                        cg.builder.build_store(temp_ptr, value).unwrap();
                    }
                }
            }
        }
        BlockKind::TailStore(target_ptr) => {
            if let Some(value) = codegen_expr(cg, proc_cg, false, expr, kind) {
                cg.builder.build_store(target_ptr, value).unwrap();
            }
        }
    }
}
