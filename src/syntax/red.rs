use super::{
    green::{GreenElement, GreenNode, GreenToken},
    kinds::SyntaxKind,
    NodeOrToken,
};
use std::{fmt, rc::Rc, sync::Arc};

pub type RedNode = Rc<RedNodeData>;
pub type RedToken = Rc<RedTokenData>;
pub type RedElement = NodeOrToken<RedNode, RedToken>;

#[derive(Clone)]
pub struct RedNodeData {
    parent: Option<RedNode>,
    idx_in_parent: usize,
    green: GreenNode,
    text_offset: usize,
}

#[derive(Clone)]
pub struct RedTokenData {
    parent: Option<RedNode>,
    green: GreenToken,
    text_offset: usize,
}

impl RedNodeData {
    pub fn new(root: GreenNode) -> RedNode {
        Rc::new(RedNodeData {
            parent: None,
            idx_in_parent: 0, //@todo?
            green: root,
            text_offset: 0, //@todo?
        })
    }

    fn parent(&self) -> Option<&RedNode> {
        self.parent.as_ref()
    }
    fn green(&self) -> &GreenNode {
        &self.green
    }
    fn text_offset(&self) -> usize {
        self.text_offset
    }

    fn kind(&self) -> SyntaxKind {
        self.green.kind()
    }
    fn text_len(&self) -> usize {
        self.green.text_len()
    }

    pub fn children<'a>(self: &'a RedNode) -> impl Iterator<Item = RedElement> + 'a {
        let mut offset_in_parent = 0;

        self.green()
            .children()
            .enumerate()
            .map(move |(idx_in_parent, green_child)| {
                let text_offset = self.text_offset();
                offset_in_parent += green_child.text_len(); //@todo?

                match green_child {
                    NodeOrToken::Node(node) => Rc::new(RedNodeData {
                        parent: Some(Rc::clone(self)),
                        idx_in_parent,
                        text_offset,
                        green: node,
                    })
                    .into(),
                    NodeOrToken::Token(token) => Rc::new(RedTokenData {
                        parent: Some(Rc::clone(self)),
                        text_offset,
                        green: token,
                    })
                    .into(),
                }
            })
    }

    pub fn replace_child(self: &RedNode, idx: usize, new_child: GreenElement) -> RedNode {
        let new_green = self.green().replace_child(idx, new_child);
        self.replace_ourselves(Arc::new(new_green))
    }

    fn replace_ourselves(self: &RedNode, new_green: GreenNode) -> RedNode {
        match self.parent() {
            Some(parent) => parent.replace_child(self.idx_in_parent, new_green.into()),
            None => RedNodeData::new(new_green),
        }
    }
}

impl RedTokenData {
    fn parent(&self) -> Option<&RedNode> {
        self.parent.as_ref()
    }
    fn green(&self) -> &GreenToken {
        &self.green
    }
    fn text_offset(&self) -> usize {
        self.text_offset
    }

    fn kind(&self) -> SyntaxKind {
        self.green.kind()
    }
    fn text(&self) -> &str {
        self.green.text()
    }
    fn text_len(&self) -> usize {
        self.green.text_len()
    }
}

impl RedElement {
    pub fn parent(&self) -> Option<&RedNode> {
        match self {
            NodeOrToken::Node(it) => it.parent(),
            NodeOrToken::Token(it) => it.parent(),
        }
    }
    pub fn text_offset(&self) -> usize {
        match self {
            NodeOrToken::Node(it) => it.text_offset(),
            NodeOrToken::Token(it) => it.text_offset(),
        }
    }
    pub fn kind(&self) -> SyntaxKind {
        match self {
            NodeOrToken::Node(it) => it.kind(),
            NodeOrToken::Token(it) => it.kind(),
        }
    }
    pub fn text_len(&self) -> usize {
        match self {
            NodeOrToken::Node(it) => it.text_len(),
            NodeOrToken::Token(it) => it.text_len(),
        }
    }
}

impl From<RedNode> for RedElement {
    fn from(value: RedNode) -> Self {
        Self::Node(value)
    }
}

impl From<RedToken> for RedElement {
    fn from(value: RedToken) -> Self {
        Self::Token(value)
    }
}

impl fmt::Display for RedNodeData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.green(), f)
    }
}

impl fmt::Display for RedTokenData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.green(), f)
    }
}

impl fmt::Display for RedElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeOrToken::Node(it) => fmt::Display::fmt(it, f),
            NodeOrToken::Token(it) => fmt::Display::fmt(it, f),
        }
    }
}
