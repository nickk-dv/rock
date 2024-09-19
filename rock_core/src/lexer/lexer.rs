use crate::error::{DiagnosticCollection, ErrorComp, ErrorSink, SourceRange};
use crate::session::ModuleID;
use crate::text::{TextOffset, TextRange};
use crate::token::TokenList;
use std::{iter::Peekable, str::Chars};

pub struct Lexer<'src> {
    cursor: TextOffset,
    chars: Peekable<Chars<'src>>,
    pub tokens: TokenList,
    pub source: &'src str,
    pub module_id: ModuleID,
    pub with_trivia: bool,
    pub buffer: String,
    pub diagnostics: DiagnosticCollection,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str, module_id: ModuleID, with_trivia: bool) -> Lexer {
        Lexer {
            cursor: 0.into(),
            chars: source.chars().peekable(),
            tokens: TokenList::new(0), //@no cap estimation
            source,
            module_id,
            with_trivia,
            buffer: String::with_capacity(64),
            diagnostics: DiagnosticCollection::new(),
        }
    }

    pub fn finish(self) -> (TokenList, Vec<ErrorComp>) {
        (self.tokens, self.diagnostics.errors_moveout())
    }

    pub fn start_range(&self) -> TextOffset {
        self.cursor
    }

    pub fn make_range(&self, start: TextOffset) -> TextRange {
        TextRange::new(start, self.cursor)
    }

    pub fn make_src(&self, start: TextOffset) -> SourceRange {
        let range = TextRange::new(start, self.cursor);
        SourceRange::new(self.module_id, range)
    }

    pub fn at(&mut self, c: char) -> bool {
        self.peek().map(|p| p == c).unwrap_or(false)
    }

    pub fn at_next(&self, c: char) -> bool {
        self.peek_next().map(|p| p == c).unwrap_or(false)
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
}

impl<'src> ErrorSink for Lexer<'src> {
    fn diagnostics(&self) -> &crate::error::DiagnosticCollection {
        &self.diagnostics
    }
    fn diagnostics_mut(&mut self) -> &mut crate::error::DiagnosticCollection {
        &mut self.diagnostics
    }
}
