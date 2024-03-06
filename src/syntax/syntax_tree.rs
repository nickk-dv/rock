use crate::ast::token::Token;
use crate::text_range::*;
use std::rc::Rc;

struct SyntaxTree {
    source: String,
    root: SyntaxNode,
}

type SyntaxNode = Rc<SyntaxNodeData>;
struct SyntaxNodeData {
    kind: SyntaxNodeKind,
    parent: Option<SyntaxNode>,
    children: Vec<SyntaxElement>,
    range: TextRange,
}

struct SyntaxTokenData {
    kind: Token,
    range: TextRange,
}

enum SyntaxElement {
    Node(SyntaxNode),
    Token(SyntaxTokenData),
}

impl SyntaxNodeData {
    fn new(
        kind: SyntaxNodeKind,
        parent: Option<SyntaxNode>,
        children: Vec<SyntaxElement>,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            parent,
            children,
            range,
        }
    }
    fn kind(&self) -> SyntaxNodeKind {
        self.kind
    }
    fn range(&self) -> TextRange {
        self.range
    }
    fn parent<'a>(self: &'a SyntaxNode) -> Option<&'a SyntaxNode> {
        self.parent.as_ref()
    }
    fn children<'a>(self: &'a SyntaxNode) -> impl Iterator<Item = &'a SyntaxElement> {
        self.children.iter()
    }
}

impl SyntaxTokenData {
    fn new(kind: Token, range: TextRange) -> Self {
        Self { kind, range }
    }
    fn kind(&self) -> Token {
        self.kind
    }
    fn range(&self) -> TextRange {
        self.range
    }
}

impl SyntaxElement {
    fn range(&self) -> TextRange {
        match self {
            SyntaxElement::Node(it) => it.range(),
            SyntaxElement::Token(it) => it.range(),
        }
    }
}

impl From<SyntaxNode> for SyntaxElement {
    fn from(value: SyntaxNode) -> Self {
        Self::Node(value)
    }
}

impl From<SyntaxTokenData> for SyntaxElement {
    fn from(value: SyntaxTokenData) -> Self {
        Self::Token(value)
    }
}

impl SyntaxTree {
    fn new(source: String, root: SyntaxNode) -> Self {
        Self { source, root }
    }

    fn to_string(&self) -> String {
        let mut string = String::new();
        self.children_to_string(&self.root, &mut string);
        string
    }

    fn children_to_string(&self, node_data: &SyntaxNodeData, string: &mut String) {
        for element in node_data.children.iter() {
            match element {
                SyntaxElement::Node(node) => self.children_to_string(&node, string),
                SyntaxElement::Token(token) => {
                    let range = token.range().start().into()..token.range().end().into();
                    string.push_str(&self.source[range]);
                }
            }
        }
    }
}

#[test]
fn test_syntax_tree() {
    let three = SyntaxTokenData::new(Token::IntLit, TextRange::new(0.into(), 1.into()));
    let ws = SyntaxTokenData::new(Token::Whitespace, TextRange::new(1.into(), 2.into()));
    let star = SyntaxTokenData::new(Token::Star, TextRange::new(2.into(), 3.into()));
    let ws2 = SyntaxTokenData::new(Token::Whitespace, TextRange::new(3.into(), 4.into()));
    let four = SyntaxTokenData::new(Token::IntLit, TextRange::new(4.into(), 5.into()));
    let mul_expr = SyntaxNodeData::new(
        SyntaxNodeKind::EXPR_BIN,
        None,
        vec![
            three.into(),
            ws.into(),
            star.into(),
            ws2.into(),
            four.into(),
        ],
        TextRange::new(0.into(), 5.into()),
    );
    let tree = SyntaxTree::new("3 * 4".into(), Rc::new(mul_expr));
    let tree_string = tree.to_string();
    assert_eq!(tree.source, tree_string);
    println!("{}", tree_string);
}

#[repr(u8)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
enum SyntaxNodeKind {
    SOURCE_FILE,
    ERROR,

    USE_DECL,
    MOD_DECL,
    PROC_DECL,
    ENUM_DECL,
    UNION_DECL,
    STRUCT_DECL,
    CONST_DECL,
    GLOBAL_DECL,

    USE_SYMBOL_LIST,
    PROC_PARAM_LIST,
    ENUM_VARIANT_LIST,
    UNION_MEMBER_LIST,
    STRUCT_FIELD_LIST,

    USE_SYMBOL,
    PROC_PARAM,
    ENUM_VARIANT,
    UNION_MEMBER,
    STRUCT_FIELD,

    NAME,
    NAME_REF,
    PATH,
    TYPE,
    TYPE_ARRAY_SLICE,
    TYPE_ARRAY_STATIC,

    STMT_LIST,
    STMT,
    STMT_RETURN,
    STMT_DEFER,
    STMT_FOR,
    STMT_VAR_DECL,
    STMT_VAR_ASSIGN,
    STMT_EXPR,

    EXPR_UNIT,
    EXPR_LIT,
    EXPR_IF,
    EXPR_BLOCK,
    EXPR_MATCH,
    EXPR_FIELD,
    EXPR_INDEX,
    EXPR_CAST,
    EXPR_SIZEOF,
    EXPR_ITEM,
    EXPR_PROC_CALL,
    PROC_CALL_ARG_LIST,
    EXPR_STRUCT_INIT,
    EXPR_ARRAY_INIT,
    EXPR_ARRAY_REPEAT,
    EXPR_UN,
    EXPR_BIN,
}
