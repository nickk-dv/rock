use crate::error::{Error, ErrorSink, Info, SourceRange, Warning, WarningSink};
use std::path::PathBuf;

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

//==================== SESSION ====================

pub fn session_pkg_not_found(path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!(
        "could not find dependency package in `{path}`
package fetch from remote index is not yet implemented
currently dependecy packages must be placed in `<rock_install>/packages`"
    );
    Error::message(msg)
}

pub fn session_manifest_not_found(path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("could not find `Rock.toml` manifest in `{path}`");
    Error::message(msg)
}

pub fn session_src_not_found(path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("could not find `src` directory in `{path}`");
    Error::message(msg)
}

//@add more context information
pub fn session_dep_on_bin() -> Error {
    let msg = format!("cannot depend on a binary package",);
    Error::message(msg)
}

pub fn session_pkg_dep_cycle(relation: String, manifest_path: &PathBuf) -> Error {
    let msg = format!(
        "package dependency cycle detected\nfrom package manifest in `{}`\n{relation}",
        manifest_path.to_string_lossy()
    );
    Error::message(msg)
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

//==================== PARSER ====================

pub fn parse_bin_op_prec_conflit(
    emit: &mut impl ErrorSink,
    op_src: SourceRange,
    lhs_src: SourceRange,
    bin_src: SourceRange,
) {
    let msg =
        "binary operator precedence conflict\nadd parenthesis to specify the order of operations";
    emit.error(Error::new_info_vec(
        msg,
        "",
        op_src,
        vec![Info::new_val("", lhs_src), Info::new_val("", bin_src)],
    ));
}

//==================== SYNTAX ====================

pub fn syntax_invalid_doc_comment(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg =
        "invalid documentation comment placement\ndocumentation comments are only allowed before items";
    emit.error(Error::new(msg, src, None));
}

pub fn syntax_invalid_mod_comment(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg =
        "invalid module comment placement\nmodule comments are only allowed at the start of the source file";
    emit.error(Error::new(msg, src, None));
}

//==================== CHECK SCOPE ====================

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
    name: &str,
    symbol_kind: &'static str,
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

pub fn scope_enum_variant_not_found(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    enum_src: SourceRange,
    name: &str,
    enum_name: &str,
) {
    let msg = format!("variant `{name}` not found in `{enum_name}`");
    let info = Info::new("enum defined here", enum_src);
    emit.error(Error::new(msg, src, info));
}

pub fn scope_struct_field_not_found(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    struct_src: SourceRange,
    name: &str,
    struct_name: &str,
) {
    let msg = format!("field `{name}` not found in `{struct_name}`");
    let info = Info::new("struct defined here", struct_src);
    emit.error(Error::new(msg, src, info));
}

//==================== CHECK ATTRIBUTE ====================

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
    list_src: SourceRange,
    attr_name: &str,
) {
    let msg = format!("attribute `{attr_name}` expects no parameters");
    emit.error(Error::new(msg, list_src, None));
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

pub fn attr_duplicate(emit: &mut impl ErrorSink, attr_src: SourceRange, attr_name: &str) {
    let msg = format!("duplicate attribute `{attr_name}`");
    emit.error(Error::new(msg, attr_src, None));
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

pub fn attr_not_compatible(
    emit: &mut impl ErrorSink,
    attr_src: SourceRange,
    attr_name: &str,
    old_flag: &str,
    item_kinds: &'static str,
) {
    let msg = format!("attribute `{attr_name}` cannot be applied to `{old_flag}` {item_kinds}",);
    emit.error(Error::new(msg, attr_src, None));
}

pub fn attr_flag_not_compatible(
    emit: &mut impl ErrorSink,
    item_src: SourceRange,
    new_flag: &str,
    old_flag: &str,
    item_kinds: &'static str,
) {
    let msg = format!("`{new_flag}` {item_kinds} cannot be `{old_flag}`",);
    emit.error(Error::new(msg, item_src, None));
}

pub fn attr_proc_variadic_not_external(emit: &mut impl ErrorSink, proc_src: SourceRange) {
    let msg = "`variadic` procedures must be `external`";
    emit.error(Error::new(msg, proc_src, None));
}

pub fn attr_proc_variadic_zero_params(emit: &mut impl ErrorSink, proc_src: SourceRange) {
    let msg = "`variadic` procedures must have at least one parameter";
    emit.error(Error::new(msg, proc_src, None));
}

pub fn attr_proc_builtin_with_block(
    emit: &mut impl ErrorSink,
    proc_src: SourceRange,
    block_src: SourceRange,
) {
    let msg = "`builtin` procedures cannot have a body";
    let info = Info::new("remove this block", block_src);
    emit.error(Error::new(msg, proc_src, info));
}

//==================== CHECK IMPORT ====================

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

pub fn import_expected_dir_found_mod(emit: &mut impl ErrorSink, src: SourceRange, mod_name: &str) {
    let msg = format!("expected directory, found module `{mod_name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn import_expected_mod_not_found(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    mod_name: &str,
    pkg_name: &str,
) {
    let msg = format!("expected module `{mod_name}` is not found in `{pkg_name}` package");
    emit.error(Error::new(msg, src, None));
}

pub fn import_expected_mod_found_dir(emit: &mut impl ErrorSink, src: SourceRange, dir_name: &str) {
    let msg = format!("expected module, found directory `{dir_name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn import_module_into_itself(emit: &mut impl ErrorSink, src: SourceRange, mod_name: &str) {
    let msg = format!("importing module `{mod_name}` into itself is not allowed");
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

//==================== CHECK ITEM ====================

pub fn item_param_already_defined(
    emit: &mut impl ErrorSink,
    param_src: SourceRange,
    existing: SourceRange,
    name: &str,
) {
    let msg = format!("parameter `{name}` is defined multiple times");
    let info = Info::new("existing parameter", existing);
    emit.error(Error::new(msg, param_src, info));
}

pub fn item_variant_already_defined(
    emit: &mut impl ErrorSink,
    variant_src: SourceRange,
    existing: SourceRange,
    name: &str,
) {
    let msg = format!("variant `{name}` is defined multiple times");
    let info = Info::new("existing variant", existing);
    emit.error(Error::new(msg, variant_src, info));
}

pub fn item_field_already_defined(
    emit: &mut impl ErrorSink,
    field_src: SourceRange,
    existing: SourceRange,
    name: &str,
) {
    let msg = format!("field `{name}` is defined multiple times");
    let info = Info::new("existing field", existing);
    emit.error(Error::new(msg, field_src, info));
}

pub fn item_enum_non_int_tag_ty(emit: &mut impl ErrorSink, tag_src: SourceRange) {
    let msg = "enum tag type must be an integer";
    emit.error(Error::new(msg, tag_src, None));
}

pub fn item_enum_unknown_tag_ty(emit: &mut impl ErrorSink, enum_src: SourceRange) {
    let msg = "enum tag type must be specified\nadd type after name or use #[repr_c] attribute";
    emit.error(Error::new(msg, enum_src, None));
}

//==================== CHECK PATH ====================

pub fn path_not_expected(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    found_src: SourceRange,
    name: &str,
    expected: &'static str,
    found: &'static str,
) {
    let msg = format!("expected {expected}, found {found} `{name}`");
    let info = Info::new("defined here", found_src);
    emit.error(Error::new(msg, src, info));
}

pub fn path_unexpected_segment(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    after_kind: &'static str,
) {
    let msg = format!("unexpected path segment after {after_kind}");
    emit.error(Error::new(msg, src, None));
}

//==================== CHECK CONSTANT ====================

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

pub fn tycheck_type_mismatch(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    expect_src: Option<SourceRange>,
    expected_ty_fmt: &str,
    found_ty_fmt: &str,
) {
    let msg = format!("type mismatch: expected `{expected_ty_fmt}`, found `{found_ty_fmt}`");
    let info = expect_src.map(|src| Info::new_val("expected due to this", src));
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_if_missing_else(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "`if` expression is missing an `else` block\n`if` without `else` evaluates to `void` and cannot return a value";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_match_on_ty(emit: &mut impl ErrorSink, src: SourceRange, ty_fmt: &str) {
    let msg = format!("cannot match on value of type `{ty_fmt}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_const_field_access(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("cannot access fields in patterns");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_const_with_bindings(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("constant patterns cannot have bindings");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_runtime_value(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("cannot use runtime values in patterns");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_field_is_private(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    field_name: &str,
    field_src: SourceRange,
) {
    let msg = format!("field `{field_name}` is private");
    let info = Info::new("defined here", field_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_field_not_found_ty(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    field_name: &str,
    ty_fmt: &str,
) {
    let msg = format!("no field `{field_name}` exists on value of type `{ty_fmt}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_field_not_found_slice(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    field_name: &str,
) {
    let msg = format!("no field `{field_name}` exists on slice type\ndid you mean `ptr` or `len`?");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_field_not_found_array(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    field_name: &str,
) {
    let msg = format!("no field `{field_name}` exists on array type\ndid you mean `len`?");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_index_on_ty(emit: &mut impl ErrorSink, src: SourceRange, ty_fmt: &str) {
    let msg = format!("cannot index value of type `{ty_fmt}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cast_non_primitive(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    from_ty: &str,
    into_ty: &str,
) {
    let msg = format!("non primitive cast from `{from_ty}` into `{into_ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cast_redundant(
    emit: &mut impl WarningSink,
    src: SourceRange,
    from_ty: &str,
    into_ty: &str,
) {
    let msg = format!("redundant cast from `{from_ty}` into `{into_ty}`");
    emit.warning(Warning::new(msg, src, None));
}

pub fn tycheck_unreachable_stmt(emit: &mut impl WarningSink, src: SourceRange, after: SourceRange) {
    let msg = "unreachable statement";
    let info = Info::new("all statements after this are unreachable", after);
    emit.warning(Warning::new(msg, src, info));
}

pub fn tycheck_break_outside_loop(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "break outside of loop";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_break_in_defer(emit: &mut impl ErrorSink, src: SourceRange, defer_src: SourceRange) {
    let msg = "break in loop started outside of defer";
    let info = Info::new("in this defer", defer_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_continue_outside_loop(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "continue outside of loop";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_continue_in_defer(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    defer_src: SourceRange,
) {
    let msg = "continue in loop started outside of defer";
    let info = Info::new("in this defer", defer_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_return_in_defer(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    defer_src: SourceRange,
) {
    let msg = "cannot return in defer";
    let info = Info::new("in this defer", defer_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_defer_in_defer(emit: &mut impl ErrorSink, src: SourceRange, defer_src: SourceRange) {
    let msg = "defer statements cannot be nested";
    let info = Info::new("in this defer", defer_src);
    emit.error(Error::new(msg, src, info));
}

//==================== TYPECHECK UNUSED ====================

pub fn tycheck_unused_expr(emit: &mut impl WarningSink, src: SourceRange, expr_kind: &'static str) {
    let msg = format!("unused {expr_kind}");
    emit.warning(Warning::new(msg, src, None));
}

//==================== TYPECHECK INFER ====================

pub fn tycheck_cannot_infer_enum_type(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot infer enum type";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_infer_struct_type(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot infer struct type";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_infer_empty_array(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot infer type of empty array";
    emit.error(Error::new(msg, src, None));
}

//==================== TYPECHECK CALL & INPUT ====================

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
    let plural = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected{at_least} {expected_count} argument{plural}, found {input_count}");
    let info = proc_src.map(|src| Info::new_val("procedure defined here", src));
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_unexpected_variant_arg_count(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    variant_src: SourceRange,
    input_count: usize,
    expected_count: usize,
) {
    let plural = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected {expected_count} argument{plural}, found {input_count}");
    let info = Info::new("variant defined here", variant_src);
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

pub fn tycheck_unexpected_variant_bind_count(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    variant_src: SourceRange,
    input_count: usize,
    expected_count: usize,
) {
    let plural = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected {expected_count} binding{plural}, found {input_count}");
    let info = Info::new("variant defined here", variant_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_unexpected_variant_bind_list(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    variant_src: SourceRange,
) {
    let msg = "variant has no fields, remove the binding list";
    let info = Info::new("variant defined here", variant_src);
    emit.error(Error::new(msg, src, info));
}

//==================== TYPECHECK ADDRESS ====================

pub fn tycheck_cannot_ref_slice_field(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot get reference to a slice field";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_assign_slice_field(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot assign to a slice field";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_ref_temporary(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot get reference to a temporary value";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_ref_temporary_immut(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot get `&mut` to this temporary value, only `&` is allowed";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_assign_temporary(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot assign to a temporary value";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_ref_constant(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    const_src: SourceRange,
) {
    let msg = "cannot get reference to a constant, you can use `global` instead";
    let info = Info::new("constant defined here", const_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_assign_constant(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    const_src: SourceRange,
) {
    let msg = "cannot assign to a constant, you can use `global` instead";
    let info = Info::new("constant defined here", const_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_ref_var_immut(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    var_src: SourceRange,
) {
    let msg = "cannot get `&mut` to an immutable variable";
    let info = Info::new("variable defined here", var_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_assign_var_immut(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    var_src: SourceRange,
) {
    let msg = "cannot assign to an immutable variable";
    let info = Info::new("variable defined here", var_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_ref_val_behind_ref(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    deref_src: SourceRange,
) {
    let msg = "cannot get `&mut` to a value behind an immutable reference";
    let info = Info::new("immutable reference accessed here", deref_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_assign_val_behind_ref(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    deref_src: SourceRange,
) {
    let msg = "cannot assign to a value behind an immutable reference";
    let info = Info::new("immutable reference accessed here", deref_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_ref_val_behind_slice(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    slice_src: SourceRange,
) {
    let msg = "cannot get `&mut` to a value behind an immutable slice";
    let info = Info::new("immutable slice accessed here", slice_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_assign_val_behind_slice(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    slice_src: SourceRange,
) {
    let msg = "cannot assign to a value behind an immutable slice";
    let info = Info::new("immutable slice accessed here", slice_src);
    emit.error(Error::new(msg, src, info));
}

//==================== TYPECHECK OTHER ====================

pub fn tycheck_cannot_apply_un_op(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    rhs_ty_fmt: &str,
) {
    let msg = format!("cannot apply unary operator `{op}` on value of type `{rhs_ty_fmt}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_apply_bin_op(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs_ty_fmt: &str,
) {
    let msg = format!("cannot apply binary operator `{op}` on value of type `{lhs_ty_fmt}`");
    emit.error(Error::new(msg, src, None));
}

//==================== ENTRY POINT ====================

pub fn entry_main_mod_not_found(emit: &mut impl ErrorSink) {
    let msg = "could not find `main` module, expected `src/main.rock` to exist";
    emit.error(Error::message(msg));
}

pub fn entry_main_proc_not_found(emit: &mut impl ErrorSink) {
    let msg = "could not find entry point in `src/main.rock`\ndefine it like this: `proc main() -> s32 { return 0; }`";
    emit.error(Error::message(msg));
}

pub fn entry_main_with_parameters(emit: &mut impl ErrorSink, main_src: SourceRange) {
    let msg = "`main` procedure cannot have any parameters`";
    emit.error(Error::new(msg, main_src, None));
}

pub fn entry_main_wrong_return_ty(emit: &mut impl ErrorSink, ret_src: SourceRange) {
    let msg = "`main` procedure must return `s32` or `never`";
    emit.error(Error::new(msg, ret_src, None));
}

//==================== INTERNAL ====================

pub fn internal_generic_types_not_implemented(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "internal: generic parameterized types are not implemented";
    emit.error(Error::new(msg, src, None));
}

pub fn internal_slice_expr_not_implemented(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "internal: slice expression not implemented";
    emit.error(Error::new(msg, src, None));
}
