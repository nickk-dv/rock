use super::parser::Event;
use super::syntax_kind::SyntaxKind;
use crate::support::{Arena, IndexID, TempBuffer, ID};
use crate::token::{Token, TokenList, Trivia};

pub struct SyntaxTree<'syn> {
    #[allow(unused)]
    arena: Arena<'syn>,
    nodes: Vec<Node<'syn>>,
    tokens: TokenList,
    complete: bool,
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
    pub fn root(&self) -> &Node<'syn> {
        self.nodes.id_get(ID::new_raw(0))
    }
    pub fn node(&self, id: ID<Node<'syn>>) -> &Node<'syn> {
        self.nodes.id_get(id)
    }
    pub fn tokens(&self) -> &TokenList {
        &self.tokens
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
}

pub fn build<'syn>(
    tokens: TokenList,
    mut events: Vec<Event>,
    source: &str,
    complete: bool,
) -> SyntaxTree<'syn> {
    let mut arena = Arena::new();

    let nodes_cap = events
        .iter()
        .filter(|&e| matches!(e, Event::StartNode { .. }))
        .count();
    let mut nodes = Vec::with_capacity(nodes_cap);

    let mut node_stack = Vec::with_capacity(16);
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
                parent_stack.push(kind);
                let mut parent_next = forward_parent;

                while let Some(parent_idx) = parent_next {
                    let start_event = events[parent_idx as usize];
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

                let top_kind = *parent_stack.last().unwrap();
                let mut attach: Option<AttachedTrivia> =
                    attached_trivia(top_kind, source, &tokens, token_id, trivia_id, trivia_count);

                if let Some(n_attach) = attach.as_ref() {
                    eat_n_trivias(n_attach.prepend, &mut trivia_id, &mut content);
                }

                while let Some(kind) = parent_stack.pop() {
                    let node_id = ID::new(&nodes);
                    content.add(NodeOrToken::Node(node_id));

                    let offset = content.start();
                    node_stack.push((offset, node_id));

                    if let Some(n_attach) = attach.as_ref() {
                        eat_n_trivias(n_attach.inner, &mut trivia_id, &mut content);
                        attach = None;
                    }

                    let node = Node { kind, content: &[] };
                    nodes.push(node);
                }
            }
            Event::EndNode => {
                let (offset, node_id) = node_stack.pop().unwrap();
                if nodes.id_get(node_id).kind == SyntaxKind::SOURCE_FILE {
                    attach_trailing_trivia(&mut trivia_id, trivia_count, &mut content);
                }
                nodes.id_get_mut(node_id).content = content.take(offset, &mut arena);
            }
            Event::Token => {
                attach_prepending_trivia(
                    &tokens,
                    token_id,
                    &mut trivia_id,
                    trivia_count,
                    &mut content,
                );

                content.add(NodeOrToken::Token(token_id));
                token_id = token_id.inc();
            }
            Event::Ignore => {}
        }
    }

    // source file node exists
    assert_ne!(nodes.len(), 0);
    // pre-allocated capacity was correct
    assert_eq!(nodes.capacity(), nodes_cap);
    // each node had start && end event
    assert!(node_stack.is_empty());
    // only source file content remains
    assert_eq!(content.len(), 1);
    // all tokens consumed (except 2 eof tokens)
    assert_eq!(token_id.raw_index(), tokens.token_count() - 2);
    // all trivia consumed
    assert_eq!(trivia_id.raw_index(), tokens.trivia_count());

    SyntaxTree {
        arena,
        nodes,
        tokens,
        complete,
    }
}

struct AttachedTrivia {
    prepend: usize,
    inner: usize,
}

fn attached_trivia(
    kind: SyntaxKind,
    source: &str,
    tokens: &TokenList,
    token_id: ID<Token>,
    trivia_id: ID<Trivia>,
    trivia_count: usize,
) -> Option<AttachedTrivia> {
    if kind == SyntaxKind::SOURCE_FILE {
        return None;
    }

    let can_attach_inner = match kind {
        SyntaxKind::PROC_ITEM
        | SyntaxKind::ENUM_ITEM
        | SyntaxKind::STRUCT_ITEM
        | SyntaxKind::CONST_ITEM
        | SyntaxKind::GLOBAL_ITEM
        | SyntaxKind::IMPORT_ITEM => true,
        _ => false,
    };

    let mut total_count: usize = 0;
    let mut inner_count: usize = 0;

    let token_range = tokens.token_range(token_id);
    let remaining_range = trivia_id.raw_index()..trivia_count;
    let remaining_trivias = remaining_range.map(|idx| ID::new_raw(idx));

    for trivia_id in remaining_trivias {
        let (trivia, trivia_range) = tokens.trivia_and_range(trivia_id);
        let starts_before = trivia_range.start() < token_range.start();

        if starts_before {
            total_count += 1;
        } else {
            break;
        }

        if !can_attach_inner {
            continue;
        }

        match trivia {
            Trivia::Whitespace => {
                let trivia_text = &source[trivia_range.as_usize()];
                let new_lines = trivia_text.chars().filter(|&c| c == '\n').count();

                if new_lines >= 2 {
                    inner_count = 0;
                } else {
                    inner_count += 1;
                }
            }
            Trivia::LineComment => inner_count += 1,
            //@temp, update attachment rules
            Trivia::DocComment => inner_count += 1,
            Trivia::ModComment => inner_count += 1,
        };
    }

    Some(AttachedTrivia {
        prepend: total_count - inner_count,
        inner: inner_count,
    })
}

fn eat_n_trivias(
    n_trivias: usize,
    trivia_id: &mut ID<Trivia>,
    content: &mut TempBuffer<NodeOrToken>,
) {
    let remaining_range = trivia_id.raw_index()..(trivia_id.raw_index() + n_trivias);
    let remaining_trivias = remaining_range.map(|idx| ID::new_raw(idx));

    for id in remaining_trivias {
        *trivia_id = id.inc();
        content.add(NodeOrToken::Trivia(id));
    }
}

fn attach_prepending_trivia(
    tokens: &TokenList,
    token_id: ID<Token>,
    trivia_id: &mut ID<Trivia>,
    trivia_count: usize,
    content: &mut TempBuffer<NodeOrToken>,
) {
    let token_range = tokens.token_range(token_id);
    let remaining_range = trivia_id.raw_index()..trivia_count;
    let remaining_trivias = remaining_range.map(|idx| ID::new_raw(idx));

    for id in remaining_trivias {
        let trivia_range = tokens.trivia_range(id);
        let starts_before = trivia_range.start() < token_range.start();

        if starts_before {
            *trivia_id = id.inc();
            content.add(NodeOrToken::Trivia(id));
        } else {
            break;
        }
    }
}

fn attach_trailing_trivia(
    trivia_id: &mut ID<Trivia>,
    trivia_count: usize,
    content: &mut TempBuffer<NodeOrToken>,
) {
    let remaining_range = trivia_id.raw_index()..trivia_count;
    let remaining_trivias = remaining_range.map(|idx| ID::new_raw(idx));

    for id in remaining_trivias {
        *trivia_id = id.inc();
        content.add(NodeOrToken::Trivia(id));
    }
}

pub fn tree_display(tree: &SyntaxTree, source: &str) -> String {
    let mut buffer = String::with_capacity(source.len() * 8);
    print_node(&mut buffer, tree, source, tree.root(), 0);
    return buffer;

    fn print_node(buffer: &mut String, tree: &SyntaxTree, source: &str, node: &Node, depth: u32) {
        print_depth(buffer, depth);
        buffer.push_str(&format!("{:?}\n", node.kind));

        for node_or_token in node.content {
            match *node_or_token {
                NodeOrToken::Node(id) => {
                    let node = tree.node(id);
                    print_node(buffer, tree, source, node, depth + 1);
                }
                NodeOrToken::Token(id) => {
                    let range = tree.tokens.token_range(id);
                    print_depth(buffer, depth + 1);
                    buffer.push_str(&format!("@{:?} {:?}\n", range, &source[range.as_usize()]));
                }
                NodeOrToken::Trivia(id) => {
                    let (trivia, range) = tree.tokens().trivia_and_range(id);
                    let trivia = match trivia {
                        Trivia::Whitespace => "WHITESPACE",
                        Trivia::LineComment => "LINE_COMMENT",
                        Trivia::DocComment => "DOC_COMMENT",
                        Trivia::ModComment => "MOD_COMMENT",
                    };
                    print_depth(buffer, depth + 1);
                    buffer.push_str(&format!(
                        "{}@{:?} {:?}\n",
                        trivia,
                        range,
                        &source[range.as_usize()]
                    ));
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
