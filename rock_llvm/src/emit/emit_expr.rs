use super::context::{Codegen, ProcCodegen};
use super::emit_stmt;
use crate::llvm;
use rock_core::ast;
use rock_core::hir;
use rock_core::intern::InternID;

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
        hir::ConstValue::Null => codegen_const_null(cg),
        hir::ConstValue::Bool { val } => codegen_const_bool(cg, val),
        hir::ConstValue::Int { val, int_ty, .. } => codegen_const_int(cg, val, int_ty),
        hir::ConstValue::IntS(_) => unimplemented!(),
        hir::ConstValue::IntU(_) => unimplemented!(),
        hir::ConstValue::Float { val, float_ty } => codegen_const_float(cg, val, float_ty),
        hir::ConstValue::Char { val } => codegen_const_char(cg, val),
        hir::ConstValue::String { id, c_string } => codegen_const_string(cg, id, c_string),
        hir::ConstValue::Procedure { proc_id } => cg.procs[proc_id.index()].into(),
        hir::ConstValue::Variant { variant } => codegen_const_variant(cg, variant),
        hir::ConstValue::Struct { struct_ } => codegen_const_struct(cg, struct_),
        hir::ConstValue::Array { array } => codegen_const_array(cg, array),
        hir::ConstValue::ArrayRepeat { value, len } => codegen_const_array_repeat(cg, value, len),
    }
}

#[inline]
fn codegen_const_null(cg: &Codegen) -> llvm::Value {
    llvm::const_all_zero(cg.ptr_type())
}

#[inline]
fn codegen_const_bool(cg: &Codegen, val: bool) -> llvm::Value {
    llvm::const_int(cg.bool_type(), val as u64, false)
}

#[inline]
fn codegen_const_int(cg: &Codegen, val: u64, int_ty: hir::BasicInt) -> llvm::Value {
    llvm::const_int(cg.basic_type(int_ty.into_basic()), val, int_ty.is_signed())
}

#[inline]
fn codegen_const_float(cg: &Codegen, val: f64, float_ty: hir::BasicFloat) -> llvm::Value {
    llvm::const_float(cg.basic_type(float_ty.into_basic()), val)
}

#[inline]
fn codegen_const_char(cg: &Codegen, val: char) -> llvm::Value {
    llvm::const_int(cg.basic_type(ast::BasicType::U32), val as u64, false)
}

fn codegen_const_string(cg: &Codegen, id: InternID, c_string: bool) -> llvm::Value {
    let global_ptr = cg.string_lits[id.index()];

    if c_string {
        global_ptr
    } else {
        let string = cg.hir.intern_string.get_str(id);
        let bytes_len = string.len() as u64;
        let slice_len = llvm::const_int(cg.ptr_sized_int(), bytes_len, false);
        let slice_val = llvm::const_struct_inline(&[global_ptr, slice_len], false);
        slice_val
    }
}

fn codegen_const_variant(cg: &Codegen, variant: &hir::ConstVariant) -> llvm::Value {
    unimplemented!()
}

fn codegen_const_struct(cg: &Codegen, struct_: &hir::ConstStruct) -> llvm::Value {
    let mut values = Vec::with_capacity(struct_.fields.len());

    for &value_id in struct_.fields {
        let value = codegen_const(cg, cg.hir.const_value(value_id));
        values.push(value);
    }

    let struct_ty = cg.struct_type(struct_.struct_id);
    llvm::const_struct_named(struct_ty, &values)
}

fn codegen_const_array(cg: &Codegen, array: &hir::ConstArray) -> llvm::Value {
    let mut values = Vec::with_capacity(array.len as usize);

    for &value_id in array.values {
        let value = codegen_const(cg, cg.hir.const_value(value_id));
        values.push(value);
    }

    //@type of zero sized arrays is not stored, will cras
    let elem_ty = llvm::typeof_value(values[0]);
    llvm::const_array(elem_ty, &values)
}

fn codegen_const_array_repeat(cg: &Codegen, value_id: hir::ConstValueID, len: u64) -> llvm::Value {
    let mut values = Vec::with_capacity(len as usize);
    let value = codegen_const(cg, cg.hir.const_value(value_id));
    values.resize(len as usize, value);

    //@zero sized array repeat is counter intuitive
    let elem_ty = llvm::typeof_value(values[0]);
    llvm::const_array(elem_ty, &values)
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
