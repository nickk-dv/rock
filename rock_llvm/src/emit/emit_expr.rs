use super::context::{Codegen, ProcCodegen};
use super::emit_stmt;
use crate::llvm;
use rock_core::ast;
use rock_core::hir;

pub fn codegen_expr_value<'c>(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
) -> llvm::Value {
    codegen_expr(cg, proc_cg, expr).unwrap()
}

fn codegen_expr<'c>(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen<'c>,
    expr: &hir::Expr<'c>,
) -> Option<llvm::Value> {
    match *expr {
        hir::Expr::Error => unreachable!(),
        hir::Expr::Const { value } => Some(codegen_const(cg, value)),
        hir::Expr::If { if_ } => todo!(),
        hir::Expr::Block { block } => {
            emit_stmt::codegen_block(cg, proc_cg, block);
            None
        }
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
        hir::Expr::Unary { op, rhs } => Some(codegen_unary(cg, proc_cg, op, rhs)),
        hir::Expr::Binary { op, lhs, rhs } => Some(codegen_binary(cg, proc_cg, op, lhs, rhs)),
    };

    unimplemented!();
}

pub fn codegen_const(cg: &Codegen, value: hir::ConstValue) -> llvm::Value {
    match value {
        hir::ConstValue::Error => unreachable!(),
        hir::ConstValue::Null => llvm::const_all_zero(cg.ptr_type()),
        hir::ConstValue::Bool { val } => llvm::const_int(cg.bool_type(), val as u64, false),
        hir::ConstValue::Int { val, int_ty, .. } => {
            llvm::const_int(cg.basic_type(int_ty.into_basic()), val, int_ty.is_signed())
        }
        hir::ConstValue::IntS(_) => unimplemented!(),
        hir::ConstValue::IntU(_) => unimplemented!(),
        hir::ConstValue::Float { val, float_ty } => {
            llvm::const_float(cg.basic_type(float_ty.into_basic()), val)
        }
        hir::ConstValue::Char { val } => {
            llvm::const_int(cg.basic_type(ast::BasicType::U32), val as u64, false)
        }
        hir::ConstValue::String { id, c_string } => todo!(),
        hir::ConstValue::Procedure { proc_id } => cg.procs[proc_id.index()].into(),
        hir::ConstValue::EnumVariant { enum_ } => todo!(),
        hir::ConstValue::Struct { struct_ } => todo!(),
        hir::ConstValue::Array { array } => todo!(),
        hir::ConstValue::ArrayRepeat { value, len } => todo!(),
    }
}

fn codegen_unary<'c>(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen<'c>,
    op: hir::UnOp,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let rhs = codegen_expr_value(cg, proc_cg, rhs);
    match op {
        hir::UnOp::Neg_Int => cg.build.neg(rhs, "un"),
        hir::UnOp::Neg_Float => cg.build.fneg(rhs, "un"),
        hir::UnOp::BitNot => cg.build.not(rhs, "un"),
        hir::UnOp::LogicNot => cg.build.not(rhs, "un"),
    }
}

fn codegen_binary<'c>(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen<'c>,
    op: hir::BinOp,
    lhs: &hir::Expr<'c>,
    rhs: &hir::Expr<'c>,
) -> llvm::Value {
    let lhs = codegen_expr_value(cg, proc_cg, lhs);
    let rhs = codegen_expr_value(cg, proc_cg, rhs);
    codegen_binary_op(cg, op, lhs, rhs)
}

fn codegen_binary_op(
    cg: &Codegen,
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
