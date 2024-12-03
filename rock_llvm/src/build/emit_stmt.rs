use super::context::{Codegen, Expect, ProcCodegen};
use super::emit_expr;
use crate::llvm;
use rock_core::hir;

pub fn codegen_block<'c>(
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    stmt: hir::Stmt<'c>,
) {
    match stmt {
        hir::Stmt::Break => codegen_break(cg, proc_cg),
        hir::Stmt::Continue => codegen_continue(cg, proc_cg),
        hir::Stmt::Return(expr) => codegen_return(cg, proc_cg, expr),
        hir::Stmt::Defer(block) => proc_cg.add_defer_block(*block),
        hir::Stmt::For(for_) => codegen_for(cg, proc_cg, for_),
        hir::Stmt::Local(local_id) => codegen_local(cg, proc_cg, local_id),
        hir::Stmt::Discard(value) => codegen_discard(cg, proc_cg, value),
        hir::Stmt::Assign(assign) => codegen_assign(cg, proc_cg, assign),
        hir::Stmt::ExprSemi(expr) => codegen_expr_semi(cg, proc_cg, expr),
        hir::Stmt::ExprTail(expr) => codegen_expr_tail(cg, proc_cg, expect, expr),
    }
}

fn codegen_break<'c>(cg: &Codegen<'c, '_, '_>, proc_cg: &mut ProcCodegen<'c>) {
    let (loop_info, defer_range) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, defer_range);
    cg.build.br(loop_info.break_bb);
}

fn codegen_continue<'c>(cg: &Codegen<'c, '_, '_>, proc_cg: &mut ProcCodegen<'c>) {
    let (loop_info, defer_range) = proc_cg.last_loop_info();
    codegen_defer_blocks(cg, proc_cg, defer_range);
    cg.build.br(loop_info.continue_bb);
}

fn codegen_return<'c>(
    cg: &Codegen<'c, '_, '_>,
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
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    defer_range: std::ops::Range<usize>,
) {
    if defer_range.is_empty() {
        return;
    }
    for block_idx in defer_range.rev() {
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

fn codegen_for<'c>(cg: &Codegen<'c, '_, '_>, proc_cg: &mut ProcCodegen<'c>, for_: &hir::For<'c>) {
    let entry_bb = cg.append_bb(proc_cg, "for_entry");
    let body_bb = cg.append_bb(proc_cg, "for_body");
    let exit_bb = cg.append_bb(proc_cg, "for_exit");
    proc_cg.set_next_loop_info(exit_bb, entry_bb);

    match for_.kind {
        hir::ForKind::Loop => {
            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);
            cg.build.br(body_bb);

            cg.build.position_at_end(body_bb);
            codegen_block(cg, proc_cg, Expect::Value(None), for_.block);
            cg.build_br_no_term(entry_bb);
        }
        hir::ForKind::Cond(cond) => {
            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);
            let cond = emit_expr::codegen_expr_value(cg, proc_cg, cond);
            cg.build.cond_br(cond, body_bb, exit_bb);

            cg.build.position_at_end(body_bb);
            codegen_block(cg, proc_cg, Expect::Value(None), for_.block);
            cg.build_br_no_term(entry_bb);
        }
        hir::ForKind::Elem(for_elem) => {
            let value_ptr = proc_cg.for_bind_ptrs[for_elem.value_id.index()];
            let index_ptr = proc_cg.for_bind_ptrs[for_elem.index_id.index()];

            // evaluate value being iterated on
            let iter_ptr = emit_expr::codegen_expr_pointer(cg, proc_cg, for_elem.expr);
            let iter_ptr = if for_elem.deref {
                cg.build
                    .load(cg.ptr_type(), iter_ptr, "deref_ptr")
                    .into_ptr()
            } else {
                iter_ptr
            };

            // set initial loop counter
            if for_elem.reverse {
                let index_init = match for_elem.kind {
                    hir::ForElemKind::Slice => {
                        let slice_ty = cg.slice_type();
                        let slice_len_ptr =
                            cg.build.gep_struct(slice_ty, iter_ptr, 1, "slice_len_ptr");
                        cg.build
                            .load(cg.ptr_sized_int(), slice_len_ptr, "slice_len")
                    }
                    hir::ForElemKind::Array(len) => cg.const_usize(cg.array_len(len)),
                };
                cg.build.store(index_init, index_ptr);
            } else {
                cg.build.store(cg.const_usize(0), index_ptr);
            }

            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);

            // condition: evaluate loop index
            let index_val = cg.build.load(cg.ptr_sized_int(), index_ptr, "index_val");

            // condition: evaluate loop condition
            let cond = if for_elem.reverse {
                emit_expr::codegen_binary_op(
                    cg,
                    hir::BinOp::NotEq_Int,
                    index_val,
                    cg.const_usize_zero(),
                )
            } else {
                //@perf: loading the slice len on every iteration!
                // it could be changed if its mutated during the loop, but...
                let collection_len = match for_elem.kind {
                    hir::ForElemKind::Slice => {
                        let slice_ty = cg.slice_type();
                        let slice_len_ptr =
                            cg.build.gep_struct(slice_ty, iter_ptr, 1, "slice_len_ptr");
                        cg.build
                            .load(cg.ptr_sized_int(), slice_len_ptr, "slice_len")
                    }
                    hir::ForElemKind::Array(len) => cg.const_usize(cg.array_len(len)),
                };
                emit_expr::codegen_binary_op(cg, hir::BinOp::Less_IntU, index_val, collection_len)
            };

            cg.build.cond_br(cond, body_bb, exit_bb);
            cg.build.position_at_end(body_bb);

            let index_val = if for_elem.reverse {
                let index = cg.build.load(cg.ptr_sized_int(), index_ptr, "index_val");
                let index_sub = emit_expr::codegen_binary_op(
                    cg,
                    hir::BinOp::Sub_Int,
                    index,
                    cg.const_usize_one(),
                );
                cg.build.store(index_sub, index_ptr);
                index_sub
            } else {
                index_val
            };

            // get iteration value ptr (@no bounds check(not needed probably))
            let elem_ty = cg.ty(for_elem.elem_ty);
            let elem_ptr = match for_elem.kind {
                hir::ForElemKind::Slice => {
                    let slice_ptr = cg
                        .build
                        .load(cg.ptr_type(), iter_ptr, "slice_ptr")
                        .into_ptr();
                    cg.build
                        .gep(elem_ty, slice_ptr, &[index_val], "slice_elem_ptr")
                }
                hir::ForElemKind::Array(len) => {
                    let len = cg.array_len(len);
                    let array_ty = llvm::array_type(elem_ty, len);
                    cg.build.gep(
                        array_ty,
                        iter_ptr,
                        &[cg.const_usize_zero(), index_val],
                        "array_elem_ptr",
                    )
                }
            };

            // store iteration value
            if for_elem.by_pointer {
                cg.build.store(elem_ptr.as_val(), value_ptr);
            } else {
                let elem_val = cg.build.load(elem_ty, elem_ptr, "elem_val");
                cg.build.store(elem_val, value_ptr);
            };

            codegen_block(cg, proc_cg, Expect::Value(None), for_.block);

            // post block operation
            if !cg.insert_bb_terminated() {
                if !for_elem.reverse {
                    //@this load could be skipped, by using value from the condition as lhs operand
                    let index_val = cg.build.load(cg.ptr_sized_int(), index_ptr, "index_val");
                    let index_inc = emit_expr::codegen_binary_op(
                        cg,
                        hir::BinOp::Add_Int,
                        index_val,
                        cg.const_usize_one(),
                    );
                    cg.build.store(index_inc, index_ptr);
                }
                cg.build.br(entry_bb);
            }
        }
        hir::ForKind::Pat(for_pat) => {
            //@todo implement match on single pattern
            cg.build.br(entry_bb);
            cg.build.position_at_end(entry_bb);
            cg.build.br(body_bb);
            cg.build.position_at_end(body_bb);
            cg.build.br(exit_bb);
        }
    }

    cg.build.position_at_end(exit_bb);
}

fn codegen_local<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    local_id: hir::LocalID,
) {
    let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
    let local_ptr = proc_cg.local_ptrs[local_id.index()];

    match local.init {
        hir::LocalInit::Init(expr) => {
            emit_expr::codegen_expr_store(cg, proc_cg, expr, local_ptr);
        }
        hir::LocalInit::Zeroed => {
            let zero_init = llvm::const_all_zero(cg.ty(local.ty));
            cg.build.store(zero_init, local_ptr);
        }
        hir::LocalInit::Undefined => {}
    }
}

//@generates not needed load for tail values
// values should be fully ignored and not stored anywhere
fn codegen_discard<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    value: Option<&hir::Expr<'c>>,
) {
    if let Some(expr) = value {
        emit_expr::codegen_expr_value_opt(cg, proc_cg, expr);
    }
}

fn codegen_assign<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    assign: &hir::Assign<'c>,
) {
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

fn codegen_expr_semi<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
) {
    let _ = emit_expr::codegen_expr_value_opt(cg, proc_cg, expr);
}

fn codegen_expr_tail<'c>(
    cg: &Codegen<'c, '_, '_>,
    proc_cg: &mut ProcCodegen<'c>,
    expect: Expect,
    expr: &hir::Expr<'c>,
) {
    emit_expr::codegen_expr_tail(cg, proc_cg, expect, expr);
}
