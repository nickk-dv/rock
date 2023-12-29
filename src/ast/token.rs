use super::span::Span;

#[derive(Copy, Clone)]
pub struct TokenSpan {
    pub span: Span,
    pub token: Token,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Token {
    Ident,
    // Literal
    LitNull,
    LitBool(bool),
    LitInt(u64),
    LitFloat(f64),
    LitChar(char),
    LitString,
    // Special
    Error,
    Eof,
    // Keyword
    KwPub,
    KwMod,
    KwEnum,
    KwStruct,
    KwImport,
    KwSuper,
    KwPackage,
    KwIf,
    KwElse,
    KwFor,
    KwDefer,
    KwBreak,
    KwSwitch,
    KwReturn,
    KwContinue,
    KwCast,
    KwSizeof,
    KwBool,
    KwS8,
    KwS16,
    KwS32,
    KwS64,
    KwSsize,
    KwU8,
    KwU16,
    KwU32,
    KwU64,
    KwUsize,
    KwF32,
    KwF64,
    KwChar,
    KwRawptr,
    // Directives
    DirCCall,
    // Delimeter
    OpenParen,
    OpenBlock,
    OpenBracket,
    CloseParen,
    CloseBlock,
    CloseBracket,
    // Separator
    Dot,
    Colon,
    Comma,
    Semicolon,
    DotDot,
    ColonColon,
    ArrowThin,
    ArrowWide,
    /// Unary op
    LogicNot,
    BitNot,
    // Binary cmp
    LogicAnd,
    LogicOr,
    Less,
    Greater,
    LessEq,
    GreaterEq,
    IsEq,
    NotEq,
    // Binary op
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    // Assign op
    Assign,
    PlusEq,
    MinusEq,
    TimesEq,
    DivEq,
    ModEq,
    BitAndEq,
    BitOrEq,
    BitXorEq,
    ShlEq,
    ShrEq,
}

impl TokenSpan {
    pub fn new(span: Span, token: Token) -> Self {
        Self { span, token }
    }

    pub fn eof() -> Self {
        Self::new(Span { start: 0, end: 0 }, Token::Eof)
    }
}

impl Token {
    pub fn as_str(kind: Token) -> &'static str {
        match kind {
            Token::Ident => "identifier",
            Token::LitNull => "null",
            Token::LitBool(..) => "bool literal",
            Token::LitInt(..) => "integer literal",
            Token::LitFloat(..) => "float literal",
            Token::LitChar(..) => "char literal",
            Token::LitString => "string literal",
            Token::Error => "error token",
            Token::Eof => "end of file token",
            Token::KwPub => "pub",
            Token::KwMod => "mod",
            Token::KwEnum => "enum",
            Token::KwStruct => "struct",
            Token::KwImport => "import",
            Token::KwSuper => "super",
            Token::KwPackage => "package",
            Token::KwIf => "if",
            Token::KwElse => "else",
            Token::KwFor => "for",
            Token::KwDefer => "defer",
            Token::KwBreak => "break",
            Token::KwSwitch => "switch",
            Token::KwReturn => "return",
            Token::KwContinue => "continue",
            Token::KwCast => "cast",
            Token::KwSizeof => "sizeof",
            Token::KwBool => "bool",
            Token::KwS8 => "s8",
            Token::KwS16 => "s16",
            Token::KwS32 => "s32",
            Token::KwS64 => "s64",
            Token::KwSsize => "ssize",
            Token::KwU8 => "u8",
            Token::KwU16 => "u16",
            Token::KwU32 => "u32",
            Token::KwU64 => "u64",
            Token::KwUsize => "usize",
            Token::KwF32 => "f32",
            Token::KwF64 => "f64",
            Token::KwChar => "char",
            Token::KwRawptr => "rawptr",
            Token::DirCCall => "c_call",
            Token::OpenParen => "(",
            Token::OpenBlock => "{",
            Token::OpenBracket => "[",
            Token::CloseParen => ")",
            Token::CloseBlock => "}",
            Token::CloseBracket => "]",
            Token::Dot => ".",
            Token::Colon => ":",
            Token::Comma => ",",
            Token::Semicolon => ";",
            Token::DotDot => "..",
            Token::ColonColon => "::",
            Token::ArrowThin => "->",
            Token::ArrowWide => "=>",
            Token::LogicNot => "!",
            Token::BitNot => "~",
            Token::LogicAnd => "&&",
            Token::LogicOr => "||",
            Token::Less => "<",
            Token::Greater => ">",
            Token::LessEq => "<=",
            Token::GreaterEq => ">=",
            Token::IsEq => "==",
            Token::NotEq => "!=",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::Times => "*",
            Token::Div => "/",
            Token::Mod => "%",
            Token::BitAnd => "&",
            Token::BitOr => "|",
            Token::BitXor => "^",
            Token::Shl => "<<",
            Token::Shr => ">>",
            Token::Assign => "=",
            Token::PlusEq => "+=",
            Token::MinusEq => "-=",
            Token::TimesEq => "*=",
            Token::DivEq => "/=",
            Token::ModEq => "%=",
            Token::BitAndEq => "&=",
            Token::BitOrEq => "|=",
            Token::BitXorEq => "^=",
            Token::ShlEq => "<<=",
            Token::ShrEq => ">>=",
        }
    }

    pub fn keyword_from_str(str: &str) -> Option<Token> {
        match str {
            "null" => Some(Token::LitNull),
            "true" => Some(Token::LitBool(true)),
            "false" => Some(Token::LitBool(false)),
            "pub" => Some(Token::KwPub),
            "mod" => Some(Token::KwMod),
            "enum" => Some(Token::KwEnum),
            "struct" => Some(Token::KwStruct),
            "import" => Some(Token::KwImport),
            "super" => Some(Token::KwSuper),
            "package" => Some(Token::KwPackage),
            "if" => Some(Token::KwIf),
            "else" => Some(Token::KwElse),
            "for" => Some(Token::KwFor),
            "defer" => Some(Token::KwDefer),
            "break" => Some(Token::KwBreak),
            "switch" => Some(Token::KwSwitch),
            "return" => Some(Token::KwReturn),
            "continue" => Some(Token::KwContinue),
            "cast" => Some(Token::KwCast),
            "sizeof" => Some(Token::KwSizeof),
            "bool" => Some(Token::KwBool),
            "s8" => Some(Token::KwS8),
            "s16" => Some(Token::KwS16),
            "s32" => Some(Token::KwS32),
            "s64" => Some(Token::KwS64),
            "ssize" => Some(Token::KwSsize),
            "u8" => Some(Token::KwU8),
            "u16" => Some(Token::KwU16),
            "u32" => Some(Token::KwU32),
            "u64" => Some(Token::KwU64),
            "usize" => Some(Token::KwUsize),
            "f32" => Some(Token::KwF32),
            "f64" => Some(Token::KwF64),
            "char" => Some(Token::KwChar),
            "rawptr" => Some(Token::KwRawptr),
            "c_call" => Some(Token::DirCCall),
            _ => None,
        }
    }

    pub fn glue(c: char) -> Option<Token> {
        match c {
            '(' => Some(Token::OpenParen),
            ')' => Some(Token::CloseParen),
            '{' => Some(Token::OpenBlock),
            '}' => Some(Token::CloseBlock),
            '[' => Some(Token::OpenBracket),
            ']' => Some(Token::CloseBracket),
            '.' => Some(Token::Dot),
            ':' => Some(Token::Colon),
            ',' => Some(Token::Comma),
            ';' => Some(Token::Semicolon),
            '!' => Some(Token::LogicNot),
            '~' => Some(Token::BitNot),
            '<' => Some(Token::Less),
            '>' => Some(Token::Greater),
            '+' => Some(Token::Plus),
            '-' => Some(Token::Minus),
            '*' => Some(Token::Times),
            '/' => Some(Token::Div),
            '%' => Some(Token::Mod),
            '&' => Some(Token::BitAnd),
            '|' => Some(Token::BitOr),
            '^' => Some(Token::BitXor),
            '=' => Some(Token::Assign),
            _ => None,
        }
    }

    pub fn glue2(c: char, kind: Token) -> Option<Token> {
        match c {
            '.' => match kind {
                Token::Dot => Some(Token::DotDot),
                _ => None,
            },
            ':' => match kind {
                Token::Colon => Some(Token::ColonColon),
                _ => None,
            },
            '&' => match kind {
                Token::BitAnd => Some(Token::LogicAnd),
                _ => None,
            },
            '|' => match kind {
                Token::BitOr => Some(Token::LogicOr),
                _ => None,
            },
            '<' => match kind {
                Token::Less => Some(Token::Shl),
                _ => None,
            },
            '>' => match kind {
                Token::Minus => Some(Token::ArrowThin),
                Token::Assign => Some(Token::ArrowWide),
                Token::Greater => Some(Token::Shr),
                _ => None,
            },
            '=' => match kind {
                Token::Less => Some(Token::LessEq),
                Token::Greater => Some(Token::GreaterEq),
                Token::LogicNot => Some(Token::NotEq),
                Token::Assign => Some(Token::IsEq),
                Token::Plus => Some(Token::PlusEq),
                Token::Minus => Some(Token::MinusEq),
                Token::Times => Some(Token::TimesEq),
                Token::Div => Some(Token::DivEq),
                Token::Mod => Some(Token::ModEq),
                Token::BitAnd => Some(Token::BitAndEq),
                Token::BitOr => Some(Token::BitOrEq),
                Token::BitXor => Some(Token::BitXorEq),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn glue3(c: char, kind: Token) -> Option<Token> {
        match c {
            '=' => match kind {
                Token::Shl => Some(Token::ShlEq),
                Token::Shr => Some(Token::ShrEq),
                _ => None,
            },
            _ => None,
        }
    }
}
