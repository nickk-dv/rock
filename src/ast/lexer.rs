use super::span::Span;
use super::token::*;
use std::{iter::Peekable, str::Chars};

pub struct LexResult {
    pub tokens: Vec<TokenSpan>,
    pub line_spans: Vec<Span>,
}

pub fn lex(str: &str) -> LexResult {
    let mut lexer = Lexer::new(str);
    LexResult {
        tokens: lexer.lex(),
        line_spans: lexer.lex_line_spans(),
    }
}

struct Lexer<'src> {
    str: &'src str,
    iter: Peekable<Chars<'src>>,
    fc: char,
    cursor_start: u32,
    cursor_end: u32,
}

enum Lexeme {
    Char,
    String { multiline: bool },
    RawString { multiline: bool },
    Ident,
    Number,
    Symbol,
}

impl Lexeme {
    fn from_char(c: char) -> Self {
        match c {
            '\'' => Lexeme::Char,
            '"' => Lexeme::String { multiline: false },
            '`' => Lexeme::RawString { multiline: false },
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
    fn new(str: &'src str) -> Self {
        Self {
            str,
            iter: str.chars().peekable(),
            fc: '?',
            cursor_start: 0,
            cursor_end: 0,
        }
    }

    fn lex(&mut self) -> Vec<TokenSpan> {
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
            Lexeme::Char => self.lex_char(),
            Lexeme::String { multiline } => self.lex_string(),
            Lexeme::RawString { multiline } => self.lex_raw_string(),
            Lexeme::Ident => self.lex_ident(),
            Lexeme::Number => self.lex_number(),
            Lexeme::Symbol => self.lex_symbol(),
        }
    }

    fn lex_escape(&mut self) -> Result<char, bool> {
        if let Some(c) = self.peek() {
            self.consume(c);
            match c {
                'n' => Ok('\n'),
                't' => Ok('\t'),
                'r' => Ok('\r'),
                '0' => Ok('\0'),
                '\\' => Ok('\\'),
                '\'' => Ok('\''),
                '\"' => Ok('\"'),
                _ => Err(true),
            }
        } else {
            Err(false)
        }
    }

    fn lex_char(&mut self) -> TokenSpan {
        self.consume(self.fc);

        let fc = match self.peek() {
            Some(c) => {
                self.consume(c);
                c
            }
            None => panic!("missing char lit character"),
        };

        let mut char_quote = false;
        let char = match fc {
            '\\' => match self.lex_escape() {
                Ok(char) => char,
                Err(invalid) => {
                    if invalid {
                        panic!("char lit invalid escape sequence");
                    }
                    panic!("char lit incomplete escape sequence");
                }
            },
            '\'' => {
                char_quote = true;
                fc
            }
            _ => fc,
        };

        let has_escape = matches!(self.peek(), Some('\''));
        if has_escape {
            self.consume('\'');
        }

        if char_quote && !has_escape {
            // example [ '' ]
            panic!("char lit cannot be empty");
        }
        if char_quote && has_escape {
            // example [ ''' ]
            panic!("char lit ' must be escaped: '\\''");
        }
        if !char_quote && !has_escape {
            // example [ 'x ]
            panic!("char lit not terminated, missing closing `'`");
        }
        return self.token_spanned(Token::LitChar(char));
    }

    fn lex_string(&mut self) -> TokenSpan {
        self.consume(self.fc);

        //@speed not the fastest way to do it...
        let mut string = String::new();
        let mut terminated = false;

        while let Some(c) = self.peek() {
            match c {
                '\n' => break,
                '\"' => {
                    self.consume(c);
                    terminated = true;
                    break;
                }
                '\\' => {
                    self.consume(c);
                    let char = match self.lex_escape() {
                        Ok(char) => char,
                        Err(invalid) => {
                            if invalid {
                                panic!("string lit invalid escape sequence");
                            }
                            panic!("string lit incomplete escape sequence");
                        }
                    };
                    string.push(char);
                }
                _ => {
                    self.consume(c);
                    string.push(c);
                }
            }
        }

        if !terminated {
            panic!("string lit not terminated, missing closing `\"`");
        }
        return self.token_spanned(Token::LitString);
    }

    fn lex_raw_string(&mut self) -> TokenSpan {
        self.consume(self.fc);

        //@speed not the fastest way to do it...
        let mut string = String::new();
        let mut terminated = false;

        while let Some(c) = self.peek() {
            match c {
                '\n' => break,
                '`' => {
                    self.consume(c);
                    terminated = true;
                    break;
                }
                _ => {
                    self.consume(c);
                    string.push(c);
                }
            }
        }

        if !terminated {
            panic!("raw string lit not terminated, missing closing `");
        }
        return self.token_spanned(Token::LitString);
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

    fn lex_symbol(&mut self) -> TokenSpan {
        self.consume(self.fc);
        let mut kind = Token::Error;

        match Token::glue(self.fc) {
            Some(sym) => {
                kind = sym;
            }
            None => {
                //@emit error on invalid token?
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
