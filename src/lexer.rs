use super::token::*;
use std::{iter::Peekable, str::CharIndices};

pub struct Lexer<'src> {
    str: &'src str,
    iter: Peekable<CharIndices<'src>>,
    fc: char,
    span: Span,
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
            iter: str.char_indices().peekable(),
            fc: ' ',
            span: Span { start: 0, end: 0 },
        }
    }

    pub fn lex(&mut self) -> Vec<Token> {
        let mut tokens: Vec<Token> = Vec::new();
        while self.iter.peek().is_some() {
            self.skip_whitespace();
            if !self.expect_token() {
                break;
            }
            tokens.push(self.lex_token());
        }
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        tokens.push(Token::eof());
        return tokens;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.iter.next();
            } else {
                break;
            }
        }
    }

    fn expect_token(&mut self) -> bool {
        match self.iter.peek() {
            Some((start, c)) => {
                self.fc = *c;
                self.span.start = *start as u32;
                self.span.end = self.span.start;
                true
            }
            None => false,
        }
    }

    fn peek(&mut self) -> Option<char> {
        match self.iter.peek() {
            Some((_, c)) => Some(*c),
            None => None,
        }
    }

    fn consume(&mut self, c: char) {
        self.iter.next();
        self.span.end += c.len_utf8() as u32;
    }

    fn token_spanned(&self, kind: TokenKind) -> Token {
        Token::new(self.span.start, self.span.end, kind)
    }

    fn lex_token(&mut self) -> Token {
        match Lexeme::from_char(self.fc) {
            Lexeme::Ident => self.lex_ident(),
            Lexeme::Number => self.lex_number(),
            Lexeme::String => self.lex_string(),
            Lexeme::Symbol => self.lex_symbol(),
        }
    }

    fn lex_ident(&mut self) -> Token {
        self.consume(self.fc);
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.consume(c);
            } else {
                break;
            }
        }

        let kind = match &self.str[self.span.start as usize..self.span.end as usize] {
            "null" => TokenKind::LitNull,
            "true" => TokenKind::LitBool(true),
            "false" => TokenKind::LitBool(false),
            "pub" => TokenKind::KwPub,
            "mod" => TokenKind::KwMod,
            "mut" => TokenKind::KwMut,
            "self" => TokenKind::KwSelf,
            "impl" => TokenKind::KwImpl,
            "enum" => TokenKind::KwEnum,
            "struct" => TokenKind::KwStruct,
            "import" => TokenKind::KwImport,
            "if" => TokenKind::KwIf,
            "else" => TokenKind::KwElse,
            "for" => TokenKind::KwFor,
            "defer" => TokenKind::KwDefer,
            "break" => TokenKind::KwBreak,
            "return" => TokenKind::KwReturn,
            "continue" => TokenKind::KwContinue,
            "cast" => TokenKind::KwCast,
            "sizeof" => TokenKind::KwSizeof,
            "bool" => TokenKind::KwBool,
            "s8" => TokenKind::KwS8,
            "s16" => TokenKind::KwS16,
            "s32" => TokenKind::KwS32,
            "s64" => TokenKind::KwS64,
            "ssize" => TokenKind::KwSsize,
            "u8" => TokenKind::KwU8,
            "u16" => TokenKind::KwU16,
            "u32" => TokenKind::KwU32,
            "u64" => TokenKind::KwU64,
            "usize" => TokenKind::KwUsize,
            "f32" => TokenKind::KwF32,
            "f64" => TokenKind::KwF64,
            "char" => TokenKind::KwChar,
            _ => TokenKind::Ident,
        };

        return self.token_spanned(kind);
    }

    pub fn print_substring(&self, start: u32, end: u32) {
        //@use for error reporting
        if let Some(substring) = self.str.get(start as usize..end as usize) {
            println!("Substring: {}", substring);
        } else {
            println!("Invalid range for substring");
        }

        if let Some(substring) = self.str.get((start - 10) as usize..(end + 10) as usize) {
            println!("Substring expanded: {}", substring);
        } else {
            println!("Invalid range for expanded substring");
        }
    }

    fn lex_number(&mut self) -> Token {
        //@todo float detection & parsing

        self.consume(self.fc);
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.consume(c);
            } else {
                break;
            }
        }

        let int_result = self.str[self.span.start as usize..self.span.end as usize].parse::<u64>();
        let kind = match int_result {
            Ok(int) => TokenKind::LitInt(int),
            Err(_) => TokenKind::Error,
        };

        return self.token_spanned(kind);
    }

    fn lex_string(&mut self) -> Token {
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

        return self.token_spanned(TokenKind::LitString);
    }

    fn lex_symbol(&mut self) -> Token {
        self.consume(self.fc);
        let mut kind = TokenKind::Error;

        match Self::lex_symbol_glue(self.fc) {
            Some(sym) => {
                kind = sym;
            }
            None => {
                return self.token_spanned(kind);
            }
        }

        if let Some(c) = self.peek() {
            match Self::lex_symbol_glue2(c, kind) {
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
            match Self::lex_symbol_glue3(c, kind) {
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

    fn lex_symbol_glue(c: char) -> Option<TokenKind> {
        match c {
            '(' => Some(TokenKind::OpenParen),
            ')' => Some(TokenKind::CloseParen),
            '{' => Some(TokenKind::OpenBlock),
            '}' => Some(TokenKind::CloseBlock),
            '[' => Some(TokenKind::OpenBracket),
            ']' => Some(TokenKind::CloseBracket),
            '@' => Some(TokenKind::At),
            '.' => Some(TokenKind::Dot),
            ':' => Some(TokenKind::Colon),
            ',' => Some(TokenKind::Comma),
            ';' => Some(TokenKind::Semicolon),
            '!' => Some(TokenKind::LogicNot),
            '~' => Some(TokenKind::BitNot),
            '<' => Some(TokenKind::Less),
            '>' => Some(TokenKind::Greater),
            '+' => Some(TokenKind::Plus),
            '-' => Some(TokenKind::Minus),
            '*' => Some(TokenKind::Times),
            '/' => Some(TokenKind::Div),
            '%' => Some(TokenKind::Mod),
            '&' => Some(TokenKind::BitAnd),
            '|' => Some(TokenKind::BitOr),
            '^' => Some(TokenKind::BitXor),
            '=' => Some(TokenKind::Assign),
            _ => None,
        }
    }

    fn lex_symbol_glue2(c: char, kind: TokenKind) -> Option<TokenKind> {
        match c {
            '.' => match kind {
                TokenKind::Dot => Some(TokenKind::DotDot),
                _ => None,
            },
            ':' => match kind {
                TokenKind::Colon => Some(TokenKind::ColonColon),
                _ => None,
            },
            '&' => match kind {
                TokenKind::BitAnd => Some(TokenKind::LogicAnd),
                _ => None,
            },
            '|' => match kind {
                TokenKind::BitOr => Some(TokenKind::LogicOr),
                _ => None,
            },
            '<' => match kind {
                TokenKind::Less => Some(TokenKind::Shl),
                _ => None,
            },
            '>' => match kind {
                TokenKind::Minus => Some(TokenKind::ArrowThin),
                TokenKind::Assign => Some(TokenKind::ArrowWide),
                TokenKind::Greater => Some(TokenKind::Shr),
                _ => None,
            },
            '=' => match kind {
                TokenKind::Less => Some(TokenKind::LessEq),
                TokenKind::Greater => Some(TokenKind::GreaterEq),
                TokenKind::LogicNot => Some(TokenKind::NotEq),
                TokenKind::Assign => Some(TokenKind::IsEq),
                TokenKind::Plus => Some(TokenKind::PlusEq),
                TokenKind::Minus => Some(TokenKind::MinusEq),
                TokenKind::Times => Some(TokenKind::TimesEq),
                TokenKind::Div => Some(TokenKind::DivEq),
                TokenKind::Mod => Some(TokenKind::ModEq),
                TokenKind::BitAnd => Some(TokenKind::BitAndEq),
                TokenKind::BitOr => Some(TokenKind::BitOrEq),
                TokenKind::BitXor => Some(TokenKind::BitXorEq),
                _ => None,
            },
            _ => None,
        }
    }

    fn lex_symbol_glue3(c: char, kind: TokenKind) -> Option<TokenKind> {
        match c {
            '=' => match kind {
                TokenKind::Shl => Some(TokenKind::ShlEq),
                TokenKind::Shr => Some(TokenKind::ShrEq),
                _ => None,
            },
            _ => None,
        }
    }
}
