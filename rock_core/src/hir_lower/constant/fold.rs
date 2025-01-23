use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::hir_lower::context::HirCtx;
use crate::support::AsStr;

pub fn fold_const_expr<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    expr: &hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    match expr.kind {
        hir::ExprKind::Error => unreachable!(),
        hir::ExprKind::Const { value } => fold_const(ctx, src, value),
        hir::ExprKind::If { .. } => unreachable!(),
        hir::ExprKind::Block { .. } => unreachable!(),
        hir::ExprKind::Match { .. } => unreachable!(),
        hir::ExprKind::StructField { target, access } => {
            fold_struct_field(ctx, src, target, &access)
        }
        hir::ExprKind::SliceField { target, access } => fold_slice_field(ctx, src, target, &access),
        hir::ExprKind::Index { target, access } => fold_index(ctx, src, target, access),
        hir::ExprKind::Slice { .. } => unreachable!(),
        hir::ExprKind::Cast { target, into, kind } => unreachable!(),
        hir::ExprKind::CallerLocation { .. } => unreachable!(),
        hir::ExprKind::ParamVar { .. } => unreachable!(),
        hir::ExprKind::Variable { .. } => unreachable!(),
        hir::ExprKind::GlobalVar { .. } => unreachable!(),
        hir::ExprKind::Variant { enum_id, variant_id, input } => {
            fold_variant(ctx, src, enum_id, variant_id, input)
        }
        hir::ExprKind::CallDirect { .. } => unreachable!(),
        hir::ExprKind::CallIndirect { .. } => unreachable!(),
        hir::ExprKind::StructInit { struct_id, input } => {
            fold_struct_init(ctx, src, struct_id, input)
        }
        hir::ExprKind::ArrayInit { array_init } => fold_array_init(ctx, src, array_init),
        hir::ExprKind::ArrayRepeat { array_repeat } => fold_array_repeat(ctx, src, array_repeat),
        hir::ExprKind::Deref { .. } => unreachable!(),
        hir::ExprKind::Address { .. } => unreachable!(),
        hir::ExprKind::Unary { op, rhs } => fold_unary_expr(ctx, src, op, rhs),
        hir::ExprKind::Binary { op, lhs, rhs } => unreachable!("bin fold"),
    }
}

fn fold_const<'hir>(
    ctx: &mut HirCtx,
    src: SourceRange,
    value: hir::ConstValue<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    match value {
        hir::ConstValue::Int { int_ty, .. } => int_range_check(ctx, src, value.into_int(), int_ty),
        hir::ConstValue::Float { val, float_ty } => float_range_check(ctx, src, val, float_ty),
        hir::ConstValue::Void
        | hir::ConstValue::Null
        | hir::ConstValue::Bool { .. }
        | hir::ConstValue::Char { .. }
        | hir::ConstValue::String { .. }
        | hir::ConstValue::Procedure { .. }
        | hir::ConstValue::Variant { .. }
        | hir::ConstValue::Struct { .. }
        | hir::ConstValue::Array { .. }
        | hir::ConstValue::ArrayRepeat { .. }
        | hir::ConstValue::ArrayEmpty { .. } => Ok(value),
    }
}

fn fold_struct_field<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    access: &hir::StructFieldAccess,
) -> Result<hir::ConstValue<'hir>, ()> {
    if access.deref.is_some() {
        unreachable!()
    }
    let target_src = SourceRange::new(src.module_id(), target.range);
    let target = fold_const_expr(ctx, target_src, target)?;

    match target {
        hir::ConstValue::Struct { struct_ } => Ok(struct_.values[access.field_id.index()]),
        _ => unreachable!(),
    }
}

fn fold_slice_field<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    access: &hir::SliceFieldAccess,
) -> Result<hir::ConstValue<'hir>, ()> {
    if access.deref.is_some() {
        unreachable!();
    }
    let target_src = SourceRange::new(src.module_id(), target.range);
    let target = fold_const_expr(ctx, target_src, target)?;

    match target {
        hir::ConstValue::String { val, string_ty } => match access.field {
            hir::SliceField::Ptr => unreachable!(),
            hir::SliceField::Len => {
                match string_ty {
                    hir::StringType::String => {
                        let string = ctx.session.intern_lit.get(val);
                        let len = string.len();
                        //@not range checked usize for 32bit?
                        Ok(hir::ConstValue::Int {
                            val: len as u64,
                            neg: false,
                            int_ty: hir::IntType::Usize,
                        })
                    }
                    hir::StringType::CString => unreachable!(),
                    hir::StringType::Untyped => unreachable!(),
                }
            }
        },
        _ => unreachable!(),
    }
}

//@check out of bounds static array access even in non constant targets (during typecheck)
fn fold_index<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    access: &hir::IndexAccess<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    if access.deref.is_some() {
        unreachable!();
    }
    let target_src = SourceRange::new(src.module_id(), target.range);
    let index_src = SourceRange::new(src.module_id(), access.index.range);
    let target = fold_const_expr(ctx, target_src, target);
    let index = fold_const_expr(ctx, index_src, access.index);

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
        hir::ConstValue::Array { array } => array.values.len() as u64,
        hir::ConstValue::ArrayRepeat { array } => array.len,
        _ => unreachable!(),
    };

    if index >= array_len {
        err::const_index_out_of_bounds(&mut ctx.emit, src, index, array_len);
        Err(())
    } else {
        let value = match target {
            hir::ConstValue::Array { array } => array.values[index as usize],
            hir::ConstValue::ArrayRepeat { array } => array.value,
            _ => unreachable!(),
        };
        Ok(value)
    }
}

/*
fn fold_cast<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    target: &hir::Expr<'hir>,
    into: hir::Type,
    kind: hir::CastKind,
) -> Result<hir::ConstValue<'hir>, ()> {
    fn into_int_ty(into: hir::Type) -> hir::IntType {
        match into {
            hir::Type::Int(int_ty) => int_ty,
            _ => unreachable!(),
        }
    }

    fn into_float_ty(into: hir::Type) -> hir::FloatType {
        match into {
            hir::Type::Float(float_ty) => float_ty,
            _ => unreachable!(),
        }
    }

    let target_src = SourceRange::new(src.module_id(), target.range);
    let mut target = fold_const_expr(ctx, target_src, target)?;

    if let hir::ConstValue::Variant { variant } = target {
        assert!(variant.values.is_empty()); //@why assert? document
        let enum_data = ctx.registry.enum_data(variant.enum_id);
        let variant = enum_data.variant(variant.variant_id);

        // extract variant tag if available
        target = match variant.kind {
            hir::VariantKind::Default(id) => {
                let eval = ctx.registry.variant_eval(id);
                eval.resolved()?
            }
            hir::VariantKind::Constant(id) => {
                let (eval, _) = ctx.registry.const_eval(id);
                eval.resolved()?
            }
        };
    }

    match kind {
        hir::CastKind::Error => unreachable!(),
        hir::CastKind::NoOp => Ok(target),
        hir::CastKind::NoOpUnchecked => Ok(target),
        hir::CastKind::Int_Trunc
        | hir::CastKind::IntS_Sign_Extend
        | hir::CastKind::IntU_Zero_Extend => {
            let val = target.into_int();
            let int_ty = into_int_ty(into);
            int_range_check(ctx, src, val, int_ty)
        }
        hir::CastKind::IntS_to_Float | hir::CastKind::IntU_to_Float => {
            let val = target.into_int();
            let float_ty = into_float_ty(into);
            float_range_check(ctx, src, val as f64, float_ty)
        }
        hir::CastKind::Float_to_IntS | hir::CastKind::Float_to_IntU => {
            let val = target.into_float();
            let int_ty = into_int_ty(into);
            int_range_check(ctx, src, val as i128, int_ty)
        }
        hir::CastKind::Float_Trunc | hir::CastKind::Float_Extend => {
            let val = target.into_float();
            let float_ty = into_float_ty(into);
            float_range_check(ctx, src, val, float_ty)
        }
        hir::CastKind::Bool_to_Int => {
            let val = target.into_bool();
            let int_ty = into_int_ty(into);
            int_range_check(ctx, src, val as i128, int_ty)
        }
        hir::CastKind::Bool_to_Bool32 => {
            let val = target.into_bool();
            Ok(hir::ConstValue::Bool { val, bool_ty: hir::BoolType::Bool32 })
        }
        hir::CastKind::Bool32_to_Bool => {
            let val = target.into_bool();
            Ok(hir::ConstValue::Bool { val, bool_ty: hir::BoolType::Bool })
        }
        hir::CastKind::Char_to_U32 => {
            let val = target.into_char();
            let int_ty = into_int_ty(into);
            int_range_check(ctx, src, val as i128, int_ty)
        }
    }
}
*/

fn fold_const_var<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    const_id: hir::ConstID,
) -> Result<hir::ConstValue<'hir>, ()> {
    let data = ctx.registry.const_data(const_id);
    let (eval, _) = ctx.registry.const_eval(data.value);
    eval.resolved()
}

fn fold_variant<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    enum_id: hir::EnumID,
    variant_id: hir::VariantID,
    input: &&[&hir::Expr<'hir>],
) -> Result<hir::ConstValue<'hir>, ()> {
    let mut correct = true;
    let mut values = Vec::with_capacity(input.len());

    for &expr in input.iter() {
        let src = SourceRange::new(src.module_id(), expr.range);
        if let Ok(value) = fold_const_expr(ctx, src, expr) {
            values.push(value);
        } else {
            correct = false;
        }
    }

    if correct {
        let values = ctx.arena.alloc_slice(&values);
        let variant = hir::ConstVariant { enum_id, variant_id, values };
        let variant = ctx.arena.alloc(variant);
        Ok(hir::ConstValue::Variant { variant })
    } else {
        Err(())
    }
}

fn fold_struct_init<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    struct_id: hir::StructID,
    input: &[hir::FieldInit<'hir>],
) -> Result<hir::ConstValue<'hir>, ()> {
    let mut correct = true;
    let mut values = Vec::new();
    values.resize(input.len(), hir::ConstValue::Null); //dummy value

    for init in input {
        let src = SourceRange::new(src.module_id(), init.expr.range);
        if let Ok(value) = fold_const_expr(ctx, src, init.expr) {
            values[init.field_id.index()] = value;
        } else {
            correct = false;
        }
    }

    if correct {
        let values = ctx.arena.alloc_slice(&values);
        let struct_ = hir::ConstStruct { struct_id, values };
        let struct_ = ctx.arena.alloc(struct_);
        Ok(hir::ConstValue::Struct { struct_ })
    } else {
        Err(())
    }
}

fn fold_array_init<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    array_init: &hir::ArrayInit<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let mut correct = true;
    let mut values = Vec::with_capacity(array_init.input.len());

    for &expr in array_init.input {
        let src = SourceRange::new(src.module_id(), expr.range);
        if let Ok(value) = fold_const_expr(ctx, src, expr) {
            values.push(value);
        } else {
            correct = false;
        }
    }

    if correct {
        let values = ctx.arena.alloc_slice(values.as_slice());
        let array = hir::ConstArray { values };
        let array = ctx.arena.alloc(array);
        Ok(hir::ConstValue::Array { array })
    } else {
        Err(())
    }
}

fn fold_array_repeat<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    array_repeat: &hir::ArrayRepeat<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let src = SourceRange::new(src.module_id(), array_repeat.value.range);
    let value = fold_const_expr(ctx, src, array_repeat.value)?;
    let len = array_repeat.len;

    let array = hir::ConstArrayRepeat { value, len };
    let array = ctx.arena.alloc(array);
    Ok(hir::ConstValue::ArrayRepeat { array })
}

fn fold_unary_expr<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    src: SourceRange,
    op: hir::UnOp,
    rhs: &'hir hir::Expr<'hir>,
) -> Result<hir::ConstValue<'hir>, ()> {
    let rhs_src = SourceRange::new(src.module_id(), rhs.range);
    let rhs = fold_const_expr(ctx, rhs_src, rhs)?;

    match op {
        hir::UnOp::Neg_Int => {
            let int_ty = rhs.into_int_ty();
            let val = -rhs.into_int();
            int_range_check(ctx, src, val, int_ty)
        }
        hir::UnOp::Neg_Float => {
            let float_ty = rhs.into_float_ty();
            let val = -rhs.into_float();
            float_range_check(ctx, src, val, float_ty)
        }
        hir::UnOp::BitNot => {
            err::internal_not_implemented(&mut ctx.emit, src, "unary `~` constant folding");
            Err(())
        }
        hir::UnOp::LogicNot => {
            let val = !rhs.into_bool();
            Ok(hir::ConstValue::Bool { val, bool_ty: hir::BoolType::Bool })
        }
    }
}

pub fn int_range_check<'hir>(
    ctx: &mut HirCtx,
    src: SourceRange,
    val: i128,
    int_ty: hir::IntType,
) -> Result<hir::ConstValue<'hir>, ()> {
    let ptr_width = ctx.session.config.target_ptr_width;
    let min = int_ty.min_128(ptr_width);
    let max = int_ty.max_128(ptr_width);

    if val < min || val > max {
        let int_ty = int_ty.as_str();
        err::const_int_out_of_range(&mut ctx.emit, src, int_ty, val, min, max);
        Err(())
    } else if val >= 0 {
        let val: u64 = val.try_into().unwrap();
        Ok(hir::ConstValue::Int { val, neg: false, int_ty })
    } else {
        let val: u64 = (-val).try_into().unwrap();
        Ok(hir::ConstValue::Int { val, neg: true, int_ty })
    }
}

pub fn float_range_check<'hir>(
    ctx: &mut HirCtx,
    src: SourceRange,
    val: f64,
    float_ty: hir::FloatType,
) -> Result<hir::ConstValue<'hir>, ()> {
    let (min, max) = match float_ty {
        hir::FloatType::F32 => (f32::MIN as f64, f32::MAX as f64),
        hir::FloatType::F64 => (f64::MIN, f64::MAX),
        hir::FloatType::Untyped => unreachable!(),
    };

    if val.is_nan() {
        err::const_float_is_nan(&mut ctx.emit, src);
        Err(())
    } else if val.is_infinite() {
        err::const_float_is_infinite(&mut ctx.emit, src);
        Err(())
    } else if val < min || val > max {
        let float_ty = float_ty.as_str();
        err::const_float_out_of_range(&mut ctx.emit, src, float_ty, val, min, max);
        Err(())
    } else {
        Ok(hir::ConstValue::Float { val, float_ty })
    }
}

impl<'hir> hir::ConstValue<'hir> {
    pub fn from_u64(val: u64, int_ty: hir::IntType) -> hir::ConstValue<'hir> {
        hir::ConstValue::Int { val, neg: false, int_ty }
    }
    pub fn from_i64(val: i64, int_ty: hir::IntType) -> hir::ConstValue<'hir> {
        if val < 0 {
            hir::ConstValue::Int { val: -val as u64, neg: true, int_ty }
        } else {
            hir::ConstValue::Int { val: val as u64, neg: false, int_ty }
        }
    }

    pub fn expect_int(self) -> (u64, bool, hir::IntType) {
        match self {
            hir::ConstValue::Int { val, neg, int_ty } => (val, neg, int_ty),
            _ => unreachable!(),
        }
    }
    pub fn expect_float(self) -> (f64, hir::FloatType) {
        match self {
            hir::ConstValue::Float { val, float_ty } => (val, float_ty),
            _ => unreachable!(),
        }
    }
    pub fn expect_bool(self) -> (bool, hir::BoolType) {
        match self {
            hir::ConstValue::Bool { val, bool_ty } => (val, bool_ty),
            _ => unreachable!(),
        }
    }

    pub fn into_bool(&self) -> bool {
        match *self {
            hir::ConstValue::Bool { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_char(&self) -> char {
        match *self {
            hir::ConstValue::Char { val } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_int_u64(&self) -> u64 {
        match *self {
            hir::ConstValue::Int { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_int_i64(&self) -> i64 {
        match *self {
            hir::ConstValue::Int { val, neg, .. } => {
                if neg {
                    -(val as i64)
                } else {
                    val as i64
                }
            }
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
    pub fn into_int_ty(&self) -> hir::IntType {
        match *self {
            hir::ConstValue::Int { int_ty, .. } => int_ty,
            _ => unreachable!(),
        }
    }
    pub fn into_float(&self) -> f64 {
        match *self {
            hir::ConstValue::Float { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_float_ty(&self) -> hir::FloatType {
        match *self {
            hir::ConstValue::Float { float_ty, .. } => float_ty,
            _ => unreachable!(),
        }
    }
    pub fn into_enum(&self) -> &hir::ConstVariant {
        match *self {
            hir::ConstValue::Variant { variant } => variant,
            _ => unreachable!(),
        }
    }
}
