use super::syntax_tree::SyntaxNodeKind;
use crate::ast::token::Token;

#[derive(Debug)]
pub enum Event {
    StartNode { kind: SyntaxNodeKind },
    EndNode,
    Token { token: Token },
    Error { message: String },
}
