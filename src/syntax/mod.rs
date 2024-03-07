mod event;
mod grammar;
mod green;
mod kinds;
mod parser;
mod parser_api;
mod red;
mod syntax_tree;
mod token_set;

pub use self::{
    green::{GreenNode, GreenNodeData, GreenToken, GreenTokenData},
    red::{RedNode, RedNodeData, RedToken, RedTokenData},
};
use kinds::SyntaxKind;
use std::sync::Arc;

#[derive(Copy, Clone)]
pub enum NodeOrToken<N, T> {
    Node(N),
    Token(T),
}

impl<N, T> NodeOrToken<N, T> {
    pub fn into_node(self) -> Option<N> {
        match self {
            NodeOrToken::Node(it) => Some(it),
            NodeOrToken::Token(_) => None,
        }
    }
    pub fn into_token(self) -> Option<T> {
        match self {
            NodeOrToken::Node(_) => None,
            NodeOrToken::Token(it) => Some(it),
        }
    }
}
