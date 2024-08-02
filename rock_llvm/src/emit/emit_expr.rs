use super::context::{Codegen, ProcCodegen};
use crate::llvm;
use rock_core::ast;
use rock_core::hir;

pub fn codegen_const_value(cg: &Codegen, value: hir::ConstValue) -> llvm::Value {
    unimplemented!();
}

pub fn codegen_expr(cg: &Codegen, proc_cg: &mut ProcCodegen, expr: &hir::Expr) -> llvm::Value {
    match *expr {
        hir::Expr::Error => todo!(),
        hir::Expr::Const { value } => todo!(),
        hir::Expr::If { if_ } => todo!(),
        hir::Expr::Block { block } => todo!(),
        hir::Expr::Match { match_ } => todo!(),
        hir::Expr::Match2 { match_ } => todo!(),
        hir::Expr::StructField {
            target,
            struct_id,
            field_id,
            deref,
        } => todo!(),
        hir::Expr::SliceField {
            target,
            field,
            deref,
        } => todo!(),
        hir::Expr::Index { target, access } => todo!(),
        hir::Expr::Slice { target, access } => todo!(),
        hir::Expr::Cast { target, into, kind } => todo!(),
        hir::Expr::LocalVar { local_id } => todo!(),
        hir::Expr::ParamVar { param_id } => todo!(),
        hir::Expr::ConstVar { const_id } => todo!(),
        hir::Expr::GlobalVar { global_id } => todo!(),
        hir::Expr::Variant {
            enum_id,
            variant_id,
            input,
        } => todo!(),
        hir::Expr::CallDirect { proc_id, input } => todo!(),
        hir::Expr::CallIndirect { target, indirect } => todo!(),
        hir::Expr::StructInit { struct_id, input } => todo!(),
        hir::Expr::ArrayInit { array_init } => todo!(),
        hir::Expr::ArrayRepeat { array_repeat } => todo!(),
        hir::Expr::Deref { rhs, ptr_ty } => todo!(),
        hir::Expr::Address { rhs } => todo!(),
        hir::Expr::Unary { op, rhs } => codegen_unary(cg, proc_cg, op, rhs),
        hir::Expr::Binary { op, lhs, rhs } => codegen_binary(cg, proc_cg, op, lhs, rhs),
    };

    unimplemented!();
}

fn codegen_unary(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen,
    op: hir::UnOp,
    rhs: &hir::Expr,
) -> llvm::Value {
    let rhs = codegen_expr(cg, proc_cg, rhs); //@value
    match op {
        hir::UnOp::Neg_Int => cg.build.neg(rhs, "un"),
        hir::UnOp::Neg_Float => cg.build.fneg(rhs, "un"),
        hir::UnOp::BitNot => cg.build.not(rhs, "un"),
        hir::UnOp::LogicNot => cg.build.not(rhs, "un"),
    }
}

fn codegen_binary(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen,
    op: hir::BinOp,
    lhs: &hir::Expr,
    rhs: &hir::Expr,
) -> llvm::Value {
    let lhs = codegen_expr(cg, proc_cg, lhs); //@value
    let rhs = codegen_expr(cg, proc_cg, rhs); //@value
    codegen_binary_op(cg, proc_cg, op, lhs, rhs)
}

fn codegen_binary_op(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen,
    op: hir::BinOp,
    lhs: llvm::Value,
    rhs: llvm::Value,
) -> llvm::Value {
    use llvm::{FloatPred, IntPred, OpCode};
    match op {
        hir::BinOp::Add_Int => cg.build.bin_op(OpCode::LLVMAdd, lhs, rhs, "bin"),
        hir::BinOp::Add_Float => cg.build.bin_op(OpCode::LLVMFAdd, lhs, rhs, "bin"),
        hir::BinOp::Sub_Int => cg.build.bin_op(OpCode::LLVMSub, lhs, rhs, "bin"),
        hir::BinOp::Sub_Float => cg.build.bin_op(OpCode::LLVMFSub, lhs, rhs, "bin"),
        hir::BinOp::Mul_Int => cg.build.bin_op(OpCode::LLVMMul, lhs, rhs, "bin"),
        hir::BinOp::Mul_Float => cg.build.bin_op(OpCode::LLVMFMul, lhs, rhs, "bin"),
        hir::BinOp::Div_IntS => cg.build.bin_op(OpCode::LLVMSDiv, lhs, rhs, "bin"),
        hir::BinOp::Div_IntU => cg.build.bin_op(OpCode::LLVMUDiv, lhs, rhs, "bin"),
        hir::BinOp::Div_Float => cg.build.bin_op(OpCode::LLVMFDiv, lhs, rhs, "bin"),
        hir::BinOp::Rem_IntS => cg.build.bin_op(OpCode::LLVMSRem, lhs, rhs, "bin"),
        hir::BinOp::Rem_IntU => cg.build.bin_op(OpCode::LLVMURem, lhs, rhs, "bin"),
        hir::BinOp::BitAnd => cg.build.bin_op(OpCode::LLVMAnd, lhs, rhs, "bin"),
        hir::BinOp::BitOr => cg.build.bin_op(OpCode::LLVMOr, lhs, rhs, "bin"),
        hir::BinOp::BitXor => cg.build.bin_op(OpCode::LLVMXor, lhs, rhs, "bin"),
        hir::BinOp::BitShl => cg.build.bin_op(OpCode::LLVMShl, lhs, rhs, "bin"),
        hir::BinOp::BitShr_IntS => cg.build.bin_op(OpCode::LLVMAShr, lhs, rhs, "bin"),
        hir::BinOp::BitShr_IntU => cg.build.bin_op(OpCode::LLVMLShr, lhs, rhs, "bin"),
        hir::BinOp::IsEq_Int => cg.build.icmp(IntPred::LLVMIntEQ, lhs, rhs, "bin"),
        hir::BinOp::IsEq_Float => cg.build.fcmp(FloatPred::LLVMRealOEQ, lhs, rhs, "bin"),
        hir::BinOp::NotEq_Int => cg.build.icmp(IntPred::LLVMIntNE, lhs, rhs, "bin"),
        hir::BinOp::NotEq_Float => cg.build.fcmp(FloatPred::LLVMRealONE, lhs, rhs, "bin"),
        hir::BinOp::Less_IntS => cg.build.icmp(IntPred::LLVMIntSLT, lhs, rhs, "bin"),
        hir::BinOp::Less_IntU => cg.build.icmp(IntPred::LLVMIntULT, lhs, rhs, "bin"),
        hir::BinOp::Less_Float => cg.build.fcmp(FloatPred::LLVMRealOLT, lhs, rhs, "bin"),
        hir::BinOp::LessEq_IntS => cg.build.icmp(IntPred::LLVMIntSLE, lhs, rhs, "bin"),
        hir::BinOp::LessEq_IntU => cg.build.icmp(IntPred::LLVMIntULE, lhs, rhs, "bin"),
        hir::BinOp::LessEq_Float => cg.build.fcmp(FloatPred::LLVMRealOLE, lhs, rhs, "bin"),
        hir::BinOp::Greater_IntS => cg.build.icmp(IntPred::LLVMIntSGT, lhs, rhs, "bin"),
        hir::BinOp::Greater_IntU => cg.build.icmp(IntPred::LLVMIntUGT, lhs, rhs, "bin"),
        hir::BinOp::Greater_Float => cg.build.fcmp(FloatPred::LLVMRealOGT, lhs, rhs, "bin"),
        hir::BinOp::GreaterEq_IntS => cg.build.icmp(IntPred::LLVMIntSGE, lhs, rhs, "bin"),
        hir::BinOp::GreaterEq_IntU => cg.build.icmp(IntPred::LLVMIntUGE, lhs, rhs, "bin"),
        hir::BinOp::GreaterEq_Float => cg.build.fcmp(FloatPred::LLVMRealOGE, lhs, rhs, "bin"),
        hir::BinOp::LogicAnd => cg.build.bin_op(llvm::OpCode::LLVMAnd, lhs, rhs, "bin"),
        hir::BinOp::LogicOr => cg.build.bin_op(llvm::OpCode::LLVMOr, lhs, rhs, "bin"),
    }
}
