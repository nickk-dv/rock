use super::hir_build::HirEmit;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;

//==================== ATTRIBUTE ====================

pub fn attr_unknown(emit: &mut HirEmit, attr_src: SourceRange, attr_name: &str) {
    let msg = format!("attribute `{attr_name}` is unknown");
    emit.error(ErrorComp::new(msg, attr_src, None));
}

pub fn attr_param_unknown(emit: &mut HirEmit, param_src: SourceRange, param_name: &str) {
    let msg = format!("attribute parameter `{param_name}` is unknown");
    emit.error(ErrorComp::new(msg, param_src, None));
}

pub fn attr_param_value_unknown(
    emit: &mut HirEmit,
    value_src: SourceRange,
    param_name: &str,
    value: &str,
) {
    let msg = format!("attribute parameter `{param_name}` value `{value}` is unknown");
    emit.error(ErrorComp::new(msg, value_src, None));
}

pub fn attr_param_value_required(emit: &mut HirEmit, param_src: SourceRange, param_name: &str) {
    let msg = format!("attribute parameter `{param_name}` requires an assigned string value");
    emit.error(ErrorComp::new(msg, param_src, None));
}

pub fn attr_param_list_required(
    emit: &mut HirEmit,
    src: SourceRange,
    attr_name: &str,
    exists: bool,
) {
    let non_empty = if exists { "non-empty " } else { "" };
    let msg = format!("attribute `{attr_name}` requires {non_empty}parameter list");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn attr_expected_single_param(emit: &mut HirEmit, param_src: SourceRange, attr_name: &str) {
    let msg = format!("attribute `{attr_name}` only expects a single parameter");
    emit.error(ErrorComp::new(msg, param_src, None));
}

//==================== CONSTANT ====================

pub fn const_cannot_use_expr(emit: &mut HirEmit, src: SourceRange, name: &'static str) {
    let msg = format!("cannot use `{name}` expression in constants");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_cannot_refer_to(emit: &mut HirEmit, src: SourceRange, name: &'static str) {
    let msg = format!("cannot refer to `{name}` in constants");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_int_div_by_zero(
    emit: &mut HirEmit,
    src: SourceRange,
    op: hir::BinOp,
    lhs: i128,
    rhs: i128,
) {
    let op_str = op.as_str();
    let msg = format!("integer division by zero\nwhen computing: `{lhs}` {op_str} `{rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_div_by_zero(
    emit: &mut HirEmit,
    src: SourceRange,
    op: hir::BinOp,
    lhs: f64,
    rhs: f64,
) {
    let op_str = op.as_str();
    let msg = format!("float division by zero\nwhen computing: `{lhs}` {op_str} `{rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_int_overflow(
    emit: &mut HirEmit,
    src: SourceRange,
    op: hir::BinOp,
    lhs: i128,
    rhs: i128,
) {
    let op_str = op.as_str();
    let msg = format!("integer constant overflow\nwhen computing: `{lhs}` {op_str} `{rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_item_size_overflow(
    emit: &mut HirEmit,
    src: SourceRange,
    item_kind: &'static str,
    lhs: u64,
    rhs: u64,
) {
    let msg = format!("{item_kind} size overflow\nwhen computing: `{lhs}` + `{rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_array_size_overflow(emit: &mut HirEmit, src: SourceRange, elem_size: u64, len: u64) {
    let msg = format!("array size overflow\nwhen computing: `{elem_size}` * `{len}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_int_out_of_range(
    emit: &mut HirEmit,
    src: SourceRange,
    int_ty: hir::BasicInt,
    val: i128,
    min: i128,
    max: i128,
) {
    let msg = format!(
        "integer constant out of range for `{}`\nvalue `{val}` is outside `{min}..={max}` range",
        int_ty.into_basic().as_str()
    );
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_out_of_range(
    emit: &mut HirEmit,
    src: SourceRange,
    float_ty: hir::BasicFloat,
    val: f64,
    min: f64,
    max: f64,
) {
    let msg = format!(
        "float constant out of range for `{}`\nvalue `{val}` is outside `{min}..={max}` range",
        float_ty.into_basic().as_str()
    );
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_index_out_of_bounds(emit: &mut HirEmit, src: SourceRange, index: u64, array_len: u64) {
    let msg = format!("index out of bounds\nvalue `{index}` is outside `0..<{array_len}` range");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_is_nan(emit: &mut HirEmit, src: SourceRange) {
    let msg = format!("float constant is NaN");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_is_infinite(emit: &mut HirEmit, src: SourceRange) {
    let msg = format!("float constant is Infinite");
    emit.error(ErrorComp::new(msg, src, None));
}
