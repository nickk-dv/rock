use super::parser::Event;
use super::token::{TokenID, TokenList, Trivia, TriviaID};
use crate::error::{ErrorBuffer, ErrorSink, SourceRange};
use crate::errors as err;
use crate::session::ModuleID;
use crate::support::{TempBuffer, TempOffset};
use crate::text::TextRange;

pub struct SyntaxTree {
    nodes: Vec<Node>,
    content: Vec<NodeOrTokenID>,
    pub tokens: TokenList,
    pub complete: bool,
}

crate::define_id!(pub NodeID);
pub struct Node {
    pub kind: SyntaxKind,
    pub range: TextRange,
    pub content_idx: u32,
    pub content_len: u32,
}

#[derive(Copy, Clone)]
struct NodeOrTokenID {
    mask: u32,
}

#[derive(Copy, Clone)]
pub enum NodeOrToken {
    Node(NodeID),
    Token(TokenID),
    Trivia(TriviaID),
}

#[derive(Clone)]
pub struct NodeContentIter<'syn> {
    pub tree: &'syn SyntaxTree,
    pos: u32,
    end: u32,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum SyntaxKind {
    ERROR,
    TOMBSTONE,
    SOURCE_FILE,

    PROC_ITEM,
    PARAM_LIST,
    PARAM,
    ENUM_ITEM,
    VARIANT_LIST,
    VARIANT,
    VARIANT_FIELD_LIST,
    VARIANT_FIELD,
    STRUCT_ITEM,
    FIELD_LIST,
    FIELD,
    CONST_ITEM,
    GLOBAL_ITEM,
    IMPORT_ITEM,
    IMPORT_PATH,
    IMPORT_SYMBOL_LIST,
    IMPORT_SYMBOL,
    IMPORT_SYMBOL_RENAME,

    DIRECTIVE_LIST,
    DIRECTIVE_SIMPLE,
    DIRECTIVE_WITH_PARAMS,
    DIRECTIVE_PARAM_LIST,
    DIRECTIVE_PARAM,

    TYPE_BASIC,
    TYPE_CUSTOM,
    TYPE_REFERENCE,
    TYPE_MULTI_REFERENCE,
    TYPE_PROCEDURE,
    PROC_TYPE_PARAM_LIST,
    PROC_TYPE_PARAM,
    TYPE_ARRAY_SLICE,
    TYPE_ARRAY_STATIC,

    BLOCK,
    STMT_BREAK,
    STMT_CONTINUE,
    STMT_RETURN,
    STMT_DEFER,
    STMT_FOR,
    FOR_BIND,
    FOR_HEADER_COND,
    FOR_HEADER_ELEM,
    FOR_HEADER_RANGE,
    FOR_HEADER_PAT,
    STMT_LOCAL,
    STMT_ASSIGN,
    STMT_EXPR_SEMI,
    STMT_EXPR_TAIL,
    STMT_WITH_DIRECTIVE,

    EXPR_PAREN,
    EXPR_IF,
    BRANCH_COND,
    BRANCH_PAT,
    EXPR_MATCH,
    MATCH_ARM_LIST,
    MATCH_ARM,
    EXPR_FIELD,
    EXPR_INDEX,
    EXPR_SLICE,
    EXPR_CALL,
    EXPR_CAST,
    EXPR_ITEM,
    EXPR_VARIANT,
    EXPR_STRUCT_INIT,
    FIELD_INIT_LIST,
    FIELD_INIT,
    EXPR_ARRAY_INIT,
    ARRAY_INIT,
    EXPR_ARRAY_REPEAT,
    EXPR_TRY,
    EXPR_DEREF,
    EXPR_ADDRESS,
    EXPR_UNARY,
    EXPR_BINARY,

    PAT_WILD,
    PAT_LIT,
    PAT_RANGE,
    PAT_ITEM,
    PAT_VARIANT,
    PAT_OR,

    LIT_VOID,
    LIT_NULL,
    LIT_BOOL,
    LIT_INT,
    LIT_FLOAT,
    LIT_CHAR,
    LIT_STRING,

    NAME,
    BIND,
    BIND_LIST,
    ARGS_LIST,
    PATH,
    PATH_SEGMENT,
    POLYMORPH_ARGS,
    POLYMORPH_PARAMS,
}

#[derive(Clone, Copy)]
pub struct SyntaxSet {
    mask: u128,
}

impl SyntaxTree {
    pub fn root(&self) -> &Node {
        &self.nodes[0]
    }
    pub fn node(&self, id: NodeID) -> &Node {
        &self.nodes[id.index()]
    }
    pub fn content<'syn>(&'syn self, node: &Node) -> NodeContentIter<'syn> {
        NodeContentIter {
            tree: self,
            pos: node.content_idx,
            end: node.content_idx + node.content_len,
        }
    }
}

impl NodeOrTokenID {
    fn encode_node(id: NodeID) -> NodeOrTokenID {
        NodeOrTokenID { mask: id.raw() }
    }
    fn encode_token(id: TokenID) -> NodeOrTokenID {
        NodeOrTokenID { mask: id.raw() | (1 << 30) }
    }
    fn encode_trivia(id: TriviaID) -> NodeOrTokenID {
        NodeOrTokenID { mask: id.raw() | (2 << 30) }
    }
    fn decode(self) -> NodeOrToken {
        let kind = (self.mask >> 30) & 0b11;
        let id = self.mask & 0x3FFFFFFF;
        match kind {
            0 => NodeOrToken::Node(NodeID(id)),
            1 => NodeOrToken::Token(TokenID::new(id as usize)),
            _ => NodeOrToken::Trivia(TriviaID::new(id as usize)),
        }
    }
}

impl Iterator for NodeContentIter<'_> {
    type Item = NodeOrToken;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.end {
            return None;
        }
        let not_id = self.tree.content[self.pos as usize];
        self.pos += 1;
        Some(not_id.decode())
    }
}

impl DoubleEndedIterator for NodeContentIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.pos == self.end {
            return None;
        }
        self.end -= 1;
        let not_id = self.tree.content[self.end as usize];
        Some(not_id.decode())
    }
}

impl SyntaxSet {
    pub const fn new(syntax: &[SyntaxKind]) -> SyntaxSet {
        let mut mask = 0u128;
        let mut i = 0;
        while i < syntax.len() {
            mask |= 1u128 << syntax[i] as u8;
            i += 1;
        }
        SyntaxSet { mask }
    }
    pub const fn empty() -> SyntaxSet {
        SyntaxSet { mask: 0 }
    }
    pub const fn combine(self, other: SyntaxSet) -> SyntaxSet {
        SyntaxSet { mask: self.mask | other.mask }
    }
    pub const fn contains(&self, syntax: SyntaxKind) -> bool {
        self.mask & (1u128 << syntax as u8) != 0
    }
}

pub fn tree_display(tree: &SyntaxTree, source: &str) -> String {
    let mut buf = String::with_capacity(source.len() * 8);
    node_display(&mut buf, tree, source, tree.root(), 0);
    buf
}

fn node_display(buf: &mut String, tree: &SyntaxTree, source: &str, node: &Node, depth: u32) {
    use std::fmt::Write;
    for _ in 0..depth {
        buf.push_str("  ");
    }
    let _ = writeln!(buf, "{:?} {:?}", node.kind, node.range);

    for not in tree.content(node) {
        match not {
            NodeOrToken::Node(id) => {
                let node = tree.node(id);
                node_display(buf, tree, source, node, depth + 1);
            }
            NodeOrToken::Token(id) => {
                for _ in 0..=depth {
                    buf.push_str("  ");
                }
                let range = tree.tokens.token_range(id);
                let text = &source[range.as_usize()];
                let _ = writeln!(buf, "{range:?} {text:?}");
            }
            NodeOrToken::Trivia(id) => {
                for _ in 0..=depth {
                    buf.push_str("  ");
                }
                let range = tree.tokens.trivia_range(id);
                let text = &source[range.as_usize()];
                let _ = writeln!(buf, "{range:?} {text:?}");
            }
        }
    }
}

struct SyntaxTreeBuild<'src> {
    source: &'src str,
    tokens: TokenList,
    events: Vec<Event>,
    module_id: ModuleID,
    errors: ErrorBuffer,

    nodes: Vec<Node>,
    content: TempBuffer<NodeOrTokenID>,
    content_vec: Vec<NodeOrTokenID>,
    node_stack: Vec<(TempOffset<NodeOrTokenID>, NodeID)>,
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

pub fn tree_build(
    source: &str,
    tokens: TokenList,
    events: Vec<Event>,
    module_id: ModuleID,
    complete: bool,
) -> (SyntaxTree, ErrorBuffer) {
    let node_count = events.iter().filter(|&e| matches!(e, Event::StartNode { .. })).count();
    let content_count = node_count + tokens.token_count() + tokens.trivia_count();

    let mut build = SyntaxTreeBuild {
        source,
        tokens,
        events,
        module_id,
        errors: ErrorBuffer::default(),
        nodes: Vec::with_capacity(node_count),
        content: TempBuffer::new(128),
        content_vec: Vec::with_capacity(content_count),
        node_stack: Vec::with_capacity(32),
        parent_stack: Vec::with_capacity(32),
        curr_token: TokenID::new(0),
        curr_trivia: TriviaID::new(0),
    };

    tree_build_impl(&mut build);

    let tree = SyntaxTree {
        nodes: build.nodes,
        tokens: build.tokens,
        content: build.content_vec,
        complete: complete && build.errors.error_count() == 0,
    };
    (tree, build.errors)
}

fn tree_build_impl(b: &mut SyntaxTreeBuild) {
    assert!(b.events.len() >= 2); // root node exists

    // SOURCE_FILE StartNode:
    {
        let node = Node {
            kind: SyntaxKind::SOURCE_FILE,
            range: TextRange::zero(),
            content_idx: 0,
            content_len: 0,
        };
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
                    let start =
                        node_or_token_range(b, b.content.view_all().last().map(|id| id.decode()))
                            .end();
                    let node = Node {
                        kind,
                        range: TextRange::new(start, start),
                        content_idx: 0,
                        content_len: 0,
                    };
                    let node_id = NodeID::new(b.nodes.len());
                    b.content.push(NodeOrTokenID::encode_node(node_id));
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
                let end =
                    node_or_token_range(b, b.content.view_all().last().map(|id| id.decode())).end();
                let node = &mut b.nodes[node_id.index()];
                node.range = TextRange::new(node.range.start(), end);

                let view = b.content.view(offset);
                node.content_idx = b.content_vec.len() as u32;
                node.content_len = view.len() as u32;
                b.content_vec.extend_from_slice(view);
                b.content.pop_view(offset);
            }
            Event::Token => {
                let token_trivia = attached_token_trivia(b);
                eat_n_outher_trivias(b, token_trivia.n_outher);

                b.content.push(NodeOrTokenID::encode_token(b.curr_token));
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
        let end = node_or_token_range(b, b.content.view_all().last().map(|id| id.decode())).end();
        let node = &mut b.nodes[node_id.index()];
        node.range = TextRange::new(node.range.start(), end);

        let view = b.content.view(offset);
        node.content_idx = b.content_vec.len() as u32;
        node.content_len = view.len() as u32;
        b.content_vec.extend_from_slice(view);
        b.content.pop_view(offset);
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
        b.content.push(NodeOrTokenID::encode_trivia(id));
    }
}

/// `OutherTrivia` will error on `Doc` or `Mod` comments
fn eat_n_outher_trivias(b: &mut SyntaxTreeBuild, n_outher: OutherTrivia) {
    let trivia_ids = b.curr_trivia.index()..(b.curr_trivia.index() + n_outher.0);
    let trivia_ids = trivia_ids.map(TriviaID::new);

    for id in trivia_ids {
        b.curr_trivia = b.curr_trivia.inc();
        b.content.push(NodeOrTokenID::encode_trivia(id));

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
