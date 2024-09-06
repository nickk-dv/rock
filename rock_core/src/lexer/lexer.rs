use crate::error::ErrorComp;
use crate::session::ModuleID;
use crate::text::{TextOffset, TextRange};
use crate::token::TokenList;
use std::{iter::Peekable, str::Chars};

pub struct Lexer<'src> {
    cursor: TextOffset,
    chars: Peekable<Chars<'src>>,
    tokens: TokenList,
    pub errors: Vec<ErrorComp>,
    pub source: &'src str,
    pub module_id: ModuleID,
    pub with_trivia: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str, module_id: ModuleID, with_trivia: bool) -> Lexer {
        Lexer {
            cursor: 0.into(),
            chars: source.chars().peekable(),
            tokens: TokenList::new(0), //@no cap estimation
            errors: Vec::new(),
            source,
            module_id,
            with_trivia,
        }
    }

    pub fn finish(self) -> (TokenList, Vec<ErrorComp>) {
        (self.tokens, self.errors)
    }

    pub fn start_range(&self) -> TextOffset {
        self.cursor
    }

    pub fn make_range(&self, start: TextOffset) -> TextRange {
        TextRange::new(start, self.cursor)
    }

    pub fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    pub fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.peek().copied()
    }

    pub fn eat(&mut self, c: char) {
        self.cursor += (c.len_utf8() as u32).into();
        self.chars.next();
    }

    pub fn tokens(&mut self) -> &mut TokenList {
        &mut self.tokens
    }
}
