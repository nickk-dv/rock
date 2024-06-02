use super::context::{Codegen, ProcCodegen};
use super::emit_expr::{codegen_bin_op, codegen_expr};
use crate::ast;
use crate::hir;
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

    let insert_bb = cg.get_insert_bb();
    if insert_bb.get_terminator().is_none() {
        codegen_defer_blocks(cg, proc_cg, proc_cg.last_defer_blocks().as_slice());
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
        let value = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca);
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
    let entry_block = cg
        .context
        .append_basic_block(proc_cg.function, "loop_entry");
    let body_block = cg.context.append_basic_block(proc_cg.function, "loop_body");
    let exit_block = cg.context.append_basic_block(proc_cg.function, "loop_exit");

    cg.builder.build_unconditional_branch(entry_block).unwrap();
    proc_cg.set_next_loop_info(exit_block, entry_block);

    match loop_.kind {
        hir::LoopKind::Loop => {
            cg.builder.position_at_end(entry_block);
            cg.builder.build_unconditional_branch(body_block).unwrap();

            cg.builder.position_at_end(body_block);
            codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);

            //@hack, might not be valid when break / continue are used 06.05.24
            // other cfg will make body_block not the actual block we want
            if body_block.get_terminator().is_none() {
                cg.builder.position_at_end(body_block);
                cg.builder.build_unconditional_branch(body_block).unwrap();
            }
        }
        hir::LoopKind::While { cond } => {
            cg.builder.position_at_end(entry_block);
            let cond =
                codegen_expr(cg, proc_cg, false, cond, BlockKind::TailAlloca).expect("value");
            cg.builder
                .build_conditional_branch(cond.into_int_value(), body_block, exit_block)
                .unwrap();

            cg.builder.position_at_end(body_block);
            codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);

            //@hack, likely wrong if positioned in the wrong place
            if cg
                .builder
                .get_insert_block()
                .unwrap()
                .get_terminator()
                .is_none()
            {
                cg.builder.build_unconditional_branch(entry_block).unwrap();
            }
        }
        hir::LoopKind::ForLoop {
            local_id,
            cond,
            assign,
        } => {
            cg.builder.position_at_end(entry_block);
            codegen_local(cg, proc_cg, local_id);
            let cond =
                codegen_expr(cg, proc_cg, false, cond, BlockKind::TailAlloca).expect("value");
            cg.builder
                .build_conditional_branch(cond.into_int_value(), body_block, exit_block)
                .unwrap();

            cg.builder.position_at_end(body_block);
            codegen_block(cg, proc_cg, loop_.block, BlockKind::TailIgnore);

            //@hack, often invalid (this assignment might need special block) if no iterator abstractions are used
            // in general loops need to be simplified in Hir, to loops and conditional breaks 06.05.24
            cg.builder.position_at_end(body_block);
            codegen_assign(cg, proc_cg, assign);

            //@hack, likely wrong if positioned in the wrong place
            if cg
                .builder
                .get_insert_block()
                .unwrap()
                .get_terminator()
                .is_none()
            {
                cg.builder.build_unconditional_branch(entry_block).unwrap();
            }
        }
    }

    cg.builder.position_at_end(exit_block);
}

//@variables without value expression are always zero initialized
// theres no way to detect potentially uninitialized variables
// during check and analysis phases, this might change. @06.04.24
fn codegen_local<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    local_id: hir::LocalID,
) {
    let local = cg.hir.proc_data(proc_cg.proc_id).local(local_id);
    let var_ptr = proc_cg.local_vars[local_id.index()];

    if let Some(expr) = local.value {
        if let Some(value) = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailStore(var_ptr)) {
            cg.builder.build_store(var_ptr, value).unwrap();
        }
    } else {
        let zero_value = cg.type_into_basic(local.ty).const_zero();
        cg.builder.build_store(var_ptr, zero_value).unwrap();
    }
}

fn codegen_assign<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    assign: &hir::Assign<'ctx>,
) {
    let lhs_ptr = codegen_expr(cg, proc_cg, true, assign.lhs, BlockKind::TailDissalow)
        .expect("value")
        .into_pointer_value();

    match assign.op {
        ast::AssignOp::Assign => {
            let rhs = codegen_expr(
                cg,
                proc_cg,
                false,
                assign.rhs,
                BlockKind::TailStore(lhs_ptr),
            );
            if let Some(value) = rhs {
                cg.builder.build_store(lhs_ptr, value).unwrap();
            }
        }
        ast::AssignOp::Bin(op) => {
            let lhs_ty = cg.type_into_basic(assign.lhs_ty);
            let lhs_value = cg.builder.build_load(lhs_ty, lhs_ptr, "load_val").unwrap();
            let rhs =
                codegen_expr(cg, proc_cg, false, assign.rhs, BlockKind::TailAlloca).expect("value");
            let bin_value = codegen_bin_op(cg, op, lhs_value, rhs, assign.lhs_signed_int);
            cg.builder.build_store(lhs_ptr, bin_value).unwrap();
        }
    }
}

fn codegen_expr_semi<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: &'ctx hir::Expr<'ctx>,
) {
    codegen_expr(cg, proc_cg, false, expr, BlockKind::TailIgnore);
}

//@alloca and return are not finalized and incorrect or not working 02.06.24
#[derive(Copy, Clone)]
pub enum BlockKind<'ctx> {
    /// dont do anything special with tail value  
    /// used for blocks that expect void
    TailIgnore,
    /// dissalow any tail expression  
    /// used for non addressable expressions
    TailDissalow,
    /// return tail value
    TailReturn,
    /// allocate tail value
    TailAlloca,
    /// store tail value
    TailStore(values::PointerValue<'ctx>),
}

fn codegen_expr_tail<'ctx>(
    cg: &Codegen<'ctx>,
    proc_cg: &mut ProcCodegen<'ctx>,
    expr: &'ctx hir::Expr<'ctx>,
    kind: BlockKind<'ctx>,
) {
    match kind {
        BlockKind::TailIgnore => {
            codegen_defer_blocks(cg, proc_cg, &proc_cg.last_defer_blocks());
            let _ = codegen_expr(cg, proc_cg, false, expr, kind);
        }
        BlockKind::TailDissalow => panic!("tail expression is dissalowed"),
        BlockKind::TailReturn => {
            //@defer might be generated again if tail returns are stacked?
            // is this correct? 30.05.24
            codegen_defer_blocks(cg, proc_cg, &proc_cg.all_defer_blocks());
            //@handle tail return kind differently?
            if let Some(value) = codegen_expr(cg, proc_cg, false, expr, BlockKind::TailAlloca) {
                cg.builder.build_return(Some(&value)).unwrap();
            } else {
                cg.builder.build_return(None).unwrap();
            }
        }
        BlockKind::TailAlloca => {
            panic!("tail alloca is not implemented");
        }
        BlockKind::TailStore(target_ptr) => {
            codegen_defer_blocks(cg, proc_cg, &proc_cg.last_defer_blocks());
            if let Some(value) = codegen_expr(cg, proc_cg, false, expr, kind) {
                cg.builder.build_store(target_ptr, value).unwrap();
            }
        }
    }
}
