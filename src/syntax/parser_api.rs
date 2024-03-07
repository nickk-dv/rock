use super::event::Event;
use super::syntax_tree::SyntaxNodeKind;
use super::token_set::TokenSet;
use crate::ast::token::Token;
use std::cell::Cell;

pub struct Parser {
    pos: usize,
    input: Vec<Token>,
    events: Vec<Event>,
    steps: Cell<u32>,
}

pub struct Marker {
    handled: bool,
    event_index: u32,
}

impl Parser {
    pub fn new(input: Vec<Token>) -> Parser {
        Parser {
            pos: 0,
            input,
            events: Vec::new(),
            steps: Cell::new(0),
        }
    }

    pub fn finish(self) -> Vec<Event> {
        self.events
    }

    pub fn at(&self, token: Token) -> bool {
        self.peek() == token
    }

    pub fn at_set(&self, token_set: TokenSet) -> bool {
        token_set.contains(self.peek())
    }

    pub fn peek(&self) -> Token {
        self.step_bump();
        self.input.get(self.pos).cloned().unwrap()
    }

    pub fn peek_next(&self) -> Token {
        self.step_bump();
        self.input.get(self.pos + 1).cloned().unwrap()
    }

    pub fn eat(&mut self, token: Token) -> bool {
        if !self.at(token) {
            return false;
        }
        self.do_bump(token);
        true
    }

    pub fn bump(&mut self, token: Token) {
        assert!(self.eat(token));
    }

    pub fn start(&mut self) -> Marker {
        let event_pos = self.events.len() as u32;
        self.push_event(Event::StartNode {
            kind: SyntaxNodeKind::TOMBSTONE,
        });
        Marker::new(event_pos)
    }

    fn do_bump(&mut self, token: Token) {
        self.step_reset();
        self.pos += 1;
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
        assert!(self.steps.get() < 16_000_000, "parser is stuck");
    }
}

impl Marker {
    fn new(event_index: u32) -> Marker {
        Marker {
            handled: false,
            event_index,
        }
    }

    pub fn complete(mut self, p: &mut Parser, kind: SyntaxNodeKind) {
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
                    kind: SyntaxNodeKind::TOMBSTONE,
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
