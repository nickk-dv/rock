#[derive(Copy, Clone, Debug)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct Token {
    pub span: Span,
    pub kind: TokenKind,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TokenKind {
    Ident,
    // Literals
    LitNull,
    LitBool(bool),
    LitInt(u64),
    LitFloat(f64),
    LitString,
    // Special
    Error,
    Eof,
    // Keywords
    KwPub,
    KwMod,
    KwMut,
    KwSelf,
    KwImpl,
    KwEnum,
    KwStruct,
    KwImport,
    // Stmt
    KwIf,
    KwElse,
    KwFor,
    KwDefer,
    KwBreak,
    KwReturn,
    KwContinue,
    // Expr
    KwCast,
    KwSizeof,
    // Type
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
    // Delimeter
    OpenParen,
    OpenBlock,
    OpenBracket,
    CloseParen,
    CloseBlock,
    CloseBracket,
    // Separator
    At,
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

impl Token {
    pub fn new(start: u32, end: u32, kind: TokenKind) -> Self {
        Self {
            span: Span { start, end },
            kind,
        }
    }

    pub fn eof() -> Self {
        Self::new(0, 0, TokenKind::Eof)
    }
}

impl TokenKind {
    pub fn as_str(kind: TokenKind) -> &'static str {
        match kind {
            TokenKind::Ident => "ident",
            TokenKind::LitNull => "null",
            TokenKind::LitBool(..) => "bool literal",
            TokenKind::LitInt(..) => "integer literal",
            TokenKind::LitFloat(..) => "float literal",
            TokenKind::LitString => "string literal",
            TokenKind::Error => "error token",
            TokenKind::Eof => "end of file token",
            TokenKind::KwPub => "pub",
            TokenKind::KwMod => "mod",
            TokenKind::KwMut => "mut",
            TokenKind::KwSelf => "self",
            TokenKind::KwImpl => "impl",
            TokenKind::KwEnum => "enum",
            TokenKind::KwStruct => "struct",
            TokenKind::KwImport => "import",
            TokenKind::KwIf => "if",
            TokenKind::KwElse => "else",
            TokenKind::KwFor => "for",
            TokenKind::KwDefer => "defer",
            TokenKind::KwBreak => "break",
            TokenKind::KwReturn => "return",
            TokenKind::KwContinue => "continue",
            TokenKind::KwCast => "cast",
            TokenKind::KwSizeof => "sizeof",
            TokenKind::KwBool => "bool",
            TokenKind::KwS8 => "s8",
            TokenKind::KwS16 => "s16",
            TokenKind::KwS32 => "s32",
            TokenKind::KwS64 => "s64",
            TokenKind::KwSsize => "ssize",
            TokenKind::KwU8 => "u8",
            TokenKind::KwU16 => "u16",
            TokenKind::KwU32 => "u32",
            TokenKind::KwU64 => "u64",
            TokenKind::KwUsize => "usize",
            TokenKind::KwF32 => "f32",
            TokenKind::KwF64 => "f64",
            TokenKind::KwChar => "char",
            TokenKind::OpenParen => "(",
            TokenKind::OpenBlock => "{",
            TokenKind::OpenBracket => "[",
            TokenKind::CloseParen => ")",
            TokenKind::CloseBlock => "}",
            TokenKind::CloseBracket => "]",
            TokenKind::At => "@",
            TokenKind::Dot => ".",
            TokenKind::Colon => ":",
            TokenKind::Comma => ",",
            TokenKind::Semicolon => ";",
            TokenKind::DotDot => "..",
            TokenKind::ColonColon => "::",
            TokenKind::ArrowThin => "->",
            TokenKind::ArrowWide => "=>",
            TokenKind::LogicNot => "!",
            TokenKind::BitNot => "~",
            TokenKind::LogicAnd => "&&",
            TokenKind::LogicOr => "||",
            TokenKind::Less => "<",
            TokenKind::Greater => ">",
            TokenKind::LessEq => "<=",
            TokenKind::GreaterEq => ">=",
            TokenKind::IsEq => "==",
            TokenKind::NotEq => "!=",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Times => "*",
            TokenKind::Div => "/",
            TokenKind::Mod => "%",
            TokenKind::BitAnd => "&",
            TokenKind::BitOr => "|",
            TokenKind::BitXor => "^",
            TokenKind::Shl => "<<",
            TokenKind::Shr => ">>",
            TokenKind::Assign => "=",
            TokenKind::PlusEq => "+=",
            TokenKind::MinusEq => "-=",
            TokenKind::TimesEq => "*=",
            TokenKind::DivEq => "/=",
            TokenKind::ModEq => "%=",
            TokenKind::BitAndEq => "&=",
            TokenKind::BitOrEq => "|=",
            TokenKind::BitXorEq => "^=",
            TokenKind::ShlEq => "<<=",
            TokenKind::ShrEq => ">>=",
        }
    }

    pub fn keyword_from_str(str: &str) -> Option<TokenKind> {
        match str {
            "null" => Some(TokenKind::LitNull),
            "true" => Some(TokenKind::LitBool(true)),
            "false" => Some(TokenKind::LitBool(false)),
            "pub" => Some(TokenKind::KwPub),
            "mod" => Some(TokenKind::KwMod),
            "mut" => Some(TokenKind::KwMut),
            "self" => Some(TokenKind::KwSelf),
            "impl" => Some(TokenKind::KwImpl),
            "enum" => Some(TokenKind::KwEnum),
            "struct" => Some(TokenKind::KwStruct),
            "import" => Some(TokenKind::KwImport),
            "if" => Some(TokenKind::KwIf),
            "else" => Some(TokenKind::KwElse),
            "for" => Some(TokenKind::KwFor),
            "defer" => Some(TokenKind::KwDefer),
            "break" => Some(TokenKind::KwBreak),
            "return" => Some(TokenKind::KwReturn),
            "continue" => Some(TokenKind::KwContinue),
            "cast" => Some(TokenKind::KwCast),
            "sizeof" => Some(TokenKind::KwSizeof),
            "bool" => Some(TokenKind::KwBool),
            "s8" => Some(TokenKind::KwS8),
            "s16" => Some(TokenKind::KwS16),
            "s32" => Some(TokenKind::KwS32),
            "s64" => Some(TokenKind::KwS64),
            "ssize" => Some(TokenKind::KwSsize),
            "u8" => Some(TokenKind::KwU8),
            "u16" => Some(TokenKind::KwU16),
            "u32" => Some(TokenKind::KwU32),
            "u64" => Some(TokenKind::KwU64),
            "usize" => Some(TokenKind::KwUsize),
            "f32" => Some(TokenKind::KwF32),
            "f64" => Some(TokenKind::KwF64),
            "char" => Some(TokenKind::KwChar),
            _ => None,
        }
    }

    pub fn glue(c: char) -> Option<TokenKind> {
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

    pub fn glue2(c: char, kind: TokenKind) -> Option<TokenKind> {
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

    pub fn glue3(c: char, kind: TokenKind) -> Option<TokenKind> {
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
