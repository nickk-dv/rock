use crate::ast::token::Token;
use crate::text_range::*;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Debug)]
struct SyntaxTree {
    source: String,
    root: SyntaxNode,
}

type SyntaxNode = Rc<SyntaxNodeData>;
#[derive(Debug)]
struct SyntaxNodeData {
    kind: SyntaxNodeKind,
    range: Cell<TextRange>,
    children: Vec<SyntaxElement>,
}

#[derive(Debug)]
struct SyntaxTokenData {
    token: Token,
    range: TextRange,
}

#[derive(Debug)]
enum SyntaxElement {
    Node(SyntaxNode),
    Token(SyntaxTokenData),
}

impl SyntaxNodeData {
    fn new(kind: SyntaxNodeKind, range: TextRange, children: Vec<SyntaxElement>) -> Self {
        Self {
            kind,
            range: Cell::new(range),
            children,
        }
    }
    fn kind(&self) -> SyntaxNodeKind {
        self.kind
    }
    fn range(&self) -> TextRange {
        self.range.get()
    }
    fn children<'a>(self: &'a SyntaxNode) -> impl Iterator<Item = &'a SyntaxElement> {
        self.children.iter()
    }
}

impl SyntaxTokenData {
    fn new(token: Token, range: TextRange) -> Self {
        Self { token, range }
    }
    fn token(&self) -> Token {
        self.token
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
        TextRange::new(0.into(), 5.into()),
        vec![
            three.into(),
            ws.into(),
            star.into(),
            ws2.into(),
            four.into(),
        ],
    );
    let tree = SyntaxTree::new("3 * 4".into(), Rc::new(mul_expr));
    let tree_string = tree.to_string();
    assert_eq!(tree.source, tree_string);
    println!("{}", tree_string);
}

#[test]
fn test_syntax_tree_build() {
    let three = SyntaxTokenData::new(Token::IntLit, TextRange::new(0.into(), 1.into()));
    let ws = SyntaxTokenData::new(Token::Whitespace, TextRange::new(1.into(), 2.into()));
    let star = SyntaxTokenData::new(Token::Star, TextRange::new(2.into(), 3.into()));
    let ws2 = SyntaxTokenData::new(Token::Whitespace, TextRange::new(3.into(), 4.into()));
    let four = SyntaxTokenData::new(Token::IntLit, TextRange::new(4.into(), 5.into()));
    let name = SyntaxTokenData::new(Token::Ident, TextRange::new(5.into(), 9.into()));

    let mut builder = SyntaxTreeBuilder::new();
    builder.start_node(SyntaxNodeKind::SOURCE_FILE);
    builder.start_node(SyntaxNodeKind::EXPR_BIN);
    builder.token(three.token, three.range);
    builder.token(ws.token, ws.range);
    builder.token(star.token, star.range);
    builder.token(ws2.token, ws2.range);
    builder.token(four.token, four.range);
    builder.end_node();
    builder.start_node(SyntaxNodeKind::NAME);
    builder.token(name.token, name.range);
    builder.end_node();
    builder.end_node();

    let tree = SyntaxTree::new("5 * 6name".into(), builder.finish());
    let tree_string = tree.to_string();
    assert_eq!(tree.source, tree_string);
    println!("{}", tree_string);
    println!("{:#?}", tree);
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

struct SyntaxTreeBuilder {
    parents: Vec<(SyntaxNodeKind, usize)>,
    children: Vec<SyntaxElement>,
}

impl SyntaxTreeBuilder {
    pub fn new() -> Self {
        Self {
            parents: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn start_node(&mut self, kind: SyntaxNodeKind) {
        let first_child = self.children.len();
        self.parents.push((kind, first_child))
    }

    pub fn end_node(&mut self) {
        let (kind, first_child) = self.parents.pop().unwrap();
        let node = SyntaxNodeData::new(
            kind,
            TextRange::empty_at(0.into()),
            self.children.drain(first_child..).collect(),
        );
        self.children.push(SyntaxElement::Node(Rc::new(node)));
    }

    pub fn token(&mut self, token: Token, range: TextRange) {
        self.children
            .push(SyntaxElement::Token(SyntaxTokenData::new(token, range)))
    }

    pub fn finish(mut self) -> SyntaxNode {
        assert_eq!(self.children.len(), 1);
        let root = match self.children.pop().unwrap() {
            SyntaxElement::Node(node) => node,
            SyntaxElement::Token(_) => panic!(),
        };
        Self::compute_ranges(&root, TextRange::empty_at(0.into()));
        root
    }

    fn compute_ranges(node: &SyntaxNodeData, mut range: TextRange) {
        for child in node.children.iter() {
            match child {
                SyntaxElement::Node(node) => {
                    Self::compute_ranges(&node, TextRange::empty_at(range.end()));
                    range = TextRange::new(range.start(), node.range.get().end());
                }
                SyntaxElement::Token(token) => {
                    range = TextRange::new(range.start(), token.range.end())
                }
            }
        }
        node.range.set(range);
    }
}
