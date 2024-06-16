use crate::arena::Arena;
use crate::id_impl;
use crate::token::token_list::TokenList;

pub struct SyntaxTree<'syn> {
    root: Node<'syn>,
    arena: Arena<'syn>,
    nodes: Vec<Node<'syn>>,
    tokens: TokenList,
}

pub struct Node<'syn> {
    kind: SyntaxKind,
    content: &'syn [NodeOrToken],
}

id_impl!(NodeID);
id_impl!(TokenID);
pub enum NodeOrToken {
    Node(NodeID),
    Token(TokenID),
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum SyntaxKind {
    SOURCE_FILE,
    TOMBSTONE,
    ERROR,

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
    IMPORT_SYMBOL_LIST,
    IMPORT_SYMBOL,

    VISIBILITY,
    NAME,
    NAME_REF,
    ATTRIBUTE,
    PATH,

    TYPE_BASIC,
    TYPE_CUSTOM,
    TYPE_REFERENCE,
    TYPE_PROCEDURE,
    TYPE_ARRAY_SLICE,
    TYPE_ARRAY_STATIC,

    STMT_BREAK,
    STMT_CONTINUE,
    STMT_RETURN,
    STMT_DEFER,
    STMT_LOOP,
    STMT_LOCAL,
    STMT_ASSIGN,
    STMT_EXPR_SEMI,
    STMT_EXPR_TAIL,

    EXPR_LIT_NULL,
    EXPR_LIT_BOOL,
    EXPR_LIT_INT,
    EXPR_LIT_FLOAT,
    EXPR_LIT_CHAR,
    EXPR_LIT_STRING,
    EXPR_IF,
    EXPR_BLOCK,
    STMT_LIST,
    EXPR_MATCH,
    MATCH_ARM_LIST,
    MATCH_ARM,
    EXPR_FIELD,
    EXPR_INDEX,
    EXPR_SLICE,
    EXPR_CALL,
    ARGUMENT_LIST,
    EXPR_CAST,
    EXPR_SIZEOF,
    EXPR_ITEM,
    EXPR_VARIANT,
    EXPR_STRUCT_INIT,
    EXPR_ARRAY_INIT,
    EXPR_ARRAY_REPEAT,
    EXPR_ADDRESS,
    EXPR_UNARY,
    EXPR_BINARY,
}
