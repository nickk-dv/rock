use super::span::Span;
use super::token::*;
use std::{iter::Peekable, str::Chars};

pub struct Lexer<'src> {
    str: &'src str,
    iter: Peekable<Chars<'src>>,
    fc: char,
    cursor_start: u32,
    cursor_end: u32,
}

enum Lexeme {
    Ident,
    Number,
    String,
    Symbol,
}

impl Lexeme {
    pub fn from_char(c: char) -> Self {
        match c {
            '"' => Lexeme::String,
            _ => {
                if c.is_ascii_alphabetic() || c == '_' {
                    return Lexeme::Ident;
                }
                if c.is_ascii_digit() {
                    return Lexeme::Number;
                }
                return Lexeme::Symbol;
            }
        }
    }
}

impl<'src> Lexer<'src> {
    pub fn new(str: &'src str) -> Self {
        Self {
            str,
            iter: str.chars().peekable(),
            fc: ' ',
            cursor_start: 0,
            cursor_end: 0,
        }
    }

    pub fn lex(&mut self) -> Vec<TokenSpan> {
        let mut tokens = Vec::new();

        while self.peek().is_some() {
            self.skip_whitespace();
            if !self.expect_token() {
                break;
            }
            tokens.push(self.lex_token());
        }

        tokens.push(TokenSpan::eof());
        tokens.push(TokenSpan::eof());
        tokens.push(TokenSpan::eof());
        tokens.push(TokenSpan::eof());
        return tokens;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.consume(c);
            } else {
                break;
            }
        }
    }

    fn expect_token(&mut self) -> bool {
        match self.peek() {
            Some(c) => {
                self.fc = c;
                self.cursor_start = self.cursor_end;
                true
            }
            None => false,
        }
    }

    fn lex_token(&mut self) -> TokenSpan {
        match Lexeme::from_char(self.fc) {
            Lexeme::Ident => self.lex_ident(),
            Lexeme::Number => self.lex_number(),
            Lexeme::String => self.lex_string(),
            Lexeme::Symbol => self.lex_symbol(),
        }
    }

    fn lex_ident(&mut self) -> TokenSpan {
        self.consume(self.fc);
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.consume(c);
            } else {
                break;
            }
        }

        let str = &self.str[self.cursor_start as usize..self.cursor_end as usize];
        let kind = match Token::keyword_from_str(str) {
            Some(kind) => kind,
            _ => Token::Ident,
        };

        return self.token_spanned(kind);
    }

    fn lex_number(&mut self) -> TokenSpan {
        //@todo float detection & parsing

        self.consume(self.fc);
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.consume(c);
            } else {
                break;
            }
        }

        let int_result =
            self.str[self.cursor_start as usize..self.cursor_end as usize].parse::<u64>();
        let kind = match int_result {
            Ok(int) => Token::LitInt(int),
            Err(_) => Token::Error,
        };

        return self.token_spanned(kind);
    }

    fn lex_string(&mut self) -> TokenSpan {
        //@todo terminator missing err
        //@todo escape sequences
        //@todo saving the strings

        self.consume(self.fc);
        while let Some(c) = self.peek() {
            match c {
                '\n' => break,
                '"' => {
                    self.consume(c);
                    break;
                }
                _ => self.consume(c),
            }
        }

        return self.token_spanned(Token::LitString);
    }

    fn lex_symbol(&mut self) -> TokenSpan {
        self.consume(self.fc);
        let mut kind = Token::Error;

        match Token::glue(self.fc) {
            Some(sym) => {
                kind = sym;
            }
            None => {
                return self.token_spanned(kind);
            }
        }

        if let Some(c) = self.peek() {
            match Token::glue2(c, kind) {
                Some(sym) => {
                    self.consume(c);
                    kind = sym;
                }
                None => {
                    return self.token_spanned(kind);
                }
            }
        } else {
            return self.token_spanned(kind);
        }

        if let Some(c) = self.peek() {
            match Token::glue3(c, kind) {
                Some(sym) => {
                    self.consume(c);
                    kind = sym;
                }
                None => {
                    return self.token_spanned(kind);
                }
            }
        } else {
            return self.token_spanned(kind);
        }

        return self.token_spanned(kind);
    }

    pub fn lex_line_spans(&mut self) -> Vec<Span> {
        let mut line_spans = Vec::new();
        let mut line_start = 0;

        self.cursor_end = 0;
        self.iter = self.str.chars().peekable();

        while let Some(c) = self.peek() {
            self.consume(c);
            if c == '\n' {
                line_spans.push(Span::new(line_start, self.cursor_end - 1));
                line_start = self.cursor_end;
            }
        }

        if line_start < self.str.len() as u32 {
            line_spans.push(Span::new(line_start, self.cursor_end));
        }
        return line_spans;
    }

    fn peek(&mut self) -> Option<char> {
        match self.iter.peek() {
            Some(c) => Some(*c),
            None => None,
        }
    }

    fn consume(&mut self, c: char) {
        self.iter.next();
        self.cursor_end += c.len_utf8() as u32;
    }

    fn token_spanned(&mut self, token: Token) -> TokenSpan {
        TokenSpan::new(Span::new(self.cursor_start, self.cursor_end), token)
    }
}
