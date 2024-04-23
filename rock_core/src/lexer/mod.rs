mod grammar;

use crate::error::ErrorComp;
use crate::session::FileID;
use crate::text::{TextOffset, TextRange};
use crate::token::token_list::TokenList;
use std::{iter::Peekable, str::Chars};

pub fn lex(
    source: &str,
    file_id: FileID,
    lex_whitespace: bool,
) -> Result<TokenList, Vec<ErrorComp>> {
    let mut lex = Lexer::new(source, file_id, lex_whitespace);
    grammar::source_file(&mut lex);

    if lex.errors.is_empty() {
        Ok(lex.tokens)
    } else {
        Err(lex.errors)
    }
}

struct Lexer<'src> {
    cursor: TextOffset,
    source: &'src str,
    chars: Peekable<Chars<'src>>,
    tokens: TokenList,
    errors: Vec<ErrorComp>,
    file_id: FileID,
    with_whitespace: bool,
}

impl<'src> Lexer<'src> {
    fn new(source: &'src str, file_id: FileID, with_whitespace: bool) -> Lexer {
        Lexer {
            cursor: 0.into(),
            source,
            chars: source.chars().peekable(),
            tokens: TokenList::new(0), //@no cap estimation
            errors: Vec::new(),
            file_id,
            with_whitespace,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().copied()
    }

    fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.peek().copied()
    }

    fn eat(&mut self, c: char) {
        self.cursor += (c.len_utf8() as u32).into();
        self.chars.next();
    }

    fn start_range(&self) -> TextOffset {
        self.cursor
    }

    fn make_range(&self, start: TextOffset) -> TextRange {
        TextRange::new(start, self.cursor)
    }

    fn error(&mut self, error: ErrorComp) {
        self.errors.push(error);
    }
}
