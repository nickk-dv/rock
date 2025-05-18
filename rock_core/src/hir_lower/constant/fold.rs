use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::hir_lower::context::HirCtx;
use crate::intern::LitID;
use crate::support::AsStr;

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
        hir::FloatType::F64 | hir::FloatType::Untyped => (f64::MIN, f64::MAX),
    };

    if val.is_finite() && (val < min || val > max) {
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
            hir::ConstValue::Char { val, .. } => val,
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
    pub fn into_string(&self) -> LitID {
        match *self {
            hir::ConstValue::String { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_enum(self) -> (hir::EnumID, hir::VariantID) {
        match self {
            hir::ConstValue::Variant { enum_id, variant_id } => (enum_id, variant_id),
            _ => unreachable!(),
        }
    }
}
