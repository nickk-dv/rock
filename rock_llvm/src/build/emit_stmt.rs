use super::context::{Codegen, Expect};
use super::emit_expr;
use crate::llvm;
use rock_core::hir;

pub fn codegen_block<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, block: hir::Block<'c>) {
    cg.proc.block_enter();
    for stmt in block.stmts {
        codegen_stmt(cg, expect, *stmt);
    }
    if !cg.insert_bb_terminated() {
        let defer_range = cg.proc.last_defer_blocks();
        codegen_defer_blocks(cg, defer_range);
    }
    cg.proc.block_exit();
}

fn codegen_stmt<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, stmt: hir::Stmt<'c>) {
    match stmt {
        hir::Stmt::Break => codegen_break(cg),
        hir::Stmt::Continue => codegen_continue(cg),
        hir::Stmt::Return(expr) => codegen_return(cg, expr),
        hir::Stmt::Defer(block) => cg.proc.add_defer_block(*block),
        hir::Stmt::For(for_) => codegen_for(cg, for_),
        hir::Stmt::Loop(block) => codegen_loop(cg, block),
        hir::Stmt::Local(local) => codegen_local(cg, local),
        hir::Stmt::Discard(value) => codegen_discard(cg, value),
        hir::Stmt::Assign(assign) => codegen_assign(cg, assign),
        hir::Stmt::ExprSemi(expr) => codegen_expr_semi(cg, expr),
        hir::Stmt::ExprTail(expr) => codegen_expr_tail(cg, expect, expr),
    }
}

fn codegen_break(cg: &mut Codegen) {
    let (loop_info, defer_range) = cg.proc.last_loop_info();
    codegen_defer_blocks(cg, defer_range);
    cg.build.br(loop_info.break_bb);
}

fn codegen_continue(cg: &mut Codegen) {
    let (loop_info, defer_range) = cg.proc.last_loop_info();
    codegen_defer_blocks(cg, defer_range);
    cg.build.br(loop_info.continue_bb);
}

fn codegen_return<'c>(cg: &mut Codegen<'c, '_, '_>, expr: Option<&hir::Expr<'c>>) {
    let defer_range = cg.proc.all_defer_blocks();
    codegen_defer_blocks(cg, defer_range);

    let value =
        if let Some(expr) = expr { emit_expr::codegen_expr_value_opt(cg, expr) } else { None };
    cg.build.ret(value);
}

fn codegen_defer_blocks(cg: &mut Codegen, defer_range: std::ops::Range<usize>) {
    if defer_range.is_empty() {
        return;
    }
    for block_idx in defer_range.rev() {
        let defer_bb = cg.append_bb("defer_bb");
        cg.build.br(defer_bb);
        cg.build.position_at_end(defer_bb);
        let block = cg.proc.defer_block(block_idx);
        codegen_block(cg, Expect::Value(None), block);
    }
    let exit_bb = cg.append_bb("defer_exit");
    cg.build.br(exit_bb);
    cg.build.position_at_end(exit_bb);
}

fn codegen_for<'c>(cg: &mut Codegen<'c, '_, '_>, for_: &hir::For<'c>) {
    let entry_bb = cg.append_bb("for_entry");
    let body_bb = cg.append_bb("for_body");
    let exit_bb = cg.append_bb("for_exit");
    cg.proc.set_next_loop_info(exit_bb, entry_bb);

    let value_ptr = cg.proc.variable_ptrs[for_.value_id.index()];
    let index_ptr = cg.proc.variable_ptrs[for_.index_id.index()];

    // evaluate value being iterated on
    let iter_ptr = emit_expr::codegen_expr_pointer(cg, for_.expr);
    let iter_ptr = if for_.deref {
        cg.build.load(cg.ptr_type(), iter_ptr, "deref_ptr").into_ptr()
    } else {
        iter_ptr
    };

    // set initial loop counter
    if for_.reverse {
        let index_init = match for_.kind {
            hir::ForElemKind::Slice => {
                let slice_ty = cg.slice_type();
                let slice_len_ptr = cg.build.gep_struct(slice_ty, iter_ptr, 1, "slice_len_ptr");
                cg.build.load(cg.ptr_sized_int(), slice_len_ptr, "slice_len")
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
    let cond = if for_.reverse {
        emit_expr::codegen_binary_op(cg, hir::BinOp::NotEq_Int, index_val, cg.const_usize_zero())
    } else {
        //@perf: loading the slice len on every iteration!
        // it could be changed if its mutated during the loop, but...
        let collection_len = match for_.kind {
            hir::ForElemKind::Slice => {
                let slice_ty = cg.slice_type();
                let slice_len_ptr = cg.build.gep_struct(slice_ty, iter_ptr, 1, "slice_len_ptr");
                cg.build.load(cg.ptr_sized_int(), slice_len_ptr, "slice_len")
            }
            hir::ForElemKind::Array(len) => cg.const_usize(cg.array_len(len)),
        };
        emit_expr::codegen_binary_op(cg, hir::BinOp::Less_IntU, index_val, collection_len)
    };

    cg.build.cond_br(cond, body_bb, exit_bb);
    cg.build.position_at_end(body_bb);

    let index_val = if for_.reverse {
        let index = cg.build.load(cg.ptr_sized_int(), index_ptr, "index_val");
        let index_sub =
            emit_expr::codegen_binary_op(cg, hir::BinOp::Sub_Int, index, cg.const_usize_one());
        cg.build.store(index_sub, index_ptr);
        index_sub
    } else {
        index_val
    };

    // get iteration value ptr (@no bounds check(not needed probably))
    let elem_ty = cg.ty(for_.elem_ty);
    let elem_ptr = match for_.kind {
        hir::ForElemKind::Slice => {
            let slice_ptr = cg.build.load(cg.ptr_type(), iter_ptr, "slice_ptr").into_ptr();
            cg.build.gep(elem_ty, slice_ptr, &[index_val], "slice_elem_ptr")
        }
        hir::ForElemKind::Array(len) => {
            let len = cg.array_len(len);
            let array_ty = llvm::array_type(elem_ty, len);
            cg.build.gep(array_ty, iter_ptr, &[cg.const_usize_zero(), index_val], "array_elem_ptr")
        }
    };

    // store iteration value
    if for_.by_pointer {
        cg.build.store(elem_ptr.as_val(), value_ptr);
    } else {
        let elem_val = cg.build.load(elem_ty, elem_ptr, "elem_val");
        cg.build.store(elem_val, value_ptr);
    };

    codegen_block(cg, Expect::Value(None), for_.block);

    // post block operation
    if !cg.insert_bb_terminated() {
        if !for_.reverse {
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

    cg.build.position_at_end(exit_bb);
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
            let zero_init = llvm::const_all_zero(cg.ty(var.ty));
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
