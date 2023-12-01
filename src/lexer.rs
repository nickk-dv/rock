use super::token::*;

struct Lexer {}

impl Lexer {
    fn symbol_to_token_glue(c: char) -> Option<TokenKind> {
        match c {
            '(' => Some(TokenKind::Delim(Delim::OpenParen)),
            '{' => Some(TokenKind::Delim(Delim::OpenBlock)),
            '[' => Some(TokenKind::Delim(Delim::OpenBracket)),
            ')' => Some(TokenKind::Delim(Delim::CloseParen)),
            ']' => Some(TokenKind::Delim(Delim::CloseBlock)),
            '}' => Some(TokenKind::Delim(Delim::CloseBracket)),
            '@' => Some(TokenKind::Symbol(Symbol::At)),
            '.' => Some(TokenKind::Symbol(Symbol::Dot)),
            ':' => Some(TokenKind::Symbol(Symbol::Colon)),
            ',' => Some(TokenKind::Symbol(Symbol::Comma)),
            ';' => Some(TokenKind::Symbol(Symbol::Semicolon)),
            '!' => Some(TokenKind::Symbol(Symbol::LogicNot)),
            '~' => Some(TokenKind::Symbol(Symbol::BitwiseNot)),
            '<' => Some(TokenKind::Symbol(Symbol::Less)),
            '>' => Some(TokenKind::Symbol(Symbol::Greater)),
            '+' => Some(TokenKind::Symbol(Symbol::Plus)),
            '-' => Some(TokenKind::Symbol(Symbol::Minus)),
            '*' => Some(TokenKind::Symbol(Symbol::Times)),
            '/' => Some(TokenKind::Symbol(Symbol::Div)),
            '%' => Some(TokenKind::Symbol(Symbol::Mod)),
            '&' => Some(TokenKind::Symbol(Symbol::BitAnd)),
            '|' => Some(TokenKind::Symbol(Symbol::BitOr)),
            '^' => Some(TokenKind::Symbol(Symbol::BitXor)),
            '=' => Some(TokenKind::Symbol(Symbol::Assign)),
            _ => None,
        }
    }

    fn symbol_token_glue_2(c: char, kind: TokenKind) -> Option<TokenKind> {
        match c {
            '.' => match kind {
                TokenKind::Symbol(Symbol::Dot) => Some(TokenKind::Symbol(Symbol::DotDot)),
                _ => None,
            },
            ':' => match kind {
                TokenKind::Symbol(Symbol::Colon) => Some(TokenKind::Symbol(Symbol::ColonColon)),
                _ => None,
            },
            '&' => match kind {
                TokenKind::Symbol(Symbol::BitAnd) => Some(TokenKind::Symbol(Symbol::LogicAnd)),
                _ => None,
            },
            '|' => match kind {
                TokenKind::Symbol(Symbol::BitOr) => Some(TokenKind::Symbol(Symbol::LogicOr)),
                _ => None,
            },
            '<' => match kind {
                TokenKind::Symbol(Symbol::Less) => Some(TokenKind::Symbol(Symbol::Shl)),
                _ => None,
            },
            '>' => match kind {
                TokenKind::Symbol(Symbol::Minus) => Some(TokenKind::Symbol(Symbol::ArrowThin)),
                TokenKind::Symbol(Symbol::Assign) => Some(TokenKind::Symbol(Symbol::ArrowWide)),
                TokenKind::Symbol(Symbol::Greater) => Some(TokenKind::Symbol(Symbol::Shr)),
                _ => None,
            },
            '=' => match kind {
                TokenKind::Symbol(Symbol::Less) => Some(TokenKind::Symbol(Symbol::LessEq)),
                TokenKind::Symbol(Symbol::Greater) => Some(TokenKind::Symbol(Symbol::GreaterEq)),
                TokenKind::Symbol(Symbol::LogicNot) => Some(TokenKind::Symbol(Symbol::NotEq)),
                TokenKind::Symbol(Symbol::Assign) => Some(TokenKind::Symbol(Symbol::IsEq)),
                TokenKind::Symbol(Symbol::Plus) => Some(TokenKind::Symbol(Symbol::PlusEq)),
                TokenKind::Symbol(Symbol::Minus) => Some(TokenKind::Symbol(Symbol::MinusEq)),
                TokenKind::Symbol(Symbol::Times) => Some(TokenKind::Symbol(Symbol::TimesEq)),
                TokenKind::Symbol(Symbol::Div) => Some(TokenKind::Symbol(Symbol::DivEq)),
                TokenKind::Symbol(Symbol::Mod) => Some(TokenKind::Symbol(Symbol::ModEq)),
                TokenKind::Symbol(Symbol::BitAnd) => Some(TokenKind::Symbol(Symbol::BitAndEq)),
                TokenKind::Symbol(Symbol::BitOr) => Some(TokenKind::Symbol(Symbol::BitOrEq)),
                TokenKind::Symbol(Symbol::BitXor) => Some(TokenKind::Symbol(Symbol::BitXorEq)),
                _ => None,
            },
            _ => None,
        }
    }

    fn symbol_token_glue_3(c: char, kind: TokenKind) -> Option<TokenKind> {
        match c {
            '=' => match kind {
                TokenKind::Symbol(Symbol::Shl) => Some(TokenKind::Symbol(Symbol::ShlEq)),
                TokenKind::Symbol(Symbol::Shr) => Some(TokenKind::Symbol(Symbol::ShrEq)),
                _ => None,
            },
            _ => None,
        }
    }
}
