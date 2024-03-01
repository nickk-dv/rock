use super::{kinds::SyntaxKind, NodeOrToken};
use std::{fmt, iter, sync::Arc};

pub type GreenNode = Arc<GreenNodeData>;
pub type GreenToken = Arc<GreenTokenData>;
pub type GreenElement = NodeOrToken<GreenNode, GreenToken>;

pub struct GreenNodeData {
    kind: SyntaxKind,
    children: Vec<GreenElement>,
    len: usize,
}

pub struct GreenTokenData {
    kind: SyntaxKind,
    text: String,
}

impl GreenNodeData {
    pub fn new(kind: SyntaxKind, children: Vec<GreenElement>) -> GreenNodeData {
        let len = children.iter().map(|it| it.text_len()).sum();
        GreenNodeData {
            kind,
            children,
            len,
        }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }
    pub fn text_len(&self) -> usize {
        self.len
    }
    pub fn children<'a>(&'a self) -> impl Iterator<Item = GreenElement> + '_ {
        self.children.iter().cloned()
    }

    pub fn replace_child(&self, idx: usize, new_child: GreenElement) -> GreenNodeData {
        assert!(idx < self.children.len());
        let lhs_children = self.children().take(idx);
        let rhs_children = self.children().skip(idx + 1);
        let new_children = lhs_children
            .chain(iter::once(new_child))
            .chain(rhs_children)
            .collect();
        GreenNodeData::new(self.kind, new_children)
    }
}

impl GreenTokenData {
    pub fn new(kind: SyntaxKind, text: String) -> GreenTokenData {
        GreenTokenData { kind, text }
    }

    pub fn kind(&self) -> SyntaxKind {
        self.kind
    }
    pub fn text(&self) -> &str {
        self.text.as_str()
    }
    pub fn text_len(&self) -> usize {
        self.text.len()
    }
}

impl GreenElement {
    fn kind(&self) -> SyntaxKind {
        match self {
            NodeOrToken::Node(node) => node.kind(),
            NodeOrToken::Token(token) => token.kind(),
        }
    }
    pub fn text_len(&self) -> usize {
        match self {
            NodeOrToken::Node(node) => node.text_len(),
            NodeOrToken::Token(token) => token.text_len(),
        }
    }
}

impl From<GreenNode> for GreenElement {
    fn from(value: GreenNode) -> Self {
        Self::Node(value)
    }
}

impl From<GreenToken> for GreenElement {
    fn from(value: GreenToken) -> Self {
        Self::Token(value)
    }
}

impl fmt::Display for GreenNodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for child in self.children() {
            fmt::Display::fmt(&child, f)?;
        }
        Ok(())
    }
}
impl fmt::Display for GreenTokenData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.text(), f)
    }
}

impl fmt::Display for GreenElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeOrToken::Node(it) => fmt::Display::fmt(it, f),
            NodeOrToken::Token(it) => fmt::Display::fmt(it, f),
        }
    }
}
