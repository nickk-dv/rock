use crate::error::{ErrorComp, SourceRange};
use crate::session::FileID;
use crate::text::TextRange;
use crate::token::token_list::TokenList;
use crate::token::Token;
use std::{iter::Peekable, str::Chars};

//@lexer needs a re-write @07.04.24
// error handling is annoying
// most errors currently panic!()
// support multiline strings, raw, c_strings
// validate that no null terminators present in c_strings (it would be auto inserted)

// @think about whether we need to add \0 to strings, @07.04.24
// instead of having llvm insert those as it already does
// having \0 might reduce interning deduplication in case of same C and normal strings
// (dont add \0 most likely)

pub struct Lexer<'src> {
    source: &'src str,
    chars: Peekable<Chars<'src>>,
    range: TextRange,
    file_id: FileID,
    errors: Vec<ErrorComp>,
    lex_whitespace: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str, file_id: FileID, lex_whitespace: bool) -> Self {
        Self {
            source,
            chars: source.chars().peekable(),
            range: TextRange::empty_at(0.into()),
            file_id,
            errors: Vec::new(),
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
        self.range.extend_by((c.len_utf8() as u32).into());
        self.chars.next();
    }

    #[inline]
    fn token_range(&self) -> TextRange {
        self.range
    }

    pub fn lex(mut self) -> Result<TokenList, Vec<ErrorComp>> {
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
                        let res = self.lex_string(false);
                        tokens.add_string(res.0, res.1, res.2);
                    }
                    '`' => {
                        let res = self.lex_raw_string();
                        tokens.add_string(res.0, false, res.1);
                    }
                    'c' => match self.peek() {
                        Some('\"') => {
                            self.eat(c); // eat "
                            let res = self.lex_string(true);
                            tokens.add_string(res.0, res.1, res.2);
                        }
                        Some('`') => {
                            self.eat(c); // eat `
                            let res = self.lex_raw_string();
                            tokens.add_string(res.0, false, res.1);
                        }
                        _ => {
                            let res = self.lex_ident();
                            tokens.add_token(res.0, res.1);
                        }
                    },
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

        if self.errors.is_empty() {
            Ok(tokens)
        } else {
            Err(self.errors)
        }
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
            self.eat(c); //@always eat?
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
            None => {
                self.errors.push(ErrorComp::error(
                    "expected character literal",
                    SourceRange::new(self.token_range(), self.file_id),
                    None,
                ));
                return (' ', self.token_range());
            }
        };

        let mut terminated = false;
        let char = match fc {
            '\\' => {
                //@self.eat(fc);
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

    fn lex_string(&mut self, c_string: bool) -> (String, bool, TextRange) {
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
            self.errors.push(ErrorComp::error(
                "string literal not terminated, missing closing \"",
                SourceRange::new(self.token_range(), self.file_id),
                None,
            ));
        }

        (string, c_string, self.token_range())
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
            self.errors.push(ErrorComp::error(
                "raw string literal not terminated, missing closing `",
                SourceRange::new(self.token_range(), self.file_id),
                None,
            ));
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
        let mut token = match Token::from_char(fc) {
            Some(sym) => sym,
            None => return (Token::Error, self.token_range()),
        };
        match self.peek() {
            Some(c) => match Token::glue_double(c, token) {
                Some(sym) => {
                    self.eat(c);
                    token = sym;
                }
                None => return (token, self.token_range()),
            },
            None => return (token, self.token_range()),
        }
        match self.peek() {
            Some(c) => match Token::glue_triple(c, token) {
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
}
