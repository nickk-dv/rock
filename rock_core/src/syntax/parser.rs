use super::syntax_kind::SyntaxKind;
use crate::error::{ErrorComp, SourceRange, StringOrStr};
use crate::session::ModuleID;
use crate::support::ID;
use crate::token::{Token, TokenList, TokenSet, T};
use std::cell::Cell;

pub struct Parser {
    cursor: ID<Token>,
    tokens: TokenList,
    events: Vec<Event>,
    errors: Vec<ErrorComp>,
    steps: Cell<u32>,
    module_id: ModuleID,
}

#[derive(Clone)]
pub enum Event {
    StartNode {
        kind: SyntaxKind,
        forward_parent: Option<u32>,
    },
    EndNode,
    Token,
    Ignore,
}

pub struct Marker {
    handled: bool,
    event_idx: u32,
}

pub struct MarkerClosed {
    event_idx: u32,
}

impl Parser {
    pub fn new(tokens: TokenList, module_id: ModuleID) -> Parser {
        Parser {
            cursor: ID::new_raw(0),
            tokens,
            events: Vec::new(),
            errors: Vec::new(),
            steps: Cell::new(0),
            module_id,
        }
    }

    pub fn finish(self) -> (TokenList, Vec<Event>, Vec<ErrorComp>) {
        (self.tokens, self.events, self.errors)
    }

    pub fn at(&self, token: Token) -> bool {
        self.peek() == token
    }

    pub fn at_next(&self, token: Token) -> bool {
        self.peek_next() == token
    }

    pub fn at_prev(&self, token: Token) -> bool {
        self.tokens.token(self.cursor.dec()) == token
    }

    pub fn at_set(&self, token_set: TokenSet) -> bool {
        token_set.contains(self.peek())
    }

    pub fn peek(&self) -> Token {
        self.step_bump();
        self.tokens.token(self.cursor)
    }

    pub fn peek_next(&self) -> Token {
        self.step_bump();
        self.tokens.token(self.cursor.inc())
    }

    pub fn eat(&mut self, token: Token) -> bool {
        if !self.at(token) {
            return false;
        }
        self.do_bump();
        true
    }

    pub fn expect(&mut self, token: Token) {
        if !self.eat(token) {
            self.error(format!("expected `{}`", token.as_str()));
        }
    }

    pub fn bump(&mut self, token: Token) {
        assert!(self.eat(token));
    }

    pub fn error_bump(&mut self, msg: impl Into<StringOrStr>) {
        self.error_recover(msg, TokenSet::empty())
    }

    pub fn error_recover(&mut self, msg: impl Into<StringOrStr>, recovery: TokenSet) {
        if self.at_set(recovery) {
            self.error(msg);
            return;
        }

        let m = self.start();
        self.error(msg);
        self.bump_any();
        m.complete(self, SyntaxKind::ERROR);
    }

    pub fn sync_to(&mut self, token_set: TokenSet) {
        if self.at_set(token_set) || self.at(Token::Eof) {
            return;
        }
        let m = self.start();
        while !self.at_set(token_set) && !self.at(Token::Eof) {
            self.bump_any();
        }
        m.complete(self, SyntaxKind::ERROR);
    }

    //@tweak where error is displayed (currently next or last token)
    // might display it right after current token with empty range (maybe for some tokens like , ; etc)
    pub fn error(&mut self, msg: impl Into<StringOrStr>) {
        let range = if self.at(T![eof]) {
            self.tokens.token_range(self.cursor.dec())
        } else {
            self.tokens.token_range(self.cursor)
        };

        let src = SourceRange::new(self.module_id, range);
        self.errors.push(ErrorComp::new(msg, src, None));
    }

    fn bump_any(&mut self) {
        if self.peek() == Token::Eof {
            return;
        }
        self.do_bump();
    }

    fn do_bump(&mut self) {
        self.cursor = self.cursor.inc();
        self.step_reset();
        self.push_event(Event::Token);
    }

    #[must_use]
    pub fn start(&mut self) -> Marker {
        let event_idx = self.events.len() as u32;
        self.push_event(Event::StartNode {
            kind: SyntaxKind::TOMBSTONE,
            forward_parent: None,
        });
        Marker::new(event_idx)
    }

    #[must_use]
    pub fn start_before(&mut self, m: MarkerClosed) -> Marker {
        let event_idx = self.events.len() as u32;
        self.push_event(Event::StartNode {
            kind: SyntaxKind::TOMBSTONE,
            forward_parent: None,
        });
        match &mut self.events[m.index()] {
            Event::StartNode { forward_parent, .. } => {
                assert!(forward_parent.is_none());
                *forward_parent = Some(event_idx);
            }
            _ => unreachable!(),
        }
        Marker::new(event_idx)
    }

    fn push_event(&mut self, event: Event) {
        self.events.push(event);
    }

    fn step_reset(&self) {
        self.steps.set(0);
    }

    fn step_bump(&self) {
        self.steps.set(self.steps.get() + 1);
        assert!(self.steps.get() < 1_000_000, "parser is stuck");
    }
}

impl Marker {
    fn new(event_idx: u32) -> Marker {
        Marker {
            handled: false,
            event_idx,
        }
    }

    fn index(&self) -> usize {
        self.event_idx as usize
    }

    pub fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> MarkerClosed {
        self.handled = true;
        match &mut p.events[self.index()] {
            Event::StartNode { kind: start, .. } => *start = kind,
            _ => unreachable!(),
        }
        p.push_event(Event::EndNode);
        MarkerClosed::new(self.event_idx)
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        assert!(self.handled, "marker must be completed or abandoned");
    }
}

impl MarkerClosed {
    fn new(event_idx: u32) -> MarkerClosed {
        MarkerClosed { event_idx }
    }

    fn index(&self) -> usize {
        self.event_idx as usize
    }
}
