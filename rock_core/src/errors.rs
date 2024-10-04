use crate::error::{Error, ErrorSink, Info, SourceRange, Warning, WarningSink};

//==================== COMMAND ====================

pub fn cmd_name_missing(emit: &mut impl ErrorSink) {
    let msg = "command name is missing, use `rock help` to learn the usage";
    emit.error(Error::message(msg));
}

pub fn cmd_unknown(emit: &mut impl ErrorSink, cmd: &str) {
    let msg = format!("command `{cmd}` is unknown, use `rock help` to learn the usage");
    emit.error(Error::message(msg));
}

pub fn cmd_expect_no_args(emit: &mut impl ErrorSink, cmd: &str) {
    let msg = format!("command `{cmd}` does not accept any arguments");
    emit.error(Error::message(msg));
}

pub fn cmd_expect_single_arg(emit: &mut impl ErrorSink, cmd: &str, name: &str) {
    let msg = format!("command `{cmd}` accepts a single `{name}` argument");
    emit.error(Error::message(msg));
}

pub fn cmd_expect_no_trail_args(emit: &mut impl ErrorSink, cmd: &str) {
    let msg = format!("command `{cmd}` does not accept trailing arguments");
    emit.error(Error::message(msg));
}

pub fn cmd_option_expect_no_args(emit: &mut impl ErrorSink, opt: &str) {
    let msg = format!("option `--{opt}` does not accept any arguments");
    emit.error(Error::message(msg));
}

pub fn cmd_option_conflict(emit: &mut impl ErrorSink, opt: &str, other: &str) {
    let msg = format!("options `--{opt}` and `--{other}` cannot be used together");
    emit.error(Error::message(msg));
}

pub fn cmd_option_unknown(emit: &mut impl ErrorSink, opt: &str) {
    let msg = format!("option `--{opt}` is unknown, use `rock help` to learn the usage");
    emit.error(Error::message(msg));
}

pub fn cmd_option_duplicate(emit: &mut impl ErrorSink, opt: &str) {
    let msg = format!("option `--{opt}` cannot be used multiple times");
    emit.error(Error::message(msg));
}

//==================== LEXER ====================

pub fn lexer_unknown_symbol(emit: &mut impl ErrorSink, src: SourceRange, c: char) {
    let non_acsii = if !c.is_ascii() {
        "\nonly ascii symbols are supported"
    } else {
        ""
    };
    let msg = format!("unknown symbol token `{c:?}`{non_acsii}");
    emit.error(Error::new(msg, src, None));
}

//==================== LEXER.CHAR ====================

pub fn lexer_char_incomplete(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal is incomplete";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_empty(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal cannot be empty";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_tab_not_escaped(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal `tab` must be escaped: `\\t`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_quote_not_escaped(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal `'` must be escaped: `\\'`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_not_terminated(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal not terminated, missing closing `'`";
    emit.error(Error::new(msg, src, None));
}

//==================== LEXER.STRING ====================

pub fn lexer_string_not_terminated(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "string literal not terminated, missing closing \"";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_raw_string_not_terminated(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "raw string literal not terminated, missing closing `";
    emit.error(Error::new(msg, src, None));
}

//==================== LEXER.ESCAPE ====================

pub fn lexer_escape_sequence_incomplete(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "escape sequence is incomplete\nif you meant to use `\\`, escape it: `\\\\`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_escape_sequence_not_supported(emit: &mut impl ErrorSink, src: SourceRange, c: char) {
    let msg = format!("escape sequence `\\{c}` is not supported");
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_escape_sequence_cstring_null(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg =
        "c string literals cannot contain any `\\0`\nnull terminator is automatically included";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_escape_hex_wrong_dc(emit: &mut impl ErrorSink, src: SourceRange, digit_count: u32) {
    let msg = format!("expected 1 to 6 hexadecimal digits, found {digit_count}");
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_escape_hex_non_utf8(emit: &mut impl ErrorSink, src: SourceRange, value: u32) {
    let msg = format!("hexadecimal value `{value:x}` is not valid UTF-8");
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_expect_open_bracket(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "expected opening `{` before hexadecimal value";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_expect_close_bracket(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "expected closing `}` after hexadecimal value";
    emit.error(Error::new(msg, src, None));
}

//==================== LEXER.NUMBER ====================

pub fn lexer_int_base_missing_digits(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "missing digits after integer base prefix";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_int_bin_invalid_digit(emit: &mut impl ErrorSink, src: SourceRange, digit: char) {
    let msg = format!("invalid digit `{digit}` for base 2 binary integer");
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_int_oct_invalid_digit(emit: &mut impl ErrorSink, src: SourceRange, digit: char) {
    let msg = format!("invalid digit `{digit}` for base 8 octal integer");
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_int_bin_overflow(emit: &mut impl ErrorSink, src: SourceRange, digit_count: u32) {
    let msg = format!(
        "binary integer overflow\nexpected maximum of 64 binary digits, found {digit_count}",
    );
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_int_hex_overflow(emit: &mut impl ErrorSink, src: SourceRange, digit_count: u32) {
    let msg = format!(
        "hexadecimal integer overflow\nexpected maximum of 16 hexadecimal digits, found {digit_count}",
    );
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_int_oct_overflow(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("octal integer overflow\nmaximum value is `0o17_77777_77777_77777_77777`",);
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_int_dec_overflow(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("decimal integer overflow\nmaximum value is `18_446_744_073_709_551_615`");
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_float_parse_failed(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "failed to parse float literal";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_float_exp_missing_digits(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "missing digits after float exponent";
    emit.error(Error::new(msg, src, None));
}

//==================== SCOPE ====================

pub fn scope_name_already_defined(
    emit: &mut impl ErrorSink,
    name_src: SourceRange,
    existing: SourceRange,
    name: &str,
) {
    let msg = format!("name `{name}` is defined multiple times");
    let info = Info::new("existing definition", existing);
    emit.error(Error::new(msg, name_src, info));
}

pub fn scope_symbol_is_private(
    emit: &mut impl ErrorSink,
    name_src: SourceRange,
    defined_src: SourceRange,
    symbol_kind: &'static str,
    name: &str,
) {
    let msg = format!("{symbol_kind} `{name}` is private");
    let info = Info::new("defined here", defined_src);
    emit.error(Error::new(msg, name_src, info));
}

pub fn scope_symbol_not_found(
    emit: &mut impl ErrorSink,
    name_src: SourceRange,
    name: &str,
    from_module: Option<&str>,
) {
    let msg = match from_module {
        Some(from_module) => format!("name `{name}` is not found in `{from_module}` module"),
        None => format!("name `{name}` is not found in this module"),
    };
    emit.error(Error::new(msg, name_src, None));
}

//==================== ATTRIBUTE ====================

pub fn attr_unknown(emit: &mut impl ErrorSink, attr_src: SourceRange, attr_name: &str) {
    let msg = format!("attribute `{attr_name}` is unknown");
    emit.error(Error::new(msg, attr_src, None));
}

pub fn attr_param_unknown(emit: &mut impl ErrorSink, param_src: SourceRange, param_name: &str) {
    let msg = format!("attribute parameter `{param_name}` is unknown");
    emit.error(Error::new(msg, param_src, None));
}

pub fn attr_param_value_unknown(
    emit: &mut impl ErrorSink,
    value_src: SourceRange,
    param_name: &str,
    value: &str,
) {
    let msg = format!("attribute parameter `{param_name}` value `{value}` is unknown");
    emit.error(Error::new(msg, value_src, None));
}

pub fn attr_param_value_unexpected(
    emit: &mut impl ErrorSink,
    value_src: SourceRange,
    param_name: &str,
) {
    let msg = format!("attribute parameter `{param_name}` expects no assigned string value");
    emit.error(Error::new(msg, value_src, None));
}

pub fn attr_param_value_required(
    emit: &mut impl ErrorSink,
    param_src: SourceRange,
    param_name: &str,
) {
    let msg = format!("attribute parameter `{param_name}` requires an assigned string value");
    emit.error(Error::new(msg, param_src, None));
}

pub fn attr_param_list_unexpected(
    emit: &mut impl ErrorSink,
    params_src: SourceRange,
    attr_name: &str,
) {
    let msg = format!("attribute `{attr_name}` expects no parameters");
    emit.error(Error::new(msg, params_src, None));
}

pub fn attr_expect_single_param(
    emit: &mut impl ErrorSink,
    param_src: SourceRange,
    attr_name: &str,
) {
    let msg = format!("attribute `{attr_name}` expects a single parameter");
    emit.error(Error::new(msg, param_src, None));
}

pub fn attr_param_list_required(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    attr_name: &str,
    exists: bool,
) {
    let non_empty = if exists { "non-empty " } else { "" };
    let msg = format!("attribute `{attr_name}` requires {non_empty}parameter list");
    emit.error(Error::new(msg, src, None));
}

pub fn attr_cannot_apply(
    emit: &mut impl ErrorSink,
    attr_src: SourceRange,
    attr_name: &str,
    item_kinds: &'static str,
) {
    let msg = format!("attribute `{attr_name}` cannot be applied to {item_kinds}",);
    emit.error(Error::new(msg, attr_src, None));
}

pub fn attr_struct_repr_int(
    emit: &mut impl ErrorSink,
    attr_src: SourceRange,
    int_ty: &'static str,
) {
    let msg = format!(
        "attribute `repr({int_ty})` cannot be applied to structs\nonly `repr(C)` is allowed",
    );
    emit.error(Error::new(msg, attr_src, None));
}

//==================== IMPORT ====================

pub fn import_package_dependency_not_found(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    dep_name: &str,
    src_name: &str,
) {
    let msg = format!("package `{dep_name}` is not found in dependencies of `{src_name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn import_expected_dir_not_found(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    dir_name: &str,
    pkg_name: &str,
) {
    let msg = format!("expected directory `{dir_name}` is not found in `{pkg_name}` package");
    emit.error(Error::new(msg, src, None));
}

pub fn import_expected_dir_found_mod(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    module_name: &str,
) {
    let msg = format!("expected directory, found module `{module_name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn import_expected_mod_not_found(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    module_name: &str,
    pkg_name: &str,
) {
    let msg = format!("expected module `{module_name}` is not found in `{pkg_name}` package");
    emit.error(Error::new(msg, src, None));
}

pub fn import_expected_mod_found_dir(emit: &mut impl ErrorSink, src: SourceRange, dir_name: &str) {
    let msg = format!("expected module, found directory `{dir_name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn import_module_into_itself(emit: &mut impl ErrorSink, src: SourceRange, module_name: &str) {
    let msg = format!("importing module `{module_name}` into itself is not allowed");
    emit.error(Error::new(msg, src, None));
}

pub fn import_name_alias_redundant(emit: &mut impl WarningSink, src: SourceRange, alias: &str) {
    let msg = format!("name alias `{alias}` is redundant, remove it");
    emit.warning(Warning::new(msg, src, None));
}

pub fn import_name_discard_redundant(emit: &mut impl WarningSink, src: SourceRange) {
    let msg = "name discard `_` is redundant, remove it";
    emit.warning(Warning::new(msg, src, None));
}

//==================== CONSTANT ====================

pub fn const_cannot_use_expr(emit: &mut impl ErrorSink, src: SourceRange, expr_kind: &'static str) {
    let msg = format!("cannot use `{expr_kind}` expression in constants");
    emit.error(Error::new(msg, src, None));
}

pub fn const_cannot_refer_to(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    item_kinds: &'static str,
) {
    let msg = format!("cannot refer to `{item_kinds}` in constants");
    emit.error(Error::new(msg, src, None));
}

pub fn const_int_div_by_zero(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs: i128,
    rhs: i128,
) {
    let msg = format!("integer division by zero\nwhen computing: `{lhs} {op} {rhs}`");
    emit.error(Error::new(msg, src, None));
}

pub fn const_float_div_by_zero(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs: f64,
    rhs: f64,
) {
    let msg = format!("float division by zero\nwhen computing: `{lhs} {op} {rhs}`");
    emit.error(Error::new(msg, src, None));
}

pub fn const_int_overflow(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs: i128,
    rhs: i128,
) {
    let msg = format!("integer constant overflow\nwhen computing: `{lhs} {op} {rhs}`");
    emit.error(Error::new(msg, src, None));
}

pub fn const_item_size_overflow(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    item_kind: &'static str,
    lhs: u64,
    rhs: u64,
) {
    let msg = format!("{item_kind} size overflow\nwhen computing: `{lhs} + {rhs}`");
    emit.error(Error::new(msg, src, None));
}

pub fn const_array_size_overflow(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    elem_size: u64,
    len: u64,
) {
    let msg = format!("array size overflow\nwhen computing: `{elem_size} * {len}`");
    emit.error(Error::new(msg, src, None));
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
    emit.error(Error::new(msg, src, None));
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
    emit.error(Error::new(msg, src, None));
}

pub fn const_index_out_of_bounds(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    index: u64,
    array_len: u64,
) {
    let msg = format!("index out of bounds\nvalue `{index}` is outside `0..<{array_len}` range");
    emit.error(Error::new(msg, src, None));
}

pub fn const_float_is_nan(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("float constant is NaN");
    emit.error(Error::new(msg, src, None));
}

pub fn const_float_is_infinite(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("float constant is Infinite");
    emit.error(Error::new(msg, src, None));
}

//==================== TYPECHECK ====================

pub fn tycheck_unused_expr(emit: &mut impl WarningSink, src: SourceRange, kind: &'static str) {
    let msg = format!("unused {kind}");
    emit.warning(Warning::new(msg, src, None));
}

pub fn tycheck_cannot_infer_enum_type(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot infer enum type";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_infer_struct_type(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot infer struct type";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_call_value_of_type(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    ty_fmt: &str,
) {
    let msg = format!("cannot call value of type `{ty_fmt}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_unexpected_proc_arg_count(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    proc_src: Option<SourceRange>,
    is_variadic: bool,
    input_count: usize,
    expected_count: usize,
) {
    let at_least = if is_variadic { " at least" } else { "" };
    let plural_end = if expected_count == 1 { "" } else { "s" };
    let msg =
        format!("expected{at_least} {expected_count} argument{plural_end}, found {input_count}");
    let info = match proc_src {
        Some(proc_src) => Info::new("procedure defined here", proc_src),
        None => None,
    };
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_unexpected_variant_arg_list(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    variant_src: SourceRange,
) {
    let msg = "variant has no fields, remove the argument list";
    let info = Info::new("variant defined here", variant_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_unexpected_variant_arg_count(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    variant_src: SourceRange,
    input_count: usize,
    expected_count: usize,
) {
    let plural_end = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected {expected_count} argument{plural_end}, found {input_count}");
    let info = Info::new("variant defined here", variant_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_unexpected_variant_bind_count(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    variant_src: SourceRange,
    input_count: usize,
    expected_count: usize,
) {
    let plural_end = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected {expected_count} binding{plural_end}, found {input_count}");
    let info = Info::new("variant defined here", variant_src);
    emit.error(Error::new(msg, src, info));
}
