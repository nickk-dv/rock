pub struct Span {
    pub start: u32,
    pub end: u32,
}

pub struct Token {
    pub span: Span,
    pub kind: TokenKind,
}

#[derive(PartialEq)]
pub enum TokenKind {
    Ident,
    Delim(Delim),
    Symbol(Symbol),
    Keyword(Keyword),
    Literal(Literal),
    EndOfFile,
}

#[derive(PartialEq)]
pub enum Delim {
    OpenParen,
    OpenBlock,
    OpenBracket,
    CloseParen,
    CloseBlock,
    CloseBracket,
}

#[derive(PartialEq)]
pub enum Symbol {
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
    BitwiseNot,
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

#[derive(PartialEq)]
pub enum Keyword {
    // Decl
    Pub,
    Mod,
    Mut,
    Self_,
    Impl,
    Enum,
    Struct,
    Import,
    // Stmt
    If,
    Else,
    For,
    Defer,
    Break,
    Return,
    Switch,
    Continue,
    // Expr
    Cast,
    Sizeof,
    True,
    False,
    // Type
    S8,
    S16,
    S32,
    S64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Bool,
    String,
}

#[derive(PartialEq)]
pub enum Literal {
    Int(u64),
    Float(f64),
    Bool(bool),
}

impl TokenKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenKind::Ident => "ident",
            TokenKind::Delim(delim) => delim.as_str(),
            TokenKind::Symbol(symbol) => symbol.as_str(),
            TokenKind::Keyword(keyword) => keyword.as_str(),
            TokenKind::Literal(literal) => literal.as_str(),
            TokenKind::EndOfFile => "end of file",
        }
    }
}

impl Delim {
    fn as_str(&self) -> &'static str {
        match self {
            Delim::OpenParen => "(",
            Delim::OpenBlock => "{",
            Delim::OpenBracket => "[",
            Delim::CloseParen => ")",
            Delim::CloseBlock => "}",
            Delim::CloseBracket => "]",
        }
    }
}

impl Symbol {
    fn as_str(&self) -> &'static str {
        match self {
            Symbol::At => "@",
            Symbol::Dot => ".",
            Symbol::Colon => ":",
            Symbol::Comma => ",",
            Symbol::Semicolon => ";",
            Symbol::DotDot => "..",
            Symbol::ColonColon => "::",
            Symbol::ArrowThin => "->",
            Symbol::ArrowWide => "=>",

            Symbol::LogicNot => "!",
            Symbol::BitwiseNot => "~",

            Symbol::LogicAnd => "&&",
            Symbol::LogicOr => "||",
            Symbol::Less => "<",
            Symbol::Greater => ">",
            Symbol::LessEq => "<=",
            Symbol::GreaterEq => ">=",
            Symbol::IsEq => "==",
            Symbol::NotEq => "!=",

            Symbol::Plus => "+",
            Symbol::Minus => "-",
            Symbol::Times => "*",
            Symbol::Div => "/",
            Symbol::Mod => "%",
            Symbol::BitAnd => "&",
            Symbol::BitOr => "|",
            Symbol::BitXor => "^",
            Symbol::Shl => "<<",
            Symbol::Shr => ">>",

            Symbol::Assign => "=",
            Symbol::PlusEq => "+=",
            Symbol::MinusEq => "-=",
            Symbol::TimesEq => "*=",
            Symbol::DivEq => "/=",
            Symbol::ModEq => "%=",
            Symbol::BitAndEq => "&=",
            Symbol::BitOrEq => "|=",
            Symbol::BitXorEq => "^=",
            Symbol::ShlEq => "<<=",
            Symbol::ShrEq => ">>=",
        }
    }
}

impl Keyword {
    fn as_str(&self) -> &'static str {
        match self {
            Keyword::Pub => "pub",
            Keyword::Mod => "mod",
            Keyword::Mut => "mut",
            Keyword::Self_ => "self",
            Keyword::Impl => "impl",
            Keyword::Enum => "enum",
            Keyword::Struct => "struct",
            Keyword::Import => "import",

            Keyword::If => "if",
            Keyword::Else => "else",
            Keyword::For => "for",
            Keyword::Defer => "defer",
            Keyword::Break => "break",
            Keyword::Return => "return",
            Keyword::Switch => "switch",
            Keyword::Continue => "continue",

            Keyword::Cast => "cast",
            Keyword::Sizeof => "sizeof",
            Keyword::True => "true",
            Keyword::False => "false",

            Keyword::S8 => "s8",
            Keyword::S16 => "s16",
            Keyword::S32 => "s32",
            Keyword::S64 => "s64",
            Keyword::U8 => "u8",
            Keyword::U16 => "u16",
            Keyword::U32 => "u32",
            Keyword::U64 => "u64",
            Keyword::F32 => "f32",
            Keyword::F64 => "f64",
            Keyword::Bool => "bool",
            Keyword::String => "string",
        }
    }
}

impl Literal {
    fn as_str(&self) -> &'static str {
        match self {
            Literal::Int(_) => "int literal",
            Literal::Float(_) => "float literal",
            Literal::Bool(_) => "bool literal",
        }
    }
}
