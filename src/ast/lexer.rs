use super::span::*;
use super::token::*;
use super::token_list::*;
use std::{iter::Peekable, str::Chars};

pub struct Lexer<'src> {
    source: &'src str,
    chars: Peekable<Chars<'src>>,
    span_start: u32,
    span_end: u32,
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

    fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.peek().cloned()
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
                self.span_start = self.span_end;
                self.eat(c);

                match c {
                    '\'' => {
                        let res = self.lex_char();
                        tokens.add_char(res.0, res.1);
                    }
                    '\"' => {
                        let res = self.lex_string();
                        tokens.add_string(res.0, res.1);
                    }
                    '`' => {
                        let res = self.lex_raw_string();
                        tokens.add_string(res.0, res.1);
                    }
                    _ => {
                        let res = if c.is_ascii_digit() {
                            self.lex_number()
                        } else if c == '_' || c.is_alphabetic() {
                            self.lex_ident()
                        } else {
                            self.lex_symbol(c)
                        };
                        tokens.add(res.0, res.1);
                    }
                }
            }
        }
        for _ in 0..4 {
            tokens.add(Token::Eof, Span::new(u32::MAX, u32::MAX));
        }
        return tokens;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.eat(c);
            } else if c == '/' && matches!(self.peek_next(), Some('/')) {
                self.eat(c);
                self.eat('/');
                self.skip_line_comment();
            } else if c == '/' && matches!(self.peek_next(), Some('*')) {
                self.eat(c);
                self.eat('*');
                self.skip_block_comment();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(c) = self.peek() {
            self.eat(c);
            if c == '\n' {
                break;
            }
        }
    }

    fn skip_block_comment(&mut self) {
        let mut depth: i32 = 1;
        while let Some(c) = self.peek() {
            self.eat(c);
            if c == '/' && matches!(self.peek(), Some('*')) {
                self.eat('*');
                depth += 1;
            } else if c == '*' && matches!(self.peek(), Some('/')) {
                self.eat('/');
                depth -= 1;
            }
            if depth == 0 {
                break;
            }
        }
        if depth != 0 {
            panic!("missing block comment terminators `*/` at depth: {}", depth);
        }
    }

    fn lex_escape(&mut self) -> Result<char, bool> {
        if let Some(c) = self.peek() {
            self.eat(c); // always eat?
            match c {
                'n' => Ok('\n'),
                't' => Ok('\t'),
                'r' => Ok('\r'),
                '0' => Ok('\0'),
                '\'' => Ok('\''),
                '\"' => Ok('\"'),
                '\\' => Ok('\\'),
                _ => Err(true),
            }
        } else {
            Err(false)
        }
    }

    fn lex_char(&mut self) -> (char, Span) {
        let fc = match self.peek() {
            Some(c) => {
                self.eat(c);
                c
            }
            None => panic!("missing char lit character"),
        };

        let mut terminated = false;
        let char = match fc {
            '\\' => {
                //self.eat(fc);
                match self.lex_escape() {
                    Ok(char) => char,
                    Err(invalid) => {
                        if invalid {
                            panic!("char lit invalid escape sequence");
                        }
                        panic!("char lit incomplete escape sequence");
                    }
                }
            }
            '\'' => {
                terminated = true;
                fc
            }
            _ => fc,
        };

        let has_escape = matches!(self.peek(), Some('\''));
        if has_escape {
            self.eat('\'');
        }
        if terminated && !has_escape {
            // example [ '' ]
            panic!("char literal cannot be empty");
        }
        if terminated && has_escape {
            // example [ ''' ]
            panic!("char literal `'` must be escaped: `\\'`");
        }
        if !terminated && !has_escape {
            // example [ 'x ]
            panic!("char literal not terminated, missing closing `'`");
        }

        (char, self.span())
    }

    fn lex_string(&mut self) -> (String, Span) {
        let mut string = String::new();
        let mut terminated = false;

        while let Some(c) = self.peek() {
            match c {
                '\r' | '\n' => break,
                '\"' => {
                    self.eat(c);
                    terminated = true;
                    break;
                }
                '\\' => {
                    self.eat(c); // @is this correct?
                    let char = match self.lex_escape() {
                        Ok(char) => char,
                        Err(invalid) => {
                            if invalid {
                                eprintln!("at span: {}", self.span().slice(self.source));
                                panic!("string lit invalid escape sequence");
                            }
                            panic!("string lit incomplete escape sequence");
                        }
                    };
                    string.push(char);
                }
                _ => {
                    self.eat(c);
                    string.push(c);
                }
            }
        }

        if !terminated {
            panic!("string lit not terminated, missing closing `\"`");
        }

        (string, self.span())
    }

    fn lex_raw_string(&mut self) -> (String, Span) {
        let mut string = String::new();
        let mut terminated = false;

        while let Some(c) = self.peek() {
            match c {
                '\r' | '\n' => break,
                '`' => {
                    self.eat(c);
                    terminated = true;
                    break;
                }
                _ => {
                    self.eat(c);
                    string.push(c);
                }
            }
        }

        if !terminated {
            panic!("raw string lit not terminated, missing closing `");
        }

        (string, self.span())
    }

    fn lex_number(&mut self) -> (Token, Span) {
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

    fn lex_ident(&mut self) -> (Token, Span) {
        while let Some(c) = self.peek() {
            if c == '_' || c.is_ascii_digit() || c.is_alphabetic() {
                self.eat(c);
            } else {
                break;
            }
        }

        let slice = self.span().slice(self.source);
        match Token::as_keyword(slice) {
            Some(token) => (token, self.span()),
            None => (Token::Ident, self.span()),
        }
    }

    fn lex_symbol(&mut self, fc: char) -> (Token, Span) {
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

    pub fn lex_line_spans(&mut self) -> Vec<Span> {
        let mut line_spans = Vec::new();
        let mut line_start = 0;

        self.span_end = 0;
        self.chars = self.source.chars().peekable();

        while let Some(c) = self.peek() {
            self.eat(c);
            if c == '\n' {
                line_spans.push(Span::new(line_start, self.span_end - 1));
                line_start = self.span_end;
            }
        }

        if line_start < self.source.len() as u32 {
            line_spans.push(Span::new(line_start, self.span_end));
        }
        return line_spans;
    }
}
