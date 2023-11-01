use super::token::*;

pub const TOKEN_BUFFER_SIZE: usize = 256;
pub const TOKEN_LOOKAHEAD: usize = 4;

#[derive(Default)]
pub struct Lexer<'a> {
    source: &'a [u8],
    cursor: u32,
    line_id: u32,
    line_cursor: u32,
}

fn is_letter(c: u8) -> bool {
    return (c >= b'A' && c < b'Z') || (c >= b'a' && c <= b'z');
}

fn is_number(c: u8) -> bool {
    return c >= b'0' && c <= b'9';
}

fn is_ident(c: u8) -> bool {
    return is_letter(c) || is_number(c) || c == b'_';
}

fn is_whitespace(c: u8) -> bool {
    return c == b' ' || c == b'\t' || c == b'\r' || c == b'\n';
}

enum Lexeme {
    Char,
    String,
    Ident,
    Number,
    Symbol,
}

impl Lexeme {
    fn from_char(c: u8) -> Self {
        match c {
            b'\'' => Lexeme::Char,
            b'\"' => Lexeme::String,
            _ => {
                if is_letter(c) || c == b'_' {
                    return Lexeme::Ident;
                } else if is_number(c) {
                    return Lexeme::Number;
                } else {
                    return Lexeme::Symbol;
                }
            }
        }
    }
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a [u8]) -> Self {
        Self {
            source,
            cursor: 0,
            line_id: 1,
            line_cursor: 0,
        }
    }

    pub fn tokenize(&mut self, tokens: &mut [Token<'a>; TOKEN_BUFFER_SIZE]) {
        let copy_count: usize = if self.cursor == 0 { 0 } else { TOKEN_LOOKAHEAD };

        for i in 0..copy_count {
            tokens[i] = tokens[TOKEN_BUFFER_SIZE - TOKEN_LOOKAHEAD + i];
        }

        for i in copy_count..TOKEN_BUFFER_SIZE {
            tokens[i] = self.get_token();
        }
    }

    fn get_token(&mut self) -> Token<'a> {
        self.skip_whitespace_and_comments();

        match self.peek() {
            Some(c) => match Lexeme::from_char(c) {
                Lexeme::Char => self.get_char_token(c),
                Lexeme::String => self.get_string_token(c),
                Lexeme::Ident => self.get_ident_token(c),
                Lexeme::Number => self.get_number_token(c),
                Lexeme::Symbol => self.get_symbol_token(c),
            },
            None => Token::default(),
        }
    }

    fn skip_whitespace_and_comments(&mut self) {}

    fn get_char_token(&mut self, c: u8) -> Token<'a> {
        let mut token = Token::default();
        token.span.start = self.cursor;
        self.consume();

        match c {
            b'\\' => {
                match self.peek() {
                    Some(c) => {
                        match c {
                            //todo escape sequences
                            _ => {}
                        }
                    }
                    None => {
                        //err invalid escape character
                    }
                }
            }
            _ => {
                token.data = TokenData::Char(c);
            }
        }

        match self.peek() {
            Some(c) => {
                if c != b'\'' {
                    //err expected closing '
                }
            }
            None => {
                //err expected closing '
            }
        }

        token.span.end = self.cursor;
        return token;
    }

    fn get_string_token(&mut self, c: u8) -> Token<'a> {
        let mut token = Token::default();
        token.span.start = self.cursor;
        self.consume();

        token.span.end = self.cursor;
        return token;
    }

    fn get_ident_token(&mut self, c: u8) -> Token<'a> {
        let mut token = Token::default();
        token.span.start = self.cursor;
        self.consume();

        loop {
            match self.peek() {
                Some(c) => {
                    if !is_ident(c) {
                        break;
                    }
                    self.consume();
                }
                None => {
                    break;
                }
            }
        }

        //look for keywords / types / true-false

        token.span.end = self.cursor;
        return token;
    }

    fn get_number_token(&mut self, c: u8) -> Token<'a> {
        let mut token = Token::default();
        token.span.start = self.cursor;
        self.consume();
        token.span.end = self.cursor;
        return token;
    }

    fn get_symbol_token(&mut self, c: u8) -> Token<'a> {
        let mut token = Token::default();
        token.span.start = self.cursor;

        self.consume();
        let sym_first = match c {
            b'.' => TokenType::Dot,
            b':' => TokenType::Colon,
            b',' => TokenType::Comma,
            b';' => TokenType::Semicolon,
            b'{' => TokenType::BlockStart,
            b'}' => TokenType::BlockEnd,
            b'[' => TokenType::BracketStart,
            b']' => TokenType::BracketEnd,
            b'(' => TokenType::ParenStart,
            b')' => TokenType::ParenEnd,
            b'@' => TokenType::At,
            b'#' => TokenType::Hash,
            b'?' => TokenType::Question,
            b'=' => TokenType::Assign,
            b'+' => TokenType::Plus,
            b'-' => TokenType::Minus,
            b'*' => TokenType::Times,
            b'/' => TokenType::Div,
            b'%' => TokenType::Mod,
            b'&' => TokenType::BitwiseAnd,
            b'|' => TokenType::BitwiseOr,
            b'^' => TokenType::BitwiseXor,
            b'<' => TokenType::Less,
            b'>' => TokenType::Greater,
            b'!' => TokenType::LogicNot,
            b'~' => TokenType::BitwiseNot,
            _ => TokenType::Error,
        };

        let c2_next = self.peek();
        if c2_next.is_none() || sym_first == TokenType::Error {
            token.token_type = sym_first;
            token.span.end = self.cursor;
            return token;
        }
        let c2 = c2_next.unwrap();

        let sym_second = match c2 {
            b'=' => match sym_first {
                TokenType::Assign => TokenType::IsEquals,
                TokenType::Plus => TokenType::PlusEquals,
                TokenType::Minus => TokenType::MinusEquals,
                TokenType::Times => TokenType::TimesEquals,
                TokenType::Div => TokenType::DivEquals,
                TokenType::Mod => TokenType::ModEquals,
                TokenType::BitwiseAnd => TokenType::BitwiseAndEquals,
                TokenType::BitwiseOr => TokenType::BitwiseOrEquals,
                TokenType::BitwiseXor => TokenType::BitwiseXorEquals,
                TokenType::Less => TokenType::LessEquals,
                TokenType::Greater => TokenType::GreaterEquals,
                TokenType::LogicNot => TokenType::NotEquals,
                _ => sym_first,
            },
            _ => {
                if c == c2 {
                    match sym_first {
                        TokenType::Dot => TokenType::DotDot,
                        TokenType::Colon => TokenType::ColonColon,
                        TokenType::BitwiseAnd => TokenType::LogicAnd,
                        TokenType::BitwiseOr => TokenType::LogicOr,
                        TokenType::Less => TokenType::BitshiftLeft,
                        TokenType::Greater => TokenType::BitshiftRight,
                        _ => sym_first,
                    };
                }
                sym_first
            }
        };
        if sym_first != sym_second {
            self.consume();
        }

        let c3_next = self.peek();
        if c3_next.is_none() || sym_second == sym_first {
            token.token_type = sym_second;
            token.span.end = self.cursor;
            return token;
        }
        let c3 = c3_next.unwrap();

        let sym_third = match sym_second {
            TokenType::BitshiftLeft => match c3 {
                b'=' => TokenType::BitshiftLeftEquals,
                _ => sym_second,
            },
            TokenType::BitshiftRight => match c3 {
                b'=' => TokenType::BitshiftRightEquals,
                _ => sym_second,
            },
            _ => sym_second,
        };
        if sym_third != sym_second {
            self.consume();
        }

        token.token_type = sym_third;
        token.span.end = self.cursor;
        return token;
    }

    fn peek(&mut self) -> Option<u8> {
        self.source.get(self.cursor as usize).map(|&byte| byte)
    }

    fn consume(&mut self) {
        self.cursor += 1;
    }
}
