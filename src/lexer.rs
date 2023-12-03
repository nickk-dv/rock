use super::token::*;

struct TokenBuffer {
    pub tokens: [Token; Self::SIZE],
}

impl TokenBuffer {
    pub const SIZE: usize = 256;
    pub const LOOKAHEAD: usize = 4;
}

struct Lexer {}

impl Lexer {
    fn symbol_to_token_glue(c: char) -> Option<TokenKind> {
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

    fn symbol_token_glue_2(c: char, kind: TokenKind) -> Option<TokenKind> {
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

    fn symbol_token_glue_3(c: char, kind: TokenKind) -> Option<TokenKind> {
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
