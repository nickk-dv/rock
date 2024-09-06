use super::parser::Event;
use super::syntax_kind::SyntaxKind;
use crate::support::{Arena, IndexID, TempBuffer, ID};
use crate::token::{Token, TokenList, Trivia};

pub struct SyntaxTree<'syn> {
    #[allow(unused)]
    arena: Arena<'syn>,
    nodes: Vec<Node<'syn>>,
    tokens: TokenList,
}

pub struct Node<'syn> {
    pub kind: SyntaxKind,
    pub content: &'syn [NodeOrToken<'syn>],
}

#[derive(Copy, Clone)]
pub enum NodeOrToken<'syn> {
    Node(ID<Node<'syn>>),
    Token(ID<Token>),
    Trivia(ID<Trivia>),
}

impl<'syn> SyntaxTree<'syn> {
    pub fn new(arena: Arena<'syn>, nodes: Vec<Node<'syn>>, tokens: TokenList) -> SyntaxTree<'syn> {
        SyntaxTree {
            arena,
            nodes,
            tokens,
        }
    }
    pub fn root(&self) -> &Node<'syn> {
        self.nodes.id_get(ID::new_raw(0))
    }
    pub fn node(&self, id: ID<Node<'syn>>) -> &Node<'syn> {
        self.nodes.id_get(id)
    }
    pub fn tokens(&self) -> &TokenList {
        &self.tokens
    }
}

pub fn build<'syn>(tokens: TokenList, mut events: Vec<Event>) -> SyntaxTree<'syn> {
    let mut arena = Arena::new();
    let mut nodes = Vec::new();

    let mut stack = Vec::with_capacity(16);
    let mut parent_stack = Vec::with_capacity(16);
    let mut content = TempBuffer::new(128);

    let mut token_id = ID::<Token>::new_raw(0);
    let mut trivia_id = ID::<Trivia>::new_raw(0);
    let trivia_count = tokens.trivia_count();

    for event_idx in 0..events.len() {
        match events[event_idx] {
            Event::StartNode {
                kind,
                forward_parent,
            } => {
                trivia_id = attach_prepending_trivia(
                    &tokens,
                    token_id,
                    trivia_id,
                    trivia_count,
                    &mut content,
                );

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

                //@insert attached inner trivias after first pop() node (including the origin)
                // if inner trivias are even required
                while let Some(kind) = parent_stack.pop() {
                    let node_id = ID::new(&nodes);
                    content.add(NodeOrToken::Node(node_id));
                    let offset = content.start();
                    stack.push((offset, node_id));

                    let node = Node { kind, content: &[] };
                    nodes.push(node);
                }

                let node_id = ID::new(&nodes);
                content.add(NodeOrToken::Node(node_id));
                let offset = content.start();
                stack.push((offset, node_id));

                let node = Node { kind, content: &[] };
                nodes.push(node);
            }
            Event::EndNode => {
                let (offset, node_id) = stack.pop().unwrap();

                // @hack or required?
                if nodes.id_get(node_id).kind == SyntaxKind::SOURCE_FILE {
                    trivia_id = attach_remaining_trivia(trivia_id, trivia_count, &mut content);
                }

                nodes.id_get_mut(node_id).content = content.take(offset, &mut arena);
            }
            Event::Token => {
                trivia_id = attach_prepending_trivia(
                    &tokens,
                    token_id,
                    trivia_id,
                    trivia_count,
                    &mut content,
                );

                content.add(NodeOrToken::Token(token_id));
                token_id = token_id.inc();
            }
            Event::Ignore => {}
        }
    }

    SyntaxTree::new(arena, nodes, tokens)
}

#[must_use]
fn attach_prepending_trivia(
    tokens: &TokenList,
    token_id: ID<Token>,
    trivia_id: ID<Trivia>,
    trivia_count: usize,
    content: &mut TempBuffer<NodeOrToken>,
) -> ID<Trivia> {
    let token_range = tokens.token_range(token_id);
    let remaining_range = trivia_id.raw_index()..trivia_count;
    let remaining_trivias = remaining_range.map(|idx| ID::new_raw(idx));
    let mut out_trivia_id = trivia_id;

    for trivia_id in remaining_trivias {
        let trivia_range = tokens.trivia_range(trivia_id);
        let starts_before = trivia_range.start() < token_range.start();

        if starts_before {
            out_trivia_id = trivia_id;
            content.add(NodeOrToken::Trivia(trivia_id));
        } else {
            break;
        }
    }

    out_trivia_id
}

#[must_use]
fn attach_remaining_trivia(
    trivia_id: ID<Trivia>,
    trivia_count: usize,
    content: &mut TempBuffer<NodeOrToken>,
) -> ID<Trivia> {
    let remaining_range = trivia_id.raw_index()..trivia_count;
    let remaining_trivias = remaining_range.map(|idx| ID::new_raw(idx));
    let mut out_trivia_id = trivia_id;

    for trivia_id in remaining_trivias {
        out_trivia_id = trivia_id;
        content.add(NodeOrToken::Trivia(trivia_id));
    }

    out_trivia_id
}

pub fn tree_display(tree: &SyntaxTree, source: &str) -> String {
    let mut buffer = String::with_capacity(source.len() * 8);
    print_node(&mut buffer, tree, source, tree.root(), 0);
    return buffer;

    fn print_node(buffer: &mut String, tree: &SyntaxTree, source: &str, node: &Node, depth: u32) {
        print_depth(buffer, depth);
        buffer.push_str(&format!("[{:?}]", node.kind));

        for node_or_token in node.content {
            match *node_or_token {
                NodeOrToken::Node(id) => {
                    let node = tree.node(id);
                    print_node(buffer, tree, source, node, depth + 1);
                }
                NodeOrToken::Token(id) => {
                    let range = tree.tokens.token_range(id);
                    print_depth(buffer, depth + 1);
                    buffer.push_str(&format!("@{:?} `{:?}`", range, &source[range.as_usize()]));
                }
                NodeOrToken::Trivia(id) => {
                    let range = tree.tokens().trivia_range(id);
                    print_depth(buffer, depth + 1);
                    buffer.push_str(&format!("@{:?} `{:?}`", range, &source[range.as_usize()]));
                }
            }
        }
    }

    fn print_depth(buffer: &mut String, depth: u32) {
        for _ in 0..depth {
            buffer.push_str("  ");
        }
    }
}
