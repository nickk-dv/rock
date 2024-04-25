use crate::error::ErrorComp;
use crate::session::FileID;
use crate::text::{TextOffset, TextRange};
use crate::token::token_list::TokenList;
use std::{iter::Peekable, str::Chars};

pub struct Lexer<'src> {
    cursor: TextOffset,
    chars: Peekable<Chars<'src>>,
    tokens: TokenList,
    errors: Vec<ErrorComp>,
    source: &'src str,
    file_id: FileID,
    with_whitespace: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str, file_id: FileID, with_whitespace: bool) -> Lexer {
        Lexer {
            cursor: 0.into(),
            chars: source.chars().peekable(),
            tokens: TokenList::new(0), //@no cap estimation
            errors: Vec::new(),
            source,
            file_id,
            with_whitespace,
        }
    }

    pub fn finish(self) -> Result<TokenList, Vec<ErrorComp>> {
        if self.errors.is_empty() {
            Ok(self.tokens)
        } else {
            Err(self.errors)
        }
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

    pub fn error(&mut self, error: ErrorComp) {
        self.errors.push(error);
    }

    pub fn source(&self) -> &str {
        self.source
    }

    pub fn file_id(&self) -> FileID {
        self.file_id
    }

    pub fn with_whitespace(&self) -> bool {
        self.with_whitespace
    }
}
