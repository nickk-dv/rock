use super::token::*;
use std::{iter::Peekable, str::CharIndices};

enum Lexeme {
    Ident,
    Number,
    String,
    Symbol,
}

pub struct Lexer<'src> {
    str: &'src str,
    iter: Peekable<CharIndices<'src>>,
    start_ch: char,
    start_span: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(str: &'src str) -> Self {
        Self {
            str,
            iter: str.char_indices().peekable(),
            start_ch: '0',
            start_span: 0,
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
        return tokens;
    }

    fn skip_whitespace(&mut self) {
        while let Some((_, c)) = self.iter.peek() {
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
                self.start_ch = *c;
                self.start_span = *start as u32;
                true
            }
            None => false,
        }
    }

    fn lex_token(&mut self) -> Token {
        let c = self.start_ch;
        if c.is_ascii_alphabetic() || c == '_' {
            return self.lex_ident();
        }
        if c.is_ascii_digit() {
            return self.lex_number();
        }
        if c == '"' {
            return self.lex_string();
        }
        return self.lex_symbol();
    }

    fn lex_ident(&mut self) -> Token {
        let mut end = self.start_span;
        while let Some((_, c)) = self.iter.peek() {
            if c.is_alphanumeric() || *c == '_' {
                end += c.len_utf8() as u32;
                self.iter.next();
            } else {
                break;
            }
        }
        self.print_substring(self.start_span, end); //@temp

        let mut kind = match &self.str[self.start_span as usize..end as usize] {
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
            "switch" => TokenKind::KwSwitch,
            "continue" => TokenKind::KwContinue,
            "cast" => TokenKind::KwCast,
            "sizeof" => TokenKind::KwSizeof,
            "true" => TokenKind::KwTrue,
            "false" => TokenKind::KwFalse,
            "s8" => TokenKind::KwS8,
            "s16" => TokenKind::KwS16,
            "s32" => TokenKind::KwS32,
            "s64" => TokenKind::KwS64,
            "u8" => TokenKind::KwU8,
            "u16" => TokenKind::KwU16,
            "u32" => TokenKind::KwU32,
            "u64" => TokenKind::KwU64,
            "f32" => TokenKind::KwF32,
            "f64" => TokenKind::KwF64,
            "bool" => TokenKind::KwBool,
            "string" => TokenKind::KwString,
            _ => TokenKind::Ident,
        };

        kind = match kind {
            TokenKind::KwTrue => TokenKind::LitBool(true),
            TokenKind::KwFalse => TokenKind::LitBool(false),
            _ => kind,
        };

        return Token::new(self.start_span, end, kind);
    }

    fn print_substring(&self, start: u32, end: u32) {
        if let Some(substring) = self.str.get(start as usize..end as usize) {
            println!("Substring: {}", substring);
        } else {
            println!("Invalid range for substring");
        }
    }

    fn lex_number(&mut self) -> Token {
        self.iter.next();
        return Token::new(0, 0, TokenKind::Error);
    }

    fn lex_string(&mut self) -> Token {
        self.iter.next();
        return Token::new(0, 0, TokenKind::Error);
    }

    fn lex_symbol(&mut self) -> Token {
        self.iter.next();
        let glue = Self::lex_symbol_glue(self.start_ch);
        if glue.is_none() {
            return Token::new(self.start_span, 0, TokenKind::Error);
        }
        return Token::new(self.start_span, 0, glue.unwrap_or(TokenKind::Error));
    }

    fn lex_symbol_glue(c: char) -> Option<TokenKind> {
        match c {
            '(' => Some(TokenKind::OpenParen),
            '{' => Some(TokenKind::OpenBlock),
            '[' => Some(TokenKind::OpenBracket),
            ')' => Some(TokenKind::CloseParen),
            ']' => Some(TokenKind::CloseBlock),
            '}' => Some(TokenKind::CloseBracket),
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
