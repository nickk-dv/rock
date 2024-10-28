#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SyntaxKind {
    ERROR,
    TOMBSTONE,
    SOURCE_FILE,

    ATTR_LIST,
    ATTR,
    ATTR_PARAM_LIST,
    ATTR_PARAM,
    VISIBILITY,

    PROC_ITEM,
    PARAM_LIST,
    PARAM,
    ENUM_ITEM,
    VARIANT_LIST,
    VARIANT,
    VARIANT_FIELD_LIST,
    STRUCT_ITEM,
    FIELD_LIST,
    FIELD,
    CONST_ITEM,
    GLOBAL_ITEM,
    IMPORT_ITEM,
    IMPORT_PATH,
    IMPORT_SYMBOL_LIST,
    IMPORT_SYMBOL,
    IMPORT_SYMBOL_RENAME,

    GENERIC_PARAMS,
    GENERIC_TYPES,

    TYPE_BASIC,
    TYPE_CUSTOM,
    TYPE_GENERIC,
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
    STMT_LOOP,
    LOOP_WHILE_HEADER,
    LOOP_CLIKE_HEADER,
    STMT_LOCAL,
    STMT_ASSIGN,
    STMT_EXPR_SEMI,
    STMT_EXPR_TAIL,
    STMT_ATTR_STMT,

    EXPR_PAREN,
    EXPR_IF,
    IF_BRANCH,
    EXPR_MATCH,
    MATCH_ARM_LIST,
    MATCH_ARM,
    EXPR_FIELD,
    EXPR_INDEX,
    EXPR_SLICE,
    EXPR_CALL,
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
    EXPR_UNARY,
    EXPR_BINARY,

    PAT_WILD,
    PAT_LIT,
    PAT_ITEM,
    PAT_VARIANT,
    PAT_OR,

    LIT_VOID,
    LIT_NULL,
    LIT_BOOL,
    LIT_INT,
    LIT_FLOAT,
    LIT_CHAR,
    LIT_STRING,

    RANGE_FULL,
    RANGE_TO_EXCLUSIVE,
    RANGE_TO_INCLUSIVE,
    RANGE_FROM,
    RANGE_EXCLUSIVE,
    RANGE_INCLUSIVE,

    NAME,
    PATH,
    BIND,
    BIND_LIST,
    ARGS_LIST,
}

#[derive(Clone, Copy)]
pub struct SyntaxSet {
    mask: u128,
}

impl SyntaxSet {
    pub const fn new(syntax: &[SyntaxKind]) -> SyntaxSet {
        let mut mask = 0u128;
        let mut i = 0;
        while i < syntax.len() {
            mask |= 1u128 << syntax[i] as u8;
            i += 1;
        }
        SyntaxSet { mask }
    }
    #[inline]
    pub const fn empty() -> SyntaxSet {
        SyntaxSet { mask: 0 }
    }
    #[inline]
    pub const fn combine(self, other: SyntaxSet) -> SyntaxSet {
        SyntaxSet {
            mask: self.mask | other.mask,
        }
    }
    #[inline]
    pub const fn contains(&self, syntax: SyntaxKind) -> bool {
        self.mask & 1u128 << syntax as u8 != 0
    }
}
