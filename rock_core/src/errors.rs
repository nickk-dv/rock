use crate::error::{Error, ErrorSink, Info, SourceRange, Warning, WarningSink};
use std::path::PathBuf;

//==================== OPERATING SYSTEM ====================

pub fn os_current_exe_path(io_error: String) -> Error {
    let msg = format!("failed to obtain current executable path\nreason: {io_error}");
    Error::message(msg)
}

pub fn os_dir_get_current_working(io_error: String) -> Error {
    let msg = format!("failed to obtain current working directory\nreason: {io_error}");
    Error::message(msg)
}

pub fn os_dir_set_current_working(io_error: String, path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("failed to set current working directory: `{path}`\nreason: {io_error}",);
    Error::message(msg)
}

pub fn os_dir_create(io_error: String, path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("failed to create directory: `{path}`\nreason: {io_error}");
    Error::message(msg)
}

pub fn os_dir_read(io_error: String, path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("failed to read directory: `{path}`\nreason: {io_error}");
    Error::message(msg)
}

pub fn os_dir_entry_read(io_error: String, path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("failed to read directory entry in: `{path}`\nreason: {io_error}");
    Error::message(msg)
}

pub fn os_file_create(io_error: String, path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("failed to create file: `{path}`\nreason: {io_error}");
    Error::message(msg)
}

pub fn os_file_read(io_error: String, path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("failed to read file: `{path}`\nreason: {io_error}");
    Error::message(msg)
}

pub fn os_filename_missing(path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("filename is missing in: `{path}`");
    Error::message(msg)
}

pub fn os_filename_non_utf8(path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("filename is not valid UTF-8: `{path}`");
    Error::message(msg)
}

pub fn os_canonicalize(io_error: String, path: &PathBuf) -> Error {
    let path = path.to_string_lossy();
    let msg = format!("failed to canonicalize path: `{path}`\nreason: {io_error}");
    Error::message(msg)
}

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

pub fn cmd_cannot_build_lib_package() -> Error {
    let msg = "cannot `build` a library package, use `check` instead";
    Error::message(msg)
}

pub fn cmd_cannot_run_lib_package() -> Error {
    let msg = "cannot `run` a library package, use `check` instead";
    Error::message(msg)
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

pub fn session_dep_on_bin(dep_path: &str, dep_name: &str, pkg_name: &str) -> Error {
    let msg = format!("cannot depend on a binary package:\n`{dep_name}` located in `{dep_path}` depends on `{pkg_name}`");
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

pub fn lexer_symbol_unknown(emit: &mut impl ErrorSink, src: SourceRange, c: char) {
    let msg = format!("unknown symbol token {c:?}");
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_empty(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal cannot be empty";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_incomplete(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal is incomplete";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_not_terminated(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal not terminated, missing closing `'`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_quote_not_escaped(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal `'` must be escaped: `\\'`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_char_tab_not_escaped(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "character literal `tab` must be escaped: `\\t`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_string_not_terminated(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "string literal not terminated, missing closing \"";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_string_raw_not_terminated(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "raw string literal not terminated, missing closing `";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_escape_sequence_incomplete(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "escape sequence is incomplete\nif you meant to use `\\`, escape it: `\\\\`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_escape_sequence_not_supported(emit: &mut impl ErrorSink, src: SourceRange, c: char) {
    let msg = format!("escape sequence `\\{c}` is not supported");
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

pub fn lexer_int_base_missing_digits(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "missing digits after integer base prefix";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_int_bin_invalid_digit(emit: &mut impl ErrorSink, src: SourceRange, digit: char) {
    let msg = format!("invalid digit `{digit}` for base 2 binary integer");
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

pub fn lexer_int_dec_overflow(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "decimal integer overflow\nmaximum value is `18_446_744_073_709_551_615`";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_float_exp_missing_digits(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "missing digits after float exponent";
    emit.error(Error::new(msg, src, None));
}

pub fn lexer_float_parse_failed(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "failed to parse float literal";
    emit.error(Error::new(msg, src, None));
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

pub fn scope_symbol_private_vis(
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

pub fn scope_symbol_package_vis(
    emit: &mut impl ErrorSink,
    name_src: SourceRange,
    defined_src: SourceRange,
    name: &str,
    symbol_kind: &'static str,
) {
    let msg = format!("{symbol_kind} `{name}` has package visibility");
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
        None => format!("name `{name}` is not found in this scope"),
    };
    emit.error(Error::new(msg, name_src, None));
}

pub fn scope_core_symbol_not_found(emit: &mut impl ErrorSink, name: &str, kind: &'static str) {
    let msg = format!("{kind} `{name}` is not found in core library");
    emit.error(Error::message(msg));
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

pub fn scope_symbol_unused(emit: &mut impl WarningSink, src: SourceRange, name: &str, kind: &str) {
    let msg = format!("unused {kind} `{name}`");
    emit.warning(Warning::new(msg, src, None));
}

//==================== CHECK DIRECTIVE & FLAG ====================

pub fn directive_unknown(emit: &mut impl ErrorSink, src: SourceRange, name: &str) {
    let msg = format!("directive `#{name}` is unknown");
    emit.error(Error::new(msg, src, None));
}

pub fn directive_duplicate(emit: &mut impl ErrorSink, src: SourceRange, name: &str) {
    let msg = format!("duplicate directive `#{name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn directive_param_unknown(
    emit: &mut impl ErrorSink,
    param_src: SourceRange,
    param_name: &str,
) {
    let msg = format!("directive parameter `{param_name}` is unknown");
    emit.error(Error::new(msg, param_src, None));
}

pub fn directive_param_value_unknown(
    emit: &mut impl ErrorSink,
    value_src: SourceRange,
    param_name: &str,
    param_value: &str,
) {
    let msg = format!("directive parameter value `{param_value}` for `{param_name}` is unknown");
    emit.error(Error::new(msg, value_src, None));
}

pub fn directive_cannot_apply(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    name: &str,
    item_kinds: &'static str,
) {
    let msg = format!("directive `#{name}` cannot be applied to {item_kinds}",);
    emit.error(Error::new(msg, src, None));
}

pub fn directive_scope_vis_redundant(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    prev_src: Option<SourceRange>,
    vis: &str,
) {
    let msg = format!("scope visibility is already `{vis}`, remove this directive");
    let info = prev_src.map(|prev| Info::new_val("already set here", prev));
    emit.error(Error::new(msg, src, info));
}

pub fn directive_param_must_be_last(emit: &mut impl ErrorSink, src: SourceRange, dir_name: &str) {
    let msg = format!("`#{dir_name}` parameter must be last");
    emit.error(Error::new(msg, src, None));
}

pub fn flag_proc_intrinsic_non_core(emit: &mut impl ErrorSink, proc_src: SourceRange) {
    let msg = "`intrinsic` procedures can only be defined in core library";
    emit.error(Error::new(msg, proc_src, None));
}

pub fn flag_proc_variadic_external(emit: &mut impl ErrorSink, proc_src: SourceRange) {
    let msg =
        "`variadic` procedures cannot be `external`\ndid you mean to use #c_variadic instead?";
    emit.error(Error::new(msg, proc_src, None));
}

pub fn flag_proc_c_variadic_not_external(emit: &mut impl ErrorSink, proc_src: SourceRange) {
    let msg = "`c_variadic` procedures must be `external`\ndid you mean to use #variadic instead?";
    emit.error(Error::new(msg, proc_src, None));
}

pub fn flag_proc_c_variadic_zero_params(emit: &mut impl ErrorSink, proc_src: SourceRange) {
    let msg = "`c_variadic` procedures must have at least one parameter";
    emit.error(Error::new(msg, proc_src, None));
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

pub fn item_type_param_already_defined(
    emit: &mut impl ErrorSink,
    param_src: SourceRange,
    existing: SourceRange,
    name: &str,
) {
    let msg = format!("type parameter `{name}` is defined multiple times");
    let info = Info::new("existing type parameter", existing);
    emit.error(Error::new(msg, param_src, info));
}

pub fn item_poly_params_empty(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "polymorphic parameter list is empty, remove it";
    emit.error(Error::new(msg, src, None));
}

pub fn item_variant_fields_empty(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "variant field list is empty, remove it";
    emit.error(Error::new(msg, src, None));
}

pub fn item_enum_non_int_tag_ty(emit: &mut impl ErrorSink, tag_src: SourceRange) {
    let msg = "enum tag type must be an integer";
    emit.error(Error::new(msg, tag_src, None));
}

pub fn item_enum_unknown_tag_ty(emit: &mut impl ErrorSink, enum_src: SourceRange) {
    let msg = "enum tag type must be specified\nadd type after the enum name";
    emit.error(Error::new(msg, enum_src, None));
}

pub fn item_enum_duplicate_tag_value(
    emit: &mut impl ErrorSink,
    variant_src: SourceRange,
    existing: SourceRange,
    variant_name: &str,
    existing_name: &str,
    tag_value: i128,
) {
    let msg = format!("duplicate variant tag value\nboth `{variant_name}` and `{existing_name}` have the same `{tag_value}` value");
    let info = Info::new("existing variant with the same tag", existing);
    emit.error(Error::new(msg, variant_src, info));
}

//==================== CHECK PATH ====================

pub fn path_not_expected(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    defined_src: Option<SourceRange>,
    name: &str,
    expected: &'static str,
    found: &'static str,
) {
    let msg = format!("expected {expected}, found {found} `{name}`");
    let info = defined_src.map(|s| Info::new_val("defined here", s));
    emit.error(Error::new(msg, src, info));
}

pub fn path_unexpected_segments(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    after_kind: &'static str,
) {
    let msg = format!("unexpected path segments after {after_kind}");
    emit.error(Error::new(msg, src, None));
}

pub fn path_unexpected_poly_args(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    after_kind: &'static str,
) {
    let msg = format!("unexpected polymorphic arguments after {after_kind}");
    emit.error(Error::new(msg, src, None));
}

pub fn path_type_unexpected_poly_args(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    name: &str,
    item_kind: &'static str,
) {
    let msg = format!("unexpected polymorphic arguments for {item_kind} `{name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn path_type_missing_poly_args(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    name: &str,
    item_kind: &'static str,
) {
    let msg = format!("missing polymorphic arguments for {item_kind} `{name}`");
    emit.error(Error::new(msg, src, None));
}

pub fn path_unexpected_poly_arg_count(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    input_count: usize,
    expected_count: usize,
) {
    let plural = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected {expected_count} type argument{plural}, found {input_count}");
    emit.error(Error::new(msg, src, None));
}

//==================== CHECK CONSTANT ====================

pub fn const_cannot_use_expr(emit: &mut impl ErrorSink, src: SourceRange, expr_kind: &'static str) {
    let msg = format!("cannot use `{expr_kind}` expression in constants");
    emit.error(Error::new(msg, src, None));
}

pub fn const_dependency_cycle(
    emit: &mut impl ErrorSink,
    ctx_msg: String,
    src: SourceRange,
    info_vec: Vec<Info>,
) {
    let msg = "constant dependency cycle found:";
    emit.error(Error::new_info_vec(msg, ctx_msg, src, info_vec));
}

pub fn const_int_div_by_zero(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("integer division by zero");
    emit.error(Error::new(msg, src, None));
}

pub fn const_int_overflow(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs: i128,
    rhs: i128,
) {
    let msg = format!("integer constant overflow when computing: `{lhs} {op} {rhs}`");
    emit.error(Error::new(msg, src, None));
}

pub fn const_size_overflow(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = format!("constant size overflow when computing type layout");
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

pub fn const_shift_out_of_range(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    val: i128,
    min: i128,
    max: i128,
) {
    let msg = format!(
        "integer constant out of range for binary `{op}`\nvalue `{val}` is outside `{min}..={max}` range",
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

//==================== TYPECHECK ====================

pub fn tycheck_type_mismatch(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    expect_src: Option<SourceRange>,
    expected_ty: &str,
    found_ty: &str,
) {
    let msg = format!("type mismatch: expected `{expected_ty}`, found `{found_ty}`");
    let info = expect_src.map(|src| Info::new_val("expected due to this", src));
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_expected_integer(emit: &mut impl ErrorSink, src: SourceRange, found_ty: &str) {
    let msg = format!("expected integer type, found `{found_ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_expected_boolean(emit: &mut impl ErrorSink, src: SourceRange, found_ty: &str) {
    let msg = format!("expected boolean type, found `{found_ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_if_missing_else(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "`if` expression is missing an `else` block\n`if` without `else` evaluates to `void` and cannot return a value";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_match_on_ty(emit: &mut impl ErrorSink, src: SourceRange, ty: &str) {
    let msg = format!("cannot match on value of type `{ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_const_field_access(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot access fields in patterns";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_const_with_bindings(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "constant variables in patterns cannot have bindings";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_const_enum(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "constant variables in patterns cannot be enums";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_runtime_value(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot use runtime values in patterns";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_pat_in_or_bindings(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot use named bindings in `or` patterns, use `_` instead";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_field_not_found_ty(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    field_name: &str,
    ty: &str,
) {
    let msg = format!("no field `{field_name}` exists on value of type `{ty}`");
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

pub fn tycheck_field_not_found_string(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    field_name: &str,
) {
    let msg = format!("no field `{field_name}` exists on string type\ndid you mean `len`?");
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

pub fn tycheck_cannot_index_on_ty(emit: &mut impl ErrorSink, src: SourceRange, ty: &str) {
    let msg = format!("cannot index value of type `{ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_slice_on_type(emit: &mut impl ErrorSink, src: SourceRange, ty: &str) {
    let msg = format!("cannot slice value of type `{ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cast_invalid(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    from_ty: &str,
    into_ty: &str,
) {
    let msg = format!("cannot cast from `{from_ty}` into `{into_ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_const_poly_dep(emit: &mut impl ErrorSink, src: SourceRange, ty: &str, name: &str) {
    let msg = format!(
        "constant {name} cannot use `{ty}` layout of which depends on polymorphic parameters"
    );
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_transmute_poly_dep(emit: &mut impl ErrorSink, src: SourceRange, ty: &str) {
    let msg =
        format!("transmute cannot use `{ty}` layout of which depends on polymorphic parameters");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_transmute_mismatch(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    subject: &str,
    from_ty: &str,
    from_val: u64,
    into_ty: &str,
    into_val: u64,
) {
    let msg = format!(
        "{subject} mismatch in transmute: `{from_ty}` = {from_val} vs `{into_ty}` = {into_val}"
    );
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_field_already_initialized(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    prev_src: SourceRange,
    field_name: &str,
) {
    let msg = format!("field `{field_name}` already initialized");
    let info = Info::new("initialized here", prev_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_missing_field_initializers(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    struct_src: SourceRange,
    message: String,
) {
    let msg = message;
    let info = Info::new("struct defined here", struct_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_field_init_out_of_order(
    emit: &mut impl WarningSink,
    src: SourceRange,
    init_name: &str,
    expect_name: &str,
) {
    let msg =
        format!("field `{init_name}` initialized out of order, expected `{expect_name}` first");
    emit.warning(Warning::new(msg, src, None));
}

pub fn tycheck_cannot_deref_on_ty(emit: &mut impl ErrorSink, src: SourceRange, ty: &str) {
    let msg = format!("cannot dereference value of type `{ty}`");
    emit.error(Error::new(msg, src, None));
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

pub fn tycheck_cannot_iter_on_type(emit: &mut impl ErrorSink, src: SourceRange, ty: &str) {
    let msg = format!("cannot iterate on value of type `{ty}`\nonly arrays and slices support by element iteration");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_for_range_ref(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "for range loops don't support by reference iteration";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_for_range_reverse(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "for range loops don't support reverse iteration";
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_for_range_type_mismatch(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    lhs_ty: &str,
    rhs_ty: &str,
) {
    let msg = format!("type mismatch in for range loop: `{lhs_ty}` vs `{rhs_ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_cannot_infer_local_type(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot infer local variable type";
    emit.error(Error::new(msg, src, None));
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

pub fn tycheck_cannot_infer_poly_params(emit: &mut impl ErrorSink, src: SourceRange) {
    let msg = "cannot infer polymorphic type parameters";
    emit.error(Error::new(msg, src, None));
}

//==================== TYPECHECK CALL & INPUT ====================

pub fn tycheck_intrinsic_proc_ptr(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    proc_src: SourceRange,
) {
    let msg = format!("cannot take a pointer to an #intrinsic procedure");
    let info = Info::new("procedure defined here", proc_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_cannot_call_value_of_type(emit: &mut impl ErrorSink, src: SourceRange, ty: &str) {
    let msg = format!("cannot call value of type `{ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_unexpected_proc_arg_count(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    proc_src: Option<SourceRange>,
    variadic: bool,
    input_count: usize,
    expected_count: usize,
) {
    let at_least = if variadic { " at least" } else { "" };
    let plural = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected{at_least} {expected_count} argument{plural}, found {input_count}");
    let info = proc_src.map(|src| Info::new_val("procedure defined here", src));
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
    let plural = if expected_count == 1 { "" } else { "s" };
    let msg = format!("expected {expected_count} argument{plural}, found {input_count}");
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

//==================== TYPECHECK OPERATOR ====================

pub fn tycheck_un_op_cannot_apply(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    rhs_ty: &str,
) {
    let msg = format!("cannot apply unary operator `{op}` on value of type `{rhs_ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_bin_cannot_apply(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    op: &'static str,
    lhs_ty: &str,
    rhs_ty: &str,
) {
    let msg =
        format!("cannot apply binary operator `{op}` on values of type `{lhs_ty}` and `{rhs_ty}`");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_bin_type_mismatch(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    lhs_ty: &str,
    rhs_ty: &str,
) {
    let msg = format!("type mismatch in binary expression: `{lhs_ty}` vs `{rhs_ty}`");
    emit.error(Error::new(msg, src, None));
}

//==================== TYPECHECK ADDRESSABILITY ====================

pub fn tycheck_addr(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    action: &'static str,
    subject: &'static str,
) {
    let msg = format!("cannot {action} to a {subject}");
    emit.error(Error::new(msg, src, None));
}

pub fn tycheck_addr_const(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    const_src: SourceRange,
    action: &'static str,
) {
    let msg = format!("cannot {action} to a constant, you can use global variable instead");
    let info = Info::new("constant defined here", const_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_index_const(emit: &mut impl ErrorSink, src: SourceRange, const_src: SourceRange) {
    let msg =
        "cannot index a constant with non-constant value, you can use global variable instead";
    let info = Info::new("constant defined here", const_src);
    emit.error(Error::new(msg, src, info));
}

pub fn tycheck_addr_variable(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    var_src: SourceRange,
    action: &'static str,
) {
    let msg = format!("cannot {action} to an immutable variable");
    let info = Info::new("variable defined here", var_src);
    emit.error(Error::new(msg, src, info));
}

//==================== ENTRY POINT ====================

pub fn entry_main_mod_not_found(emit: &mut impl ErrorSink) {
    let msg = "could not find `main` module, expected `src/main.rock` to exist";
    emit.error(Error::message(msg));
}

pub fn entry_main_proc_not_found(emit: &mut impl ErrorSink) {
    let msg =
        "could not find entry point in `src/main.rock`\ndefine it like this: `proc main() void {}`";
    emit.error(Error::message(msg));
}

//==================== BACKEND ====================

pub fn backend_module_verify_failed(error: String) -> Error {
    let msg = format!("llvm module verify failed, this is likely a compiler bug:\n{error}");
    Error::message(msg)
}

pub fn backend_emit_object_failed(error: String) -> Error {
    let msg = format!("llvm emit object failed, this is likely a compiler bug:\n{error}");
    Error::message(msg)
}

pub fn backend_link_command_failed(error: String, linker_path: &PathBuf) -> Error {
    let path = linker_path.to_string_lossy();
    let msg = format!("link command failed, expected linker path: `{path}`\nreason: {error}");
    Error::message(msg)
}

pub fn backend_run_command_failed(error: String, bin_path: &PathBuf) -> Error {
    let path = bin_path.to_string_lossy();
    let msg = format!("run command failed, expected binary path: `{path}`\nreason: {error}");
    Error::message(msg)
}

pub fn backend_link_failed(output: std::process::Output, args: Vec<String>) -> Error {
    use std::fmt::Write;
    let mut msg = String::with_capacity(512);
    msg.push_str("failed to link the program:\n");

    let code = match output.status.code() {
        Some(code) => code.to_string(),
        None => "<unknown>".to_string(),
    };
    let _ = write!(&mut msg, "status code: {code}\n\n");

    if !output.stderr.is_empty() {
        let _ = write!(&mut msg, "stderr output:\n{}\n", String::from_utf8_lossy(&output.stderr));
    }
    if !output.stdout.is_empty() {
        let _ = write!(&mut msg, "stdout output:\n{}\n", String::from_utf8_lossy(&output.stdout));
    }

    msg.push_str("used linker arguments:\n");
    for arg in args {
        let _ = writeln!(&mut msg, "\"{}\"", arg.as_str());
    }
    msg.pop();

    Error::message(msg)
}

//==================== INTERNAL ====================

pub fn internal_not_implemented(
    emit: &mut impl ErrorSink,
    src: SourceRange,
    feature: &'static str,
) {
    let msg = format!("internal: {feature} is not implemented");
    emit.error(Error::new(msg, src, None));
}
