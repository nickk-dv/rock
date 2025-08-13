use super::context::{Codegen, Expect};
use super::emit_expr;
use super::llvm;
use crate::hir;
use rock_core::error::SourceRange;
use rock_core::hir_lower::layout;

pub fn codegen_block<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, block: hir::Block<'c>) {
    cg.proc.block_enter();
    for stmt in block.stmts {
        codegen_stmt(cg, expect, *stmt);
    }
    cg.proc.block_exit();
}

fn codegen_stmt<'c>(cg: &mut Codegen<'c, '_, '_>, expect: Expect, stmt: hir::Stmt<'c>) {
    match stmt {
        hir::Stmt::Break => cg.build.br(cg.proc.last_loop_info().break_bb),
        hir::Stmt::Continue => cg.build.br(cg.proc.last_loop_info().continue_bb),
        hir::Stmt::Return(expr) => codegen_return(cg, expr),
        hir::Stmt::Loop(block) => codegen_loop(cg, block),
        hir::Stmt::Local(local) => codegen_local(cg, local),
        hir::Stmt::Assign(assign) => codegen_assign(cg, assign),
        hir::Stmt::ExprSemi(expr) => emit_expr::codegen_expr_discard(cg, expr),
        hir::Stmt::ExprTail(expr) => emit_expr::codegen_expr_tail(cg, expect, expr),
    }
}

fn codegen_return<'c>(cg: &mut Codegen<'c, '_, '_>, expr: Option<&hir::Expr<'c>>) {
    let value = match expr {
        Some(expr) => emit_expr::codegen_expr_value_opt(cg, expr),
        None => None,
    };
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
            let src = SourceRange::new(cg.hir.proc_data(cg.proc.proc_id).origin_id, var.name.range);
            let layout = layout::type_layout(cg, var.ty, cg.proc.poly_types, src).unwrap();

            match layout.size {
                0 => {}
                1..=32 => {
                    let zero = llvm::const_zeroed(cg.ty(var.ty));
                    cg.build.store(zero, var_ptr);
                }
                _ => {
                    let zero = llvm::const_int(cg.int_type(hir::IntType::U8), 0, false);
                    cg.build.memset(var_ptr, zero, cg.const_u64(layout.size), layout.align as u32);
                }
            }
        }
        hir::LocalInit::Undefined => {}
    }
}

fn codegen_assign<'c>(cg: &mut Codegen<'c, '_, '_>, assign: &hir::Assign<'c>) {
    let lhs_ptr = emit_expr::codegen_expr_pointer(cg, assign.lhs);

    match assign.op {
        hir::AssignOp::Assign => {
            emit_expr::codegen_expr_store(cg, assign.rhs, lhs_ptr);
        }
        hir::AssignOp::Bin(op, array) => {
            if let Some(array) = array {
                let rhs_ptr = emit_expr::codegen_expr_pointer(cg, assign.rhs);
                let bin_val = emit_expr::codegen_array_binary_op(
                    cg,
                    Expect::Value(None),
                    op,
                    array,
                    lhs_ptr,
                    rhs_ptr,
                );
                cg.build.store(bin_val.unwrap(), lhs_ptr);
            } else {
                let lhs_ty = emit_expr::bin_op_operand_type(cg, op);
                let lhs_val = cg.build.load(lhs_ty, lhs_ptr, "load_val");
                let rhs_val = emit_expr::codegen_expr_value(cg, assign.rhs);
                let bin_val = emit_expr::codegen_binary_op(cg, op, lhs_val, rhs_val);
                cg.build.store(bin_val, lhs_ptr);
            }
        }
    }
}
