use crate::ast::token::Token;
use crate::text::*;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Debug)]
pub struct SyntaxTree {
    pub source: String,
    pub root: SyntaxNode,
}

pub type SyntaxNode = Rc<SyntaxNodeData>;
#[derive(Debug)]
pub struct SyntaxNodeData {
    kind: SyntaxNodeKind,
    range: Cell<TextRange>,
    children: Vec<SyntaxElement>,
}

#[derive(Debug)]
pub struct SyntaxTokenData {
    token: Token,
    range: TextRange,
}

#[derive(Debug)]
pub enum SyntaxElement {
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

    pub fn to_string(&self) -> String {
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

    pub fn format_to_string(&self, ansi: bool) -> String {
        let mut string = if ansi { "\n".into() } else { "".into() };
        let depth: u32 = 0;
        self.children_format(ansi, depth, &self.root, &mut string);
        string
    }

    fn children_format(
        &self,
        ansi: bool,
        depth: u32,
        node_data: &SyntaxNodeData,
        string: &mut String,
    ) {
        for _ in 0..depth {
            string.push_str("  ");
        }

        use crate::error::ansi;

        let format = if ansi {
            format!(
                "{}{:?}{}@{:?}{}\n",
                ansi::YELLOW_BOLD,
                node_data.kind,
                ansi::MAGENTA,
                node_data.range.get(),
                ansi::CLEAR
            )
        } else {
            format!("{:?}@{:?}\n", node_data.kind, node_data.range.get())
        };
        string.push_str(format.as_str());

        for element in node_data.children.iter() {
            match element {
                SyntaxElement::Node(node) => {
                    self.children_format(ansi, depth + 1, &node, string);
                }
                SyntaxElement::Token(token) => {
                    for _ in 0..=depth {
                        string.push_str("  ");
                    }
                    let token_string = &self.source[token.range().as_usize()];
                    let format = if ansi {
                        format!(
                            "{:?}{}@{:?} {}{:?}{}\n",
                            token.token,
                            ansi::MAGENTA,
                            token.range,
                            ansi::GREEN,
                            token_string,
                            ansi::CLEAR
                        )
                    } else {
                        format!("{:?}@{:?} {:?}\n", token.token, token.range, token_string)
                    };
                    string.push_str(format.as_str());
                }
            }
        }
    }
}

#[repr(u8)]
#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug)]
pub enum SyntaxNodeKind {
    SOURCE_FILE,
    TOMBSTONE,
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
    USE_RENAME,
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
    PROC_ARG_LIST,
    EXPR_STRUCT_INIT,
    EXPR_ARRAY_INIT,
    EXPR_ARRAY_REPEAT,
    EXPR_UN,
    EXPR_BIN,
}

pub struct SyntaxTreeBuilder {
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

    pub fn token_(&mut self, token: Token, range: TextRange) {
        self.children
            .push(SyntaxElement::Token(SyntaxTokenData::new(token, range)))
    }

    pub fn token(&mut self, token: (Token, TextRange)) {
        self.children
            .push(SyntaxElement::Token(SyntaxTokenData::new(token.0, token.1)))
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
