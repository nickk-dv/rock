#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(non_camel_case_types)]
pub enum SyntaxKind {
    ERROR,
    TOMBSTONE,
    SOURCE_FILE,

    ATTRIBUTE_LIST,
    ATTRIBUTE,
    VISIBILITY,
    PROC_ITEM,
    PARAM_LIST,
    PARAM,
    ENUM_ITEM,
    VARIANT_LIST,
    VARIANT,
    STRUCT_ITEM,
    FIELD_LIST,
    FIELD,
    CONST_ITEM,
    GLOBAL_ITEM,
    IMPORT_ITEM,
    IMPORT_PATH,
    IMPORT_SYMBOL_LIST,
    IMPORT_SYMBOL,
    SYMBOL_RENAME,

    NAME,
    PATH,

    TYPE_BASIC,
    TYPE_CUSTOM,
    TYPE_REFERENCE,
    TYPE_PROCEDURE,
    PARAM_TYPE_LIST,
    TYPE_ARRAY_SLICE,
    TYPE_ARRAY_STATIC,

    BLOCK,
    STMT_BREAK,
    STMT_CONTINUE,
    STMT_RETURN,
    STMT_DEFER,
    SHORT_BLOCK,
    STMT_LOOP,
    LOOP_WHILE_HEADER,
    LOOP_CLIKE_HEADER,
    STMT_LOCAL,
    STMT_ASSIGN,
    STMT_EXPR_SEMI,
    STMT_EXPR_TAIL,

    EXPR_PAREN,
    EXPR_LIT_NULL,
    EXPR_LIT_BOOL,
    EXPR_LIT_INT,
    EXPR_LIT_FLOAT,
    EXPR_LIT_CHAR,
    EXPR_LIT_STRING,
    EXPR_IF,
    ENTRY_BRANCH,
    ELSE_IF_BRANCH,
    EXPR_BLOCK,
    EXPR_MATCH,
    MATCH_ARM_LIST,
    MATCH_ARM,
    MATCH_FALLBACK,
    EXPR_FIELD,
    EXPR_INDEX,
    EXPR_CALL,
    CALL_ARGUMENT_LIST,
    EXPR_CAST,
    EXPR_SIZEOF,
    EXPR_ITEM,
    EXPR_VARIANT,
    EXPR_STRUCT_INIT,
    FIELD_INIT_LIST,
    FIELD_INIT,
    EXPR_ARRAY_INIT,
    EXPR_ARRAY_REPEAT,
    EXPR_DEREF,
    EXPR_ADDRESS,
    EXPR_RANGE_FULL,
    EXPR_RANGE_TO,
    EXPR_RANGE_TO_INCLUSIVE,
    EXPR_RANGE_FROM,
    EXPR_RANGE,
    EXPR_RANGE_INCLUSIVE,
    EXPR_UNARY,
    EXPR_BINARY,
}
