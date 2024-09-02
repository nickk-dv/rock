use crate::config::TargetPtrWidth;
use crate::error::SourceRange;
use crate::hir;
use crate::hir_lower::errors as err;
use crate::hir_lower::hir_build::{HirData, HirEmit};

pub fn fold_const_expr<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    expr: &hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    match expr.kind {
        hir::ExprKind::Error => unreachable!(),
        hir::ExprKind::Const { value } => fold_const(hir, emit, src, value),
        hir::ExprKind::If { .. } => unreachable!(),
        hir::ExprKind::Block { .. } => unreachable!(),
        hir::ExprKind::Match { .. } => unreachable!(),
        hir::ExprKind::Match2 { .. } => unreachable!(),
        hir::ExprKind::StructField {
            target,
            field_id,
            deref,
            ..
        } => fold_struct_field(hir, emit, src, target, field_id, deref),
        hir::ExprKind::SliceField {
            target,
            field,
            deref,
        } => fold_slice_field(hir, emit, src, target, field, deref),
        hir::ExprKind::Index { target, access } => fold_index(hir, emit, src, target, access),
        hir::ExprKind::Slice { .. } => unreachable!(),
        hir::ExprKind::Cast { target, into, kind } => {
            fold_cast(hir, emit, src, target, *into, kind)
        }
        hir::ExprKind::LocalVar { .. } => unreachable!(),
        hir::ExprKind::ParamVar { .. } => unreachable!(),
        hir::ExprKind::ConstVar { const_id } => fold_const_var(hir, emit, const_id),
        hir::ExprKind::GlobalVar { .. } => unreachable!(),
        hir::ExprKind::Variant { .. } => unimplemented!("fold enum variant"),
        hir::ExprKind::CallDirect { .. } => unreachable!(),
        hir::ExprKind::CallIndirect { .. } => unreachable!(),
        hir::ExprKind::StructInit { struct_id, input } => {
            fold_struct_init(hir, emit, src, struct_id, input)
        }
        hir::ExprKind::ArrayInit { array_init } => fold_array_init(hir, emit, src, array_init),
        hir::ExprKind::ArrayRepeat { array_repeat } => {
            fold_array_repeat(hir, emit, src, array_repeat)
        }
        hir::ExprKind::Deref { .. } => unreachable!(),
        hir::ExprKind::Address { .. } => unreachable!(),
        hir::ExprKind::Unary { op, rhs } => fold_unary_expr(hir, emit, src, op, rhs),
        hir::ExprKind::Binary { op, lhs, rhs } => fold_binary(hir, emit, src, op, lhs, rhs),
    }
}

fn fold_const<'hir>(
    hir: &HirData,
    emit: &mut HirEmit,
    src: SourceRange,
    value: hir::ConstValue<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    match value {
        hir::ConstValue::Int { val, int_ty, .. } => {
            int_range_check(hir, emit, src, val.into(), int_ty)
        }
        hir::ConstValue::Float { val, float_ty } => float_range_check(emit, src, val, float_ty),
        hir::ConstValue::Null
        | hir::ConstValue::Bool { .. }
        | hir::ConstValue::Char { .. }
        | hir::ConstValue::String { .. }
        | hir::ConstValue::Procedure { .. }
        | hir::ConstValue::Variant { .. }
        | hir::ConstValue::Struct { .. }
        | hir::ConstValue::Array { .. }
        | hir::ConstValue::ArrayRepeat { .. } => Ok(value),
    }
}

fn fold_struct_field<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    field_id: hir::FieldID,
    deref: bool,
) -> Result<hir::ConstValue<'hir>, ()> {
    if deref {
        unreachable!()
    }
    let target_src = SourceRange::new(src.module_id(), target.range);
    let target = fold_const_expr(hir, emit, target_src, target)?;

    match target {
        hir::ConstValue::Struct { struct_ } => {
            let value_id = struct_.value_ids[field_id.index()];
            Ok(emit.const_intern.get(value_id))
        }
        _ => unreachable!(),
    }
}

fn fold_slice_field<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    field: hir::SliceField,
    deref: bool,
) -> Result<hir::ConstValue<'hir>, ()> {
    if deref {
        unreachable!();
    }
    let target_src = SourceRange::new(src.module_id(), target.range);
    let target = fold_const_expr(hir, emit, target_src, target)?;

    match target {
        hir::ConstValue::String { id, c_string } => match field {
            hir::SliceField::Ptr => unreachable!(),
            hir::SliceField::Len => {
                if !c_string {
                    let string = hir.intern_string().get_str(id);
                    let len = string.len();
                    Ok(hir::ConstValue::Int {
                        val: len as u64,
                        neg: false,
                        int_ty: hir::BasicInt::Usize,
                    })
                } else {
                    unreachable!()
                }
            }
        },
        _ => unreachable!(),
    }
}

//@check out of bounds static array access even in non constant targets (during typecheck)
fn fold_index<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    access: &hir::IndexAccess<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    if access.deref {
        unreachable!();
    }
    let target_src = SourceRange::new(src.module_id(), target.range);
    let index_src = SourceRange::new(src.module_id(), access.index.range);
    let target = fold_const_expr(hir, emit, target_src, target);
    let index = fold_const_expr(hir, emit, index_src, access.index);

    let target = target?;
    let index = index?;

    let index = match index {
        hir::ConstValue::Int { val, neg, int_ty } => {
            assert!(!neg);
            assert!(!int_ty.is_signed());
            val
        }
        _ => unreachable!(),
    };

    let array_len = match target {
        hir::ConstValue::Array { array } => array.len,
        hir::ConstValue::ArrayRepeat { len, .. } => len,
        _ => unreachable!(),
    };

    if index >= array_len {
        err::const_index_out_of_bounds(emit, src, index, array_len);
        Err(())
    } else {
        let value_id = match target {
            hir::ConstValue::Array { array } => array.value_ids[index as usize],
            hir::ConstValue::ArrayRepeat { value, .. } => value,
            _ => unreachable!(),
        };
        Ok(emit.const_intern.get(value_id))
    }
}

//@store type enums like BasicInt / BasicFloat
// in cast kind to decrease invariance
//@check how bool to int is handled
fn fold_cast<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    into: hir::Type,
    kind: hir::CastKind,
) -> Result<hir::ConstValue<'hir>, ()> {
    fn into_int_ty(into: hir::Type) -> hir::BasicInt {
        match into {
            hir::Type::Basic(basic) => hir::BasicInt::from_basic(basic).unwrap(),
            _ => unreachable!(),
        }
    }

    fn into_float_ty(into: hir::Type) -> hir::BasicFloat {
        match into {
            hir::Type::Basic(basic) => hir::BasicFloat::from_basic(basic).unwrap(),
            _ => unreachable!(),
        }
    }

    let target_src = SourceRange::new(src.module_id(), target.range);
    let target = fold_const_expr(hir, emit, target_src, target)?;

    match kind {
        hir::CastKind::Error => unreachable!(),
        hir::CastKind::NoOp => Ok(target),
        hir::CastKind::Int_Trunc
        | hir::CastKind::IntS_Sign_Extend
        | hir::CastKind::IntU_Zero_Extend => {
            let val = target.into_int();
            let int_ty = into_int_ty(into);
            int_range_check(hir, emit, src, val, int_ty)
        }
        hir::CastKind::IntS_to_Float | hir::CastKind::IntU_to_Float => {
            let val = target.into_int();
            let float_ty = into_float_ty(into);
            let val_cast = val as f64;
            float_range_check(emit, src, val_cast, float_ty)
        }
        hir::CastKind::Float_to_IntS | hir::CastKind::Float_to_IntU => {
            let val = target.into_float();
            let int_ty = into_int_ty(into);
            let val_cast = val as i128;
            int_range_check(hir, emit, src, val_cast, int_ty)
        }
        hir::CastKind::Float_Trunc | hir::CastKind::Float_Extend => {
            let val = target.into_float();
            let float_ty = into_float_ty(into);
            float_range_check(emit, src, val, float_ty)
        }
    }
}

fn fold_const_var<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    const_id: hir::ConstID,
) -> Result<hir::ConstValue<'hir>, ()> {
    let data = hir.registry().const_data(const_id);
    let (eval, _) = hir.registry().const_eval(data.value);

    let value_id = eval.get_resolved()?;
    let value = emit.const_intern.get(value_id);
    Ok(value)
}

fn fold_struct_init<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    struct_id: hir::StructID,
    input: &[hir::FieldInit<'hir>],
) -> Result<hir::ConstValue<'hir>, ()> {
    let mut correct = true;
    let mut value_ids = Vec::new();
    value_ids.resize(input.len(), hir::ConstValueID::dummy());

    for init in input {
        let src = SourceRange::new(src.module_id(), init.expr.range);
        if let Ok(value) = fold_const_expr(hir, emit, src, init.expr) {
            value_ids[init.field_id.index()] = emit.const_intern.intern(value);
        } else {
            correct = false;
        }
    }

    if correct {
        let value_ids = emit.const_intern.arena().alloc_slice(&value_ids);
        let const_struct = hir::ConstStruct {
            struct_id,
            value_ids,
        };
        let struct_ = emit.const_intern.arena().alloc(const_struct);
        Ok(hir::ConstValue::Struct { struct_ })
    } else {
        Err(())
    }
}

fn fold_array_init<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    array_init: &hir::ArrayInit<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let mut correct = true;
    let mut value_ids = Vec::with_capacity(array_init.input.len());

    for &expr in array_init.input {
        let src = SourceRange::new(src.module_id(), expr.range);
        if let Ok(value) = fold_const_expr(hir, emit, src, expr) {
            value_ids.push(emit.const_intern.intern(value));
        } else {
            correct = false;
        }
    }

    if correct {
        let len = value_ids.len() as u64;
        let value_ids = emit.const_intern.arena().alloc_slice(value_ids.as_slice());
        let const_array = hir::ConstArray { len, value_ids };
        let array = emit.const_intern.arena().alloc(const_array);
        Ok(hir::ConstValue::Array { array })
    } else {
        Err(())
    }
}

fn fold_array_repeat<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    array_repeat: &hir::ArrayRepeat<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let src = SourceRange::new(src.module_id(), array_repeat.expr.range);
    let value = fold_const_expr(hir, emit, src, array_repeat.expr)?;

    Ok(hir::ConstValue::ArrayRepeat {
        value: emit.const_intern.intern(value),
        len: array_repeat.len,
    })
}

fn fold_unary_expr<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    op: hir::UnOp,
    rhs: &'hir hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let rhs_src = SourceRange::new(src.module_id(), rhs.range);
    let rhs = fold_const_expr(hir, emit, rhs_src, rhs)?;

    match op {
        hir::UnOp::Neg_Int => {
            let int_ty = rhs.into_int_ty();
            let val = -rhs.into_int();
            int_range_check(hir, emit, src, val, int_ty)
        }
        hir::UnOp::Neg_Float => {
            let float_ty = rhs.into_float_ty();
            let val = rhs.into_float();
            float_range_check(emit, src, val, float_ty)
        }
        hir::UnOp::BitNot => unimplemented!(),
        hir::UnOp::LogicNot => {
            let val = !rhs.into_bool();
            Ok(hir::ConstValue::Bool { val })
        }
    }
}

fn fold_binary<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    src: SourceRange,
    op: hir::BinOp,
    lhs: &hir::Expr<'hir>,
    rhs: &hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let lhs_src = SourceRange::new(src.module_id(), lhs.range);
    let rhs_src = SourceRange::new(src.module_id(), rhs.range);
    let lhs = fold_const_expr(hir, emit, lhs_src, lhs);
    let rhs = fold_const_expr(hir, emit, rhs_src, rhs);

    let lhs = lhs?;
    let rhs = rhs?;

    match op {
        hir::BinOp::Add_Int => {
            let int_ty = lhs.into_int_ty();
            let val = lhs.into_int() + rhs.into_int();
            int_range_check(hir, emit, src, val, int_ty)
        }
        hir::BinOp::Add_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() + rhs.into_float();
            float_range_check(emit, src, val, float_ty)
        }
        hir::BinOp::Sub_Int => {
            let int_ty = lhs.into_int_ty();
            let val = lhs.into_int() - rhs.into_int();
            int_range_check(hir, emit, src, val, int_ty)
        }
        hir::BinOp::Sub_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() - rhs.into_float();
            float_range_check(emit, src, val, float_ty)
        }
        hir::BinOp::Mul_Int => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if let Some(val) = lhs.checked_mul(rhs) {
                int_range_check(hir, emit, src, val, int_ty)
            } else {
                err::const_int_overflow(emit, src, op, lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Mul_Float => {
            let float_ty = lhs.into_float_ty();
            let val = lhs.into_float() * rhs.into_float();
            float_range_check(emit, src, val, float_ty)
        }
        hir::BinOp::Div_IntS | hir::BinOp::Div_IntU => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if rhs == 0 {
                err::const_int_div_by_zero(emit, src, op, lhs, rhs);
                Err(())
            } else if let Some(val) = lhs.checked_div(rhs) {
                int_range_check(hir, emit, src, val, int_ty)
            } else {
                err::const_int_overflow(emit, src, op, lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::Div_Float => {
            let float_ty = lhs.into_float_ty();
            let lhs = lhs.into_float();
            let rhs = rhs.into_float();

            if rhs == 0.0 {
                err::const_float_div_by_zero(emit, src, op, lhs, rhs);
                Err(())
            } else {
                let val = lhs / rhs;
                float_range_check(emit, src, val, float_ty)
            }
        }
        hir::BinOp::Rem_IntS | hir::BinOp::Rem_IntU => {
            let int_ty = lhs.into_int_ty();
            let lhs = lhs.into_int();
            let rhs = rhs.into_int();

            if rhs == 0 {
                err::const_int_div_by_zero(emit, src, op, lhs, rhs);
                Err(())
            } else if let Some(val) = lhs.checked_rem(rhs) {
                int_range_check(hir, emit, src, val, int_ty)
            } else {
                err::const_int_overflow(emit, src, op, lhs, rhs);
                Err(())
            }
        }
        hir::BinOp::BitAnd => unimplemented!(),
        hir::BinOp::BitOr => unimplemented!(),
        hir::BinOp::BitXor => unimplemented!(),
        hir::BinOp::BitShl => unimplemented!(),
        hir::BinOp::BitShr_IntS => unimplemented!(),
        hir::BinOp::BitShr_IntU => unimplemented!(),
        hir::BinOp::IsEq_Int => {
            let val = lhs.into_int() == rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::IsEq_Float => {
            let val = lhs.into_float() == rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::NotEq_Int => {
            let val = lhs.into_int() != rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::NotEq_Float => {
            let val = lhs.into_float() != rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Less_IntS | hir::BinOp::Less_IntU => {
            let val = lhs.into_int() < rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Less_Float => {
            let val = lhs.into_float() < rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LessEq_IntS | hir::BinOp::LessEq_IntU => {
            let val = lhs.into_int() <= rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LessEq_Float => {
            let val = lhs.into_float() <= rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Greater_IntS | hir::BinOp::Greater_IntU => {
            let val = lhs.into_int() > rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::Greater_Float => {
            let val = lhs.into_float() > rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::GreaterEq_IntS | hir::BinOp::GreaterEq_IntU => {
            let val = lhs.into_int() >= rhs.into_int();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::GreaterEq_Float => {
            let val = lhs.into_float() >= rhs.into_float();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LogicAnd => {
            let val = lhs.into_bool() && rhs.into_bool();
            Ok(hir::ConstValue::Bool { val })
        }
        hir::BinOp::LogicOr => {
            let val = lhs.into_bool() || rhs.into_bool();
            Ok(hir::ConstValue::Bool { val })
        }
    }
}

pub fn int_range_check<'hir>(
    hir: &HirData,
    emit: &mut HirEmit,
    src: SourceRange,
    val: i128,
    int_ty: hir::BasicInt,
) -> Result<hir::ConstValue<'hir>, ()> {
    let (min, max) = match int_ty {
        hir::BasicInt::S8 => (i8::MIN as i128, i8::MAX as i128),
        hir::BasicInt::S16 => (i16::MIN as i128, i16::MAX as i128),
        hir::BasicInt::S32 => (i32::MIN as i128, i32::MAX as i128),
        hir::BasicInt::S64 => (i64::MIN as i128, i64::MAX as i128),
        hir::BasicInt::Ssize => {
            let ptr_width = hir.target().arch().ptr_width();
            match ptr_width {
                TargetPtrWidth::Bit_32 => (i32::MIN as i128, i32::MAX as i128),
                TargetPtrWidth::Bit_64 => (i64::MIN as i128, i64::MAX as i128),
            }
        }
        hir::BasicInt::U8 => (u8::MIN as i128, u8::MAX as i128),
        hir::BasicInt::U16 => (u16::MIN as i128, u16::MAX as i128),
        hir::BasicInt::U32 => (u32::MIN as i128, u32::MAX as i128),
        hir::BasicInt::U64 => (u64::MIN as i128, u64::MAX as i128),
        hir::BasicInt::Usize => {
            let ptr_width = hir.target().arch().ptr_width();
            match ptr_width {
                TargetPtrWidth::Bit_32 => (u32::MIN as i128, u32::MAX as i128),
                TargetPtrWidth::Bit_64 => (u64::MIN as i128, u64::MAX as i128),
            }
        }
    };

    if val < min || val > max {
        err::const_int_out_of_range(emit, src, int_ty, val, min, max);
        Err(())
    } else {
        if val > 0 {
            let val: u64 = val.try_into().unwrap();
            let neg = false;
            Ok(hir::ConstValue::Int { val, neg, int_ty })
        } else {
            let val: u64 = (-val).try_into().unwrap();
            let neg = true;
            Ok(hir::ConstValue::Int { val, neg, int_ty })
        }
    }
}

fn float_range_check<'hir>(
    emit: &mut HirEmit,
    src: SourceRange,
    val: f64,
    float_ty: hir::BasicFloat,
) -> Result<hir::ConstValue<'hir>, ()> {
    let (min, max) = match float_ty {
        hir::BasicFloat::F32 => (f32::MIN as f64, f32::MAX as f64),
        hir::BasicFloat::F64 => (f64::MIN as f64, f64::MAX as f64),
    };

    if val.is_nan() {
        err::const_float_is_nan(emit, src);
        Err(())
    } else if val.is_infinite() {
        err::const_float_is_infinite(emit, src);
        Err(())
    } else if val < min || val > max {
        err::const_float_out_of_range(emit, src, float_ty, val, min, max);
        Err(())
    } else {
        Ok(hir::ConstValue::Float { val, float_ty })
    }
}

impl<'hir> hir::ConstValue<'hir> {
    fn into_bool(&self) -> bool {
        match *self {
            hir::ConstValue::Bool { val } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_int(&self) -> i128 {
        match *self {
            hir::ConstValue::Int { val, neg, .. } => {
                if neg {
                    -(val as i128)
                } else {
                    val as i128
                }
            }
            _ => unreachable!(),
        }
    }
    fn into_int_ty(&self) -> hir::BasicInt {
        match *self {
            hir::ConstValue::Int { int_ty, .. } => int_ty,
            _ => unreachable!(),
        }
    }
    fn into_float(&self) -> f64 {
        match *self {
            hir::ConstValue::Float { val, .. } => val,
            _ => unreachable!(),
        }
    }
    fn into_float_ty(&self) -> hir::BasicFloat {
        match *self {
            hir::ConstValue::Float { float_ty, .. } => float_ty,
            _ => unreachable!(),
        }
    }
}
