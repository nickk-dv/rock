use super::parser::Event;
use super::syntax_kind::SyntaxKind;
use crate::arena::Arena;
use crate::error::ErrorComp;
use crate::id_impl;
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

    pub fn node(&self, node_id: NodeID) -> &Node {
        &self.nodes[node_id.index()]
    }
    pub fn token(&self, token_id: TokenID) -> Token {
        self.tokens.get_token(token_id.index())
    }
    pub fn token_range(&self, token_id: TokenID) -> TextRange {
        self.tokens.get_range(token_id.index())
    }
}

pub fn build<'syn>(
    input: (TokenList, Vec<Event>, Vec<ErrorComp>),
) -> (SyntaxTree<'syn>, Vec<ErrorComp>) {
    let mut arena = Arena::new();
    let mut nodes = Vec::new();
    let (tokens, mut events, errors) = input;

    let mut stack = Vec::with_capacity(16);
    let mut parent_stack = Vec::with_capacity(16);
    let mut content = TempBuffer::new();
    let mut token_idx = 0;

    for event_idx in 0..events.len() {
        match events[event_idx] {
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
                    content.add(NodeOrToken::Node(node_id));
                    let offset = content.start();
                    stack.push((offset, node_id));

                    let node = Node { kind, content: &[] };
                    nodes.push(node);
                }

                let node_id = NodeID::new(nodes.len());
                content.add(NodeOrToken::Node(node_id));
                let offset = content.start();
                stack.push((offset, node_id));

                let node = Node { kind, content: &[] };
                nodes.push(node);
            }
            Event::EndNode => {
                let (offset, node_id) = stack.pop().unwrap();
                nodes[node_id.index()].content = content.take(offset, &mut arena);
            }
            Event::Token => {
                let token_id = TokenID::new(token_idx);
                token_idx += 1;
                content.add(NodeOrToken::Token(token_id));
            }
            Event::Ignore => {}
        }
    }

    (SyntaxTree::new(arena, nodes, tokens), errors)
}

pub fn tree_print(tree: &SyntaxTree, source: &str) {
    print_node(tree, source, tree.node(NodeID::new(0)), 0);

    fn print_depth(depth: u32) {
        for _ in 0..depth {
            print!("  ");
        }
    }

    fn print_node(tree: &SyntaxTree, source: &str, node: &Node, depth: u32) {
        print_depth(depth);
        println!("[{:?}]", node.kind);

        for node_or_token in node.content {
            match *node_or_token {
                NodeOrToken::Node(node_id) => {
                    let node = tree.node(node_id);
                    print_node(tree, source, node, depth + 1);
                }
                NodeOrToken::Token(token_id) => {
                    let range = tree.token_range(token_id);
                    print_depth(depth + 1);
                    println!("@{:?} `{}`", range, &source[range.as_usize()]);
                }
            }
        }
    }
}
