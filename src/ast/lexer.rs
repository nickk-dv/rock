use super::token::*;
use super::token_list::*;
use crate::text::TextRange;
use std::{iter::Peekable, str::Chars};

pub struct Lexer<'src> {
    source: &'src str,
    chars: Peekable<Chars<'src>>,
    range: TextRange,
    lex_whitespace: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str, lex_whitespace: bool) -> Self {
        Self {
            source,
            chars: source.chars().peekable(),
            range: TextRange::empty_at(0.into()),
            lex_whitespace,
        }
    }

    #[inline]
    fn peek(&mut self) -> Option<char> {
        self.chars.peek().cloned()
    }

    #[inline]
    fn peek_next(&self) -> Option<char> {
        let mut iter = self.chars.clone();
        iter.next();
        iter.peek().cloned()
    }

    #[inline]
    fn eat(&mut self, c: char) {
        self.range.extend_by(c.len_utf8().try_into().unwrap());
        self.chars.next();
    }

    #[inline]
    fn token_range(&self) -> TextRange {
        self.range
    }

    pub fn lex(mut self) -> TokenList {
        let init_cap = self.source.len() / 8;
        let mut tokens = TokenList::new(init_cap);

        while self.peek().is_some() {
            self.skip_whitespace(&mut tokens);
            if let Some(c) = self.peek() {
                self.range = TextRange::empty_at(self.range.end());
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
                        tokens.add_token(res.0, res.1);
                    }
                }
            }
        }
        // @define parser lookead and assert on it in peek_next() in parser
        // and use it in a loop here
        for _ in 0..4 {
            tokens.add_token(Token::Eof, TextRange::empty_at(0.into()));
        }
        tokens
    }

    fn skip_whitespace(&mut self, tokens: &mut TokenList) {
        if self.lex_whitespace {
            self.range = TextRange::empty_at(self.range.end());
        }
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
        if self.lex_whitespace && self.token_range().len() > 0 {
            tokens.add_token(Token::Whitespace, self.token_range())
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

    fn lex_char(&mut self) -> (char, TextRange) {
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

        (char, self.token_range())
    }

    fn lex_string(&mut self) -> (String, TextRange) {
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
                                eprintln!("at range: {}", &self.source[self.range.as_usize()]);
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

        (string, self.token_range())
    }

    fn lex_raw_string(&mut self) -> (String, TextRange) {
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

        (string, self.token_range())
    }

    fn lex_number(&mut self) -> (Token, TextRange) {
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.eat(c);
            } else if c == '.' && !is_float {
                match self.peek_next() {
                    Some(next) => {
                        if next.is_ascii_digit() {
                            is_float = true;
                            self.eat(c);
                        } else {
                            break;
                        }
                    }
                    None => break,
                }
            } else {
                break;
            }
        }

        match is_float {
            true => (Token::FloatLit, self.token_range()),
            false => (Token::IntLit, self.token_range()),
        }
    }

    fn lex_ident(&mut self) -> (Token, TextRange) {
        while let Some(c) = self.peek() {
            if c == '_' || c.is_ascii_digit() || c.is_alphabetic() {
                self.eat(c);
            } else {
                break;
            }
        }

        let string = &self.source[self.range.as_usize()];
        match Token::as_keyword(string) {
            Some(token) => (token, self.token_range()),
            None => (Token::Ident, self.token_range()),
        }
    }

    fn lex_symbol(&mut self, fc: char) -> (Token, TextRange) {
        let mut token = match Token::glue(fc) {
            Some(sym) => sym,
            None => return (Token::Error, self.token_range()),
        };
        match self.peek() {
            Some(c) => match Token::glue2(c, token) {
                Some(sym) => {
                    self.eat(c);
                    token = sym;
                }
                None => return (token, self.token_range()),
            },
            None => return (token, self.token_range()),
        }
        match self.peek() {
            Some(c) => match Token::glue3(c, token) {
                Some(sym) => {
                    self.eat(c);
                    token = sym;
                }
                None => return (token, self.token_range()),
            },
            None => return (token, self.token_range()),
        }
        (token, self.token_range())
    }

    //@overall this should be removed in favor of binary search over the text
    // to find line + col of requested range
    pub fn lex_line_ranges(&mut self) -> Vec<TextRange> {
        self.range = TextRange::empty_at(0.into());
        self.chars = self.source.chars().peekable();
        let mut line_ranges = Vec::new();
        let mut ate: bool = false; //@find better way to create ranges without /n

        while let Some(c) = self.peek() {
            if c == '\n' {
                line_ranges.push(self.range);
                self.eat(c);
                self.range = TextRange::empty_at(self.range.end());
                ate = false;
            } else {
                self.eat(c);
                ate = true;
            }
        }

        if ate {
            line_ranges.push(self.range);
        }
        line_ranges
    }
}
