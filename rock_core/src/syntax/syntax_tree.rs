use super::parser::Event;
use super::syntax_kind::SyntaxKind;
use super::token::{TokenID, TokenList, Trivia, TriviaID};
use crate::error::{ErrorBuffer, ErrorSink, SourceRange};
use crate::errors as err;
use crate::session::ModuleID;
use crate::support::{Arena, TempBuffer, TempOffset};
use crate::text::TextRange;

pub struct SyntaxTree<'syn> {
    #[allow(unused)]
    arena: Arena<'syn>,
    nodes: Vec<Node<'syn>>,
    tokens: TokenList,
    complete: bool,
}

crate::define_id!(pub NodeID);
pub struct Node<'syn> {
    pub kind: SyntaxKind,
    pub range: TextRange,
    pub content: &'syn [NodeOrToken],
}

#[derive(Copy, Clone)]
pub enum NodeOrToken {
    Node(NodeID),
    Token(TokenID),
    Trivia(TriviaID),
}

impl<'syn> SyntaxTree<'syn> {
    pub fn root(&self) -> &Node<'syn> {
        &self.nodes[0]
    }
    pub fn node(&self, id: NodeID) -> &Node<'syn> {
        &self.nodes[id.index()]
    }
    pub fn tokens(&self) -> &TokenList {
        &self.tokens
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
}

pub fn tree_display(tree: &SyntaxTree, source: &str) -> String {
    let mut buffer = String::with_capacity(source.len() * 8);
    print_node(&mut buffer, tree, source, tree.root(), 0);
    return buffer;

    fn print_node(buffer: &mut String, tree: &SyntaxTree, source: &str, node: &Node, depth: u32) {
        use std::fmt::Write;
        for _ in 0..depth {
            buffer.push_str("  ");
        }
        let _ = writeln!(buffer, "{:?} {:?}", node.kind, node.range);

        for not in node.content.iter().copied() {
            match not {
                NodeOrToken::Node(id) => {
                    let node = tree.node(id);
                    print_node(buffer, tree, source, node, depth + 1);
                }
                NodeOrToken::Token(id) => {
                    for _ in 0..=depth {
                        buffer.push_str("  ");
                    }
                    let range = tree.tokens.token_range(id);
                    let text = &source[range.as_usize()];
                    let _ = writeln!(buffer, "{range:?} {text:?}");
                }
                NodeOrToken::Trivia(id) => {
                    for _ in 0..=depth {
                        buffer.push_str("  ");
                    }
                    let range = tree.tokens().trivia_range(id);
                    let text = &source[range.as_usize()];
                    let _ = writeln!(buffer, "{range:?} {text:?}");
                }
            }
        }
    }
}

struct SyntaxTreeBuild<'syn, 'src> {
    source: &'src str,
    tokens: TokenList,
    events: Vec<Event>,
    module_id: ModuleID,
    errors: ErrorBuffer,

    arena: Arena<'syn>,
    nodes: Vec<Node<'syn>>,
    content: TempBuffer<NodeOrToken>,
    node_stack: Vec<(TempOffset<NodeOrToken>, NodeID)>,
    parent_stack: Vec<SyntaxKind>,
    curr_token: TokenID,
    curr_trivia: TriviaID,
}

struct NodeTrivia {
    n_inner: InnerTrivia,
    n_outher: OutherTrivia,
}

struct SourceTrivia {
    n_inner: InnerTrivia,
}

struct TokenTrivia {
    n_outher: OutherTrivia,
}

#[derive(Copy, Clone)]
struct InnerTrivia(usize);

#[derive(Copy, Clone)]
struct OutherTrivia(usize);

pub fn tree_build<'syn>(
    source: &str,
    tokens: TokenList,
    events: Vec<Event>,
    module_id: ModuleID,
    complete: bool,
) -> (SyntaxTree<'syn>, ErrorBuffer) {
    let node_count = events.iter().filter(|&e| matches!(e, Event::StartNode { .. })).count();

    let mut build = SyntaxTreeBuild {
        source,
        tokens,
        events,
        module_id,
        errors: ErrorBuffer::default(),
        arena: Arena::new(),
        nodes: Vec::with_capacity(node_count),
        content: TempBuffer::new(128),
        node_stack: Vec::with_capacity(32),
        parent_stack: Vec::with_capacity(32),
        curr_token: TokenID::new(0),
        curr_trivia: TriviaID::new(0),
    };

    tree_build_impl(&mut build);

    let tree = SyntaxTree {
        arena: build.arena,
        nodes: build.nodes,
        tokens: build.tokens,
        complete: complete && build.errors.error_count() == 0,
    };
    (tree, build.errors)
}

fn tree_build_impl(b: &mut SyntaxTreeBuild) {
    assert!(b.events.len() >= 2); // root node exists

    // SOURCE_FILE StartNode:
    {
        let node = Node { kind: SyntaxKind::SOURCE_FILE, range: TextRange::zero(), content: &[] };
        let node_id = NodeID::new(b.nodes.len());
        let offset = b.content.start();

        b.nodes.push(node);
        b.node_stack.push((offset, node_id));

        let source_trivia = attached_source_trivia(b);
        eat_n_inner_trivias(b, source_trivia.n_inner);
    }

    for event_idx in 1..b.events.len() - 1 {
        match b.events[event_idx] {
            Event::StartNode { kind, forward_parent } => {
                let mut parent_next = forward_parent;
                b.parent_stack.push(kind);

                while let Some(parent_idx) = parent_next {
                    match b.events[parent_idx as usize] {
                        Event::StartNode { kind, forward_parent } => {
                            parent_next = forward_parent;
                            b.parent_stack.push(kind);
                            b.events[parent_idx as usize] = Event::Ignore;
                        }
                        _ => unreachable!(),
                    }
                }

                let top_kind = *b.parent_stack.last().unwrap();
                let node_trivia = attached_node_trivia(b, top_kind);
                let mut added_inner = false;
                eat_n_outher_trivias(b, node_trivia.n_outher);

                while let Some(kind) = b.parent_stack.pop() {
                    let start = node_or_token_range(b, b.content.view_all().last().copied()).end();
                    let node = Node { kind, range: TextRange::new(start, start), content: &[] };
                    let node_id = NodeID::new(b.nodes.len());
                    b.content.push(NodeOrToken::Node(node_id));
                    let offset = b.content.start();

                    b.nodes.push(node);
                    b.node_stack.push((offset, node_id));

                    if !added_inner {
                        added_inner = true;
                        eat_n_inner_trivias(b, node_trivia.n_inner);
                    }
                }
            }
            Event::EndNode => {
                let (offset, node_id) = b.node_stack.pop().unwrap();
                let end = node_or_token_range(b, b.content.view_all().last().copied()).end();
                let node = &mut b.nodes[node_id.index()];
                node.content = b.content.take(offset, &mut b.arena);
                node.range = TextRange::new(node.range.start(), end);
            }
            Event::Token => {
                let token_trivia = attached_token_trivia(b);
                eat_n_outher_trivias(b, token_trivia.n_outher);

                b.content.push(NodeOrToken::Token(b.curr_token));
                b.curr_token = b.curr_token.inc();
            }
            Event::Ignore => {}
        }
    }

    // SOURCE_FILE EndNode:
    {
        let n_remaining = b.tokens.trivia_count() - b.curr_trivia.index();
        eat_n_outher_trivias(b, OutherTrivia(n_remaining));

        let (offset, node_id) = b.node_stack.pop().unwrap();
        let end = node_or_token_range(b, b.content.view_all().last().copied()).end();
        let node = &mut b.nodes[node_id.index()];
        node.content = b.content.take(offset, &mut b.arena);
        node.range = TextRange::new(node.range.start(), end);
    }

    assert!(b.content.is_empty()); // all content has been taken
    assert!(b.node_stack.is_empty()); // each node had start & end event
    assert_eq!(b.curr_token.index() + 2, b.tokens.token_count()); // all tokens have been consumed (except 2 eof tokens)
    assert_eq!(b.curr_trivia.index(), b.tokens.trivia_count()); // all trivias have been consumed
}

fn node_or_token_range(b: &SyntaxTreeBuild, not: Option<NodeOrToken>) -> TextRange {
    match not {
        Some(NodeOrToken::Node(id)) => b.nodes[id.index()].range,
        Some(NodeOrToken::Token(id)) => b.tokens.token_range(id),
        Some(NodeOrToken::Trivia(id)) => b.tokens.trivia_range(id),
        None => TextRange::zero(),
    }
}

fn attached_node_trivia(b: &mut SyntaxTreeBuild, kind: SyntaxKind) -> NodeTrivia {
    let can_attach_inner = matches!(
        kind,
        SyntaxKind::PROC_ITEM
            | SyntaxKind::ENUM_ITEM
            | SyntaxKind::STRUCT_ITEM
            | SyntaxKind::CONST_ITEM
            | SyntaxKind::GLOBAL_ITEM
            | SyntaxKind::IMPORT_ITEM
    );

    let mut total_count: usize = 0;
    let mut inner_count: usize = 0;
    let mut collect_inner = false;
    let token_range = b.tokens.token_range(b.curr_token);
    let remaining_ids = b.curr_trivia.index()..b.tokens.trivia_count();
    let remaining_ids = remaining_ids.map(TriviaID::new);

    for trivia_id in remaining_ids {
        let (trivia, range) = b.tokens.trivia_and_range(trivia_id);

        let before_token = range.start() < token_range.start();
        if !before_token {
            break;
        }
        total_count += 1;

        if !can_attach_inner {
            continue;
        }

        match trivia {
            Trivia::Whitespace => {
                let whitespace = &b.source[range.as_usize()];
                let new_lines = whitespace.chars().filter(|&c| c == '\n').count();

                if new_lines >= 2 {
                    collect_inner = false;
                    inner_count = 0;
                } else if collect_inner {
                    inner_count += 1;
                }
            }
            Trivia::LineComment => {
                if collect_inner {
                    inner_count += 1;
                }
            }
            Trivia::DocComment => {
                collect_inner = true;
                inner_count += 1
            }
            Trivia::ModComment => {
                collect_inner = false;
                inner_count = 0;
            }
        };
    }

    NodeTrivia {
        n_inner: InnerTrivia(inner_count),
        n_outher: OutherTrivia(total_count - inner_count),
    }
}

fn attached_source_trivia(b: &mut SyntaxTreeBuild) -> SourceTrivia {
    let mut total_count: usize = 0;
    let mut mod_trivia_found = false;
    let token_range = b.tokens.token_range(b.curr_token);
    let remaining_ids = b.curr_trivia.index()..b.tokens.trivia_count();
    let remaining_ids = remaining_ids.map(TriviaID::new);

    for trivia_id in remaining_ids {
        let (trivia, range) = b.tokens.trivia_and_range(trivia_id);

        let before_token = range.start() < token_range.start();
        if !before_token {
            break;
        }

        match trivia {
            Trivia::Whitespace => {
                let whitespace = &b.source[range.as_usize()];
                let new_lines = whitespace.chars().filter(|&c| c == '\n').count();

                if new_lines >= 2 && mod_trivia_found {
                    break;
                } else {
                    total_count += 1;
                }
            }
            Trivia::LineComment => break,
            Trivia::DocComment => break,
            Trivia::ModComment => {
                mod_trivia_found = true;
                total_count += 1;
            }
        }
    }

    SourceTrivia { n_inner: InnerTrivia(total_count) }
}

fn attached_token_trivia(b: &mut SyntaxTreeBuild) -> TokenTrivia {
    let mut total_count: usize = 0;
    let token_range = b.tokens.token_range(b.curr_token);
    let remaining_ids = b.curr_trivia.index()..b.tokens.trivia_count();
    let remaining_ids = remaining_ids.map(TriviaID::new);

    for trivia_id in remaining_ids {
        let range = b.tokens.trivia_range(trivia_id);

        let before_token = range.start() < token_range.start();
        if !before_token {
            break;
        }
        total_count += 1;
    }

    TokenTrivia { n_outher: OutherTrivia(total_count) }
}

/// `InnerTrivia` is always considered valid
fn eat_n_inner_trivias(b: &mut SyntaxTreeBuild, n_inner: InnerTrivia) {
    let trivia_ids = b.curr_trivia.index()..(b.curr_trivia.index() + n_inner.0);
    let trivia_ids = trivia_ids.map(TriviaID::new);

    for id in trivia_ids {
        b.curr_trivia = b.curr_trivia.inc();
        b.content.push(NodeOrToken::Trivia(id));
    }
}

/// `OutherTrivia` will error on `Doc` or `Mod` comments
fn eat_n_outher_trivias(b: &mut SyntaxTreeBuild, n_outher: OutherTrivia) {
    let trivia_ids = b.curr_trivia.index()..(b.curr_trivia.index() + n_outher.0);
    let trivia_ids = trivia_ids.map(TriviaID::new);

    for id in trivia_ids {
        b.curr_trivia = b.curr_trivia.inc();
        b.content.push(NodeOrToken::Trivia(id));

        let (trivia, range) = b.tokens.trivia_and_range(id);
        match trivia {
            Trivia::Whitespace => {}
            Trivia::LineComment => {}
            Trivia::DocComment => {
                let src = SourceRange::new(b.module_id, range);
                err::syntax_invalid_doc_comment(&mut b.errors, src);
            }
            Trivia::ModComment => {
                let src = SourceRange::new(b.module_id, range);
                err::syntax_invalid_mod_comment(&mut b.errors, src);
            }
        };
    }
}
