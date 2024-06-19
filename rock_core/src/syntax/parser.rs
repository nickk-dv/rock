use super::syntax_tree::SyntaxKind;
use super::token_set::TokenSet;
use crate::token::token_list::TokenList;
use crate::token::Token;
use std::cell::Cell;

pub struct Parser {
    cursor: usize,
    input: TokenList,
    events: Vec<Event>,
    steps: Cell<u32>,
}

pub enum Event {
    StartNode { kind: SyntaxKind },
    EndNode,
    Token { token: Token },
    Error { message: String },
}

pub struct Marker {
    handled: bool,
    event_index: u32,
}

impl Parser {
    pub fn new(input: TokenList) -> Parser {
        Parser {
            cursor: 0,
            input,
            events: Vec::new(),
            steps: Cell::new(0),
        }
    }

    pub fn finish(self) -> Vec<Event> {
        self.events
    }

    pub fn sync_to(&mut self, token_set: TokenSet) {
        while !self.at_set(token_set) && !self.at(Token::Eof) {
            self.bump_any();
        }
    }

    pub fn at(&self, token: Token) -> bool {
        self.peek() == token
    }

    pub fn at_next(&self, token: Token) -> bool {
        self.peek_next() == token
    }

    pub fn at_prev(&self, token: Token) -> bool {
        self.input.get_token(self.cursor - 1) == token
    }

    pub fn at_set(&self, token_set: TokenSet) -> bool {
        token_set.contains(self.peek())
    }

    pub fn peek(&self) -> Token {
        self.step_bump();
        self.input.get_token(self.cursor)
    }

    pub fn peek_next(&self) -> Token {
        self.step_bump();
        self.input.get_token(self.cursor + 1)
    }

    pub fn eat(&mut self, token: Token) -> bool {
        if !self.at(token) {
            return false;
        }
        self.do_bump(token);
        true
    }

    pub fn expect(&mut self, token: Token) {
        if !self.eat(token) {
            self.error(format!("expected {}", token.as_str()));
        }
    }

    pub fn bump(&mut self, token: Token) {
        assert!(self.eat(token));
    }

    pub fn error_bump<S: Into<String>>(&mut self, message: S) {
        self.error_recover(message, TokenSet::EMPTY)
    }

    pub fn error_recover<S: Into<String>>(&mut self, message: S, recovery: TokenSet) {
        if self.at_set(recovery) {
            self.error(message);
            return;
        }

        let m = self.start();
        self.error(message);
        self.bump_any();
        m.complete(self, SyntaxKind::ERROR);
    }

    #[must_use]
    pub fn start(&mut self) -> Marker {
        let event_pos = self.events.len() as u32;
        self.push_event(Event::StartNode {
            kind: SyntaxKind::TOMBSTONE,
        });
        Marker::new(event_pos)
    }

    pub fn error<S: Into<String>>(&mut self, message: S) {
        self.push_event(Event::Error {
            message: message.into(),
        });
    }

    fn bump_any(&mut self) {
        if self.peek() == Token::Eof {
            return;
        }
        self.do_bump(self.peek());
    }

    fn do_bump(&mut self, token: Token) {
        self.cursor += 1;
        self.step_reset();
        self.push_event(Event::Token { token });
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
    fn new(event_index: u32) -> Marker {
        Marker {
            handled: false,
            event_index,
        }
    }

    pub fn complete(mut self, p: &mut Parser, kind: SyntaxKind) {
        self.handled = true;
        match &mut p.events[self.index()] {
            Event::StartNode { kind: start } => *start = kind,
            _ => unreachable!(),
        }
        p.push_event(Event::EndNode);
    }

    pub fn abandon(mut self, p: &mut Parser) {
        self.handled = true;
        if self.index() == p.events.len() - 1 {
            match p.events.pop() {
                Some(Event::StartNode {
                    kind: SyntaxKind::TOMBSTONE,
                }) => {}
                _ => unreachable!(),
            }
        }
    }

    fn index(&self) -> usize {
        self.event_index as usize
    }
}

impl Drop for Marker {
    fn drop(&mut self) {
        assert!(self.handled, "marker must be completed or abandoned");
    }
}
