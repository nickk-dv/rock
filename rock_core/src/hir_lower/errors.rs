use crate::error::{ErrorComp, ErrorSink, Info, SourceRange};

//==================== SCOPE ====================

pub fn scope_name_already_defined(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    existing: SourceRange,
    name: &str,
) {
    let msg = format!("name `{name}` is defined multiple times");
    let info = Info::new("existing definition", existing);
    emit.error(ErrorComp::new(msg, src, info));
}

//==================== ATTRIBUTE ====================

pub fn attr_unknown(emit: &mut impl ErrorSink, attr_src: SourceRange, attr_name: &str) {
    let msg = format!("attribute `{attr_name}` is unknown");
    emit.error(ErrorComp::new(msg, attr_src, None));
}

pub fn attr_param_unknown(emit: &mut impl ErrorSink, param_src: SourceRange, param_name: &str) {
    let msg = format!("attribute parameter `{param_name}` is unknown");
    emit.error(ErrorComp::new(msg, param_src, None));
}

pub fn attr_param_value_unknown(
    emit: &mut impl ErrorSink,
    value_src: SourceRange,
    param_name: &str,
    value: &str,
) {
    let msg = format!("attribute parameter `{param_name}` value `{value}` is unknown");
    emit.error(ErrorComp::new(msg, value_src, None));
}

pub fn attr_param_value_unexpected(
    emit: &mut impl ErrorSink,
    value_src: SourceRange,
    param_name: &str,
) {
    let msg = format!("attribute parameter `{param_name}` expects no assigned string value");
    emit.error(ErrorComp::new(msg, value_src, None));
}

pub fn attr_param_value_required(
    emit: &mut impl ErrorSink,
    param_src: SourceRange,
    param_name: &str,
) {
    let msg = format!("attribute parameter `{param_name}` requires an assigned string value");
    emit.error(ErrorComp::new(msg, param_src, None));
}

pub fn attr_param_list_unexpected(
    emit: &mut impl ErrorSink,
    params_src: SourceRange,
    attr_name: &str,
) {
    let msg = format!("attribute `{attr_name}` expects no parameters");
    emit.error(ErrorComp::new(msg, params_src, None));
}

pub fn attr_expect_single_param(
    emit: &mut impl ErrorSink,
    param_src: SourceRange,
    attr_name: &str,
) {
    let msg = format!("attribute `{attr_name}` expects a single parameter");
    emit.error(ErrorComp::new(msg, param_src, None));
}

pub fn attr_param_list_required(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    attr_name: &str,
    exists: bool,
) {
    let non_empty = if exists { "non-empty " } else { "" };
    let msg = format!("attribute `{attr_name}` requires {non_empty}parameter list");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn attr_cannot_apply(
    emit: &mut impl ErrorSink,
    attr_src: SourceRange,
    attr_name: &str,
    item_kinds: &'static str,
) {
    let msg = format!("attribute `{attr_name}` cannot be applied to {item_kinds}",);
    emit.error(ErrorComp::new(msg, attr_src, None));
}

pub fn attr_struct_repr_int(
    emit: &mut impl ErrorSink,
    attr_src: SourceRange,
    int_ty: &'static str,
) {
    let msg = format!(
        "attribute `repr({int_ty})` cannot be applied to structs\nonly `repr(C)` is allowed",
    );
    emit.error(ErrorComp::new(msg, attr_src, None));
}

//==================== CONSTANT ====================

pub fn const_cannot_use_expr(emit: &mut impl ErrorSink, src: SourceRange, expr_kind: &'static str) {
    let msg = format!("cannot use `{expr_kind}` expression in constants");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_cannot_refer_to(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    item_kinds: &'static str,
) {
    let msg = format!("cannot refer to `{item_kinds}` in constants");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_int_div_by_zero(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs: i128,
    rhs: i128,
) {
    let msg = format!("integer division by zero\nwhen computing: `{lhs} {op} {rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_div_by_zero(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs: f64,
    rhs: f64,
) {
    let msg = format!("float division by zero\nwhen computing: `{lhs} {op} {rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_int_overflow(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs: i128,
    rhs: i128,
) {
    let msg = format!("integer constant overflow\nwhen computing: `{lhs} {op} {rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_item_size_overflow(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    item_kind: &'static str,
    lhs: u64,
    rhs: u64,
) {
    let msg = format!("{item_kind} size overflow\nwhen computing: `{lhs} + {rhs}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_array_size_overflow(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    elem_size: u64,
    len: u64,
) {
    let msg = format!("array size overflow\nwhen computing: `{elem_size} * {len}`");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_int_out_of_range(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    int_ty: &'static str,
    val: i128,
    min: i128,
    max: i128,
) {
    let msg = format!(
        "integer constant out of range for `{int_ty}`\nvalue `{val}` is outside `{min}..={max}` range",
    );
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_out_of_range(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    float_ty: &'static str,
    val: f64,
    min: f64,
    max: f64,
) {
    let msg = format!(
        "float constant out of range for `{float_ty}`\nvalue `{val}` is outside `{min}..={max}` range",
    );
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_index_out_of_bounds(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    index: u64,
    array_len: u64,
) {
    let msg = format!("index out of bounds\nvalue `{index}` is outside `0..<{array_len}` range");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_is_nan(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("float constant is NaN");
    emit.error(ErrorComp::new(msg, src, None));
}

pub fn const_float_is_infinite(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("float constant is Infinite");
    emit.error(ErrorComp::new(msg, src, None));
}
