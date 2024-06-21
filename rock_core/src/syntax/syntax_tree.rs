use super::parser::Event;
use super::syntax_kind::SyntaxKind;
use crate::arena::Arena;
use crate::error::{ErrorComp, SourceRange};
use crate::id_impl;
use crate::session::FileID;
use crate::temp_buffer::TempBuffer;
use crate::text::TextRange;
use crate::token::token_list::TokenList;
use crate::token::Token;

pub struct SyntaxTree<'syn> {
    #[allow(unused)]
    arena: Arena<'syn>,
    nodes: Vec<Node<'syn>>,
    tokens: TokenList,
}

id_impl!(NodeID);
pub struct Node<'syn> {
    pub kind: SyntaxKind,
    pub content: &'syn [NodeOrToken],
}

id_impl!(TokenID);
#[derive(Copy, Clone)]
pub enum NodeOrToken {
    Node(NodeID),
    Token(TokenID),
}

impl<'syn> SyntaxTree<'syn> {
    pub fn new(arena: Arena<'syn>, nodes: Vec<Node<'syn>>, tokens: TokenList) -> SyntaxTree<'syn> {
        SyntaxTree {
            arena,
            nodes,
            tokens,
        }
    }
    pub fn root(&self) -> &Node {
        self.node(NodeID::new(0))
    }
    pub fn node(&self, id: NodeID) -> &Node {
        &self.nodes[id.index()]
    }
    pub fn token(&self, id: TokenID) -> Token {
        self.tokens.get_token(id.index())
    }
    pub fn token_range(&self, id: TokenID) -> TextRange {
        self.tokens.get_range(id.index())
    }
}

pub fn tree_build<'syn>(
    input: (TokenList, Vec<Event>),
    file_id: FileID,
) -> (SyntaxTree<'syn>, Vec<ErrorComp>) {
    let mut arena = Arena::new();
    let mut nodes = Vec::new();
    let (tokens, mut events) = input;

    let mut stack = Vec::new();
    let mut parent_stack = Vec::with_capacity(8);
    let mut content_buf = TempBuffer::new();
    let mut token_idx = 0;
    let mut errors = Vec::new();

    for event_idx in 0..events.len() {
        //@clone to get around borrowing,
        //change the error handling to copy/clone events
        match events[event_idx].clone() {
            Event::StartNode {
                kind,
                forward_parent,
            } => {
                let mut parent_next = forward_parent;
                parent_stack.clear();

                while let Some(parent_idx) = parent_next {
                    let start_event = events[parent_idx as usize].clone();

                    match start_event {
                        Event::StartNode {
                            kind,
                            forward_parent,
                        } => {
                            parent_stack.push(kind);
                            parent_next = forward_parent;
                            events[parent_idx as usize] = Event::Ignore;
                        }
                        _ => unreachable!(),
                    }
                }

                while let Some(kind) = parent_stack.pop() {
                    let node_id = NodeID::new(nodes.len());
                    content_buf.add(NodeOrToken::Node(node_id));
                    let offset = content_buf.start();
                    stack.push((offset, node_id));

                    let node = Node { kind, content: &[] };
                    nodes.push(node);
                }

                let node_id = NodeID::new(nodes.len());
                content_buf.add(NodeOrToken::Node(node_id));
                let offset = content_buf.start();
                stack.push((offset, node_id));

                let node = Node { kind, content: &[] };
                nodes.push(node);
            }
            Event::EndNode => {
                let (offset, node_id) = stack.pop().unwrap();
                nodes[node_id.index()].content = content_buf.take(offset, &mut arena);
            }
            Event::Token => {
                let token_id = TokenID::new(token_idx);
                token_idx += 1;
                content_buf.add(NodeOrToken::Token(token_id));
            }
            Event::Error { message } => {
                let token_id = TokenID::new(token_idx);
                let range = tokens.get_range(token_id.index());
                let src = SourceRange::new(range, file_id);
                errors.push(ErrorComp::new(message, src, None));
            }
            Event::Ignore => {}
        }
    }

    (SyntaxTree::new(arena, nodes, tokens), errors)
}
