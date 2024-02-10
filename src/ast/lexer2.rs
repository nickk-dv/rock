use super::span::*;
use super::token2::*;
use std::{iter::Peekable, str::Chars};

pub struct Lexer<'src> {
    span_start: u32,
    span_end: u32,
    source: &'src str,
    chars: Peekable<Chars<'src>>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            chars: source.chars().peekable(),
            span_start: 0,
            span_end: 0,
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().cloned()
    }

    fn eat(&mut self, c: char) {
        self.span_end += c.len_utf8() as u32;
        self.chars.next();
    }

    fn span(&self) -> Span {
        Span::new(self.span_start, self.span_end)
    }

    pub fn lex(mut self) -> TokenList {
        let init_cap = self.source.len() / 8;
        let mut tokens = TokenList::new(init_cap);

        while self.peek().is_some() {
            self.skip_whitespace();
            if let Some(c) = self.peek() {
                let token = self.lex_token(c);
                tokens.add(token.0, token.1);
            }
        }
        tokens.add(Token::Eof, Span::new(u32::MAX, u32::MAX));
        tokens.add(Token::Eof, Span::new(u32::MAX, u32::MAX));
        tokens.add(Token::Eof, Span::new(u32::MAX, u32::MAX));
        tokens.add(Token::Eof, Span::new(u32::MAX, u32::MAX));
        return tokens;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.eat(c);
            } else {
                break;
            }
        }
    }

    fn lex_token(&mut self, fc: char) -> (Token, Span) {
        self.span_start = self.span_end;
        if fc == '\"' {
            self.lex_string(fc)
        } else if fc.is_ascii_digit() {
            self.lex_number(fc)
        } else if fc == '_' || fc.is_alphabetic() {
            self.lex_ident(fc)
        } else {
            self.lex_symbol(fc)
        }
    }

    fn lex_string(&mut self, fc: char) -> (Token, Span) {
        self.eat(fc);

        let mut terminated = false;
        while let Some(c) = self.peek() {
            match c {
                '\r' | '\n' => break,
                '\"' => {
                    self.eat(c);
                    terminated = true;
                }
                _ => self.eat(c),
            }
        }

        if terminated {
            (Token::StringLit, self.span())
        } else {
            (Token::Error, self.span())
        }
    }

    fn lex_number(&mut self, fc: char) -> (Token, Span) {
        self.eat(fc);

        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.eat(c);
            } else if c == '.' && !is_float {
                is_float = true;
                self.eat(c);
            } else {
                break;
            }
        }

        match is_float {
            true => (Token::FloatLit, self.span()),
            false => (Token::IntLit, self.span()),
        }
    }

    fn lex_ident(&mut self, fc: char) -> (Token, Span) {
        self.eat(fc);

        while let Some(c) = self.peek() {
            if c == '_' || c.is_ascii_digit() || c.is_alphabetic() {
                self.eat(c);
            } else {
                break;
            }
        }

        let range = self.span_start as usize..self.span_end as usize;
        let slice = unsafe { self.source.get_unchecked(range) };

        match Token::as_keyword(slice) {
            Some(token) => (token, self.span()),
            None => (Token::Ident, self.span()),
        }
    }

    fn lex_symbol(&mut self, fc: char) -> (Token, Span) {
        self.eat(fc);

        let mut token = match Token::glue(fc) {
            Some(sym) => sym,
            None => return (Token::Error, self.span()),
        };
        match self.peek() {
            Some(c) => match Token::glue2(c, token) {
                Some(sym) => {
                    self.eat(c);
                    token = sym;
                }
                None => return (token, self.span()),
            },
            None => return (token, self.span()),
        }
        match self.peek() {
            Some(c) => match Token::glue3(c, token) {
                Some(sym) => {
                    self.eat(c);
                    token = sym;
                }
                None => return (token, self.span()),
            },
            None => return (token, self.span()),
        }
        (token, self.span())
    }
}
