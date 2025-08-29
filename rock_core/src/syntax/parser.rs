use super::token::{Token, TokenID, TokenList, TokenSet};
use super::tree::SyntaxKind;
use crate::error::{Error, ErrorBuffer, ErrorSink, SourceRange, StringOrStr};
use crate::session::ModuleID;
use std::cell::Cell;

pub struct Parser<'src> {
    cursor: TokenID,
    tokens: TokenList,
    events: Vec<Event>,
    errors: ErrorBuffer,
    steps: Cell<u32>,
    module_id: ModuleID,
    source: &'src str,
}

#[derive(Copy, Clone)]
pub enum Event {
    StartNode { kind: SyntaxKind, forward_parent: Option<u32> },
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

impl<'src> Parser<'src> {
    pub fn new(tokens: TokenList, module_id: ModuleID, source: &'src str) -> Parser<'src> {
        Parser {
            cursor: TokenID::new(0),
            tokens,
            events: Vec::new(),
            errors: ErrorBuffer::default(),
            steps: Cell::new(0),
            module_id,
            source,
        }
    }

    pub fn finish(self) -> (TokenList, Vec<Event>, ErrorBuffer) {
        (self.tokens, self.events, self.errors)
    }

    pub fn at(&self, token: Token) -> bool {
        self.peek() == token
    }

    pub fn at_next(&self, token: Token) -> bool {
        self.peek_next() == token
    }

    pub fn at_set(&self, token_set: TokenSet) -> bool {
        token_set.contains(self.peek())
    }

    pub fn at_prev(&self, token: Token) -> bool {
        self.tokens.token(self.cursor.dec()) == token
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
        if self.at(token) {
            self.do_bump();
            true
        } else {
            false
        }
    }

    pub fn expect(&mut self, token: Token) {
        if !self.eat(token) && self.errors.errors.is_empty() {
            self.error(format!("expected `{}`", token.as_str()));
        }
    }

    pub fn bump(&mut self, token: Token) {
        assert!(self.eat(token));
    }

    fn do_bump(&mut self) {
        assert!(!self.at(Token::Eof));
        self.steps.set(0);
        self.cursor = self.cursor.inc();
        self.events.push(Event::Token);
    }

    fn step_bump(&self) {
        self.steps.set(self.steps.get() + 1);
        assert!(self.steps.get() < 1_000_000, "parser is stuck");
    }

    pub fn error_recover(
        &mut self,
        msg: impl Into<StringOrStr>,
        recovery: TokenSet,
    ) -> MarkerClosed {
        self.error(msg);
        let m = self.start();
        if !self.at_set(recovery) && !self.at(Token::Eof) {
            self.do_bump();
        }
        m.complete(self, SyntaxKind::ERROR)
    }

    pub fn error(&mut self, msg: impl Into<StringOrStr>) {
        if self.errors.errors.is_empty() {
            let cursor = if self.at(Token::Eof) { self.cursor.dec() } else { self.cursor };
            let range = self.tokens.token_range(cursor);
            let src = SourceRange::new(self.module_id, range);
            self.errors.error(Error::new(msg, src, None));
        }
    }

    pub fn bump_sync(&mut self, token_set: TokenSet) {
        if self.at_set(token_set) || self.at(Token::Eof) {
            return;
        }
        while !self.at_set(token_set) && !self.at(Token::Eof) {
            self.do_bump();
        }
    }

    pub fn current_token_string(&mut self) -> &str {
        let range = self.tokens.token_range(self.cursor);
        &self.source[range.as_usize()]
    }

    #[must_use]
    pub fn start(&mut self) -> Marker {
        let event_idx = self.events.len() as u32;
        self.events.push(Event::StartNode { kind: SyntaxKind::TOMBSTONE, forward_parent: None });
        Marker::new(event_idx)
    }

    #[must_use]
    pub fn start_before(&mut self, mc: MarkerClosed) -> Marker {
        let event_idx = self.events.len() as u32;
        self.events.push(Event::StartNode { kind: SyntaxKind::TOMBSTONE, forward_parent: None });
        match &mut self.events[mc.event_idx as usize] {
            Event::StartNode { forward_parent, .. } => {
                assert!(forward_parent.is_none());
                *forward_parent = Some(event_idx);
            }
            _ => unreachable!(),
        }
        Marker::new(event_idx)
    }
}

impl Marker {
    fn new(event_idx: u32) -> Marker {
        Marker { handled: false, event_idx }
    }

    pub fn complete(mut self, p: &mut Parser, kind: SyntaxKind) -> MarkerClosed {
        self.handled = true;
        match &mut p.events[self.event_idx as usize] {
            Event::StartNode { kind: start, .. } => *start = kind,
            _ => unreachable!(),
        }
        p.events.push(Event::EndNode);
        MarkerClosed { event_idx: self.event_idx }
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        assert!(self.handled, "internal: marker must be completed");
    }
}
