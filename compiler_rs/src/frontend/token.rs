#[derive(Default, Copy, Clone)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Copy, Clone)]
pub struct Token {
    pub span: Span,
    pub token_type: TokenType,
    pub data: TokenData,
}

#[derive(Copy, Clone)]
pub enum TokenData {
    Char(u8),
    Bool(bool),
    Float(f64),
    Integer(u64),
}

impl Default for Token {
    fn default() -> Self {
        Self {
            span: Span::default(),
            token_type: TokenType::Error,
            data: TokenData::Bool(false),
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum TokenType {
    Ident,
    LiteralBool,
    LiteralFloat,
    LiteralInteger,
    LiteralString,

    KeywordStruct,
    KeywordEnum,
    KeywordIf,
    KeywordElse,
    KeywordTrue,
    KeywordFalse,
    KeywordFor,
    KeywordDefer,
    KeywordBreak,
    KeywordReturn,
    KeywordSwitch,
    KeywordContinue,
    KeywordSizeof,
    KeywordImport,
    KeywordUse,

    TypeI8,
    TypeU8,
    TypeI16,
    TypeU16,
    TypeI32,
    TypeU32,
    TypeI64,
    TypeU64,
    TypeF32,
    TypeF64,
    TypeBool,
    TypeString,

    Dot,
    Colon,
    Comma,
    Semicolon,
    DotDot,
    ColonColon,
    BlockStart,
    BlockEnd,
    BracketStart,
    BracketEnd,
    ParenStart,
    ParenEnd,
    At,
    Hash,
    Question,

    Assign,
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Less,
    Greater,
    LogicNot,
    IsEquals,
    PlusEquals,
    MinusEquals,
    TimesEquals,
    DivEquals,
    ModEquals,
    BitwiseAndEquals,
    BitwiseOrEquals,
    BitwiseXorEquals,
    LessEquals,
    GreaterEquals,
    NotEquals,
    LogicAnd,
    LogicOr,
    BitwiseNot,
    BitshiftLeft,
    BitshiftRight,
    BitshiftLeftEquals,
    BitshiftRightEquals,

    InputEnd,
    Error,
}

pub enum UnaryOp {
    Minus,
    LogicNot,
    BitwiseNot,
    AddressOf,
    Deference,
}

pub enum BinaryOp {
    LogicAnd,
    LogicOr,
    Less,
    Greater,
    LessEquals,
    GreaterEquals,
    IsEquals,
    NotEquals,
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitshiftLeft,
    BitshiftRight,
}

pub enum AssignOp {
    Assign,
    Plus,
    Minus,
    Times,
    Div,
    Mod,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitshiftLeft,
    BitshiftRight,
}

pub enum BasicType {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
    Bool,
    String,
}

impl TokenType {
    pub fn to_unary_op(&self) -> Option<UnaryOp> {
        match self {
            TokenType::Minus => Some(UnaryOp::Minus),
            TokenType::LogicNot => Some(UnaryOp::LogicNot),
            TokenType::BitwiseNot => Some(UnaryOp::BitwiseNot),
            TokenType::Times => Some(UnaryOp::AddressOf),
            TokenType::BitshiftLeft => Some(UnaryOp::Deference),
            _ => None,
        }
    }

    pub fn to_binary_op(&self) -> Option<BinaryOp> {
        match self {
            TokenType::LogicAnd => Some(BinaryOp::LogicAnd),
            TokenType::LogicOr => Some(BinaryOp::LogicOr),
            TokenType::Less => Some(BinaryOp::Less),
            TokenType::Greater => Some(BinaryOp::Greater),
            TokenType::LessEquals => Some(BinaryOp::LessEquals),
            TokenType::GreaterEquals => Some(BinaryOp::GreaterEquals),
            TokenType::IsEquals => Some(BinaryOp::IsEquals),
            TokenType::NotEquals => Some(BinaryOp::NotEquals),
            TokenType::Plus => Some(BinaryOp::Plus),
            TokenType::Minus => Some(BinaryOp::Minus),
            TokenType::Times => Some(BinaryOp::Times),
            TokenType::Div => Some(BinaryOp::Div),
            TokenType::Mod => Some(BinaryOp::Mod),
            TokenType::BitwiseAnd => Some(BinaryOp::BitwiseAnd),
            TokenType::BitwiseOr => Some(BinaryOp::BitwiseOr),
            TokenType::BitwiseXor => Some(BinaryOp::BitwiseXor),
            TokenType::BitshiftLeft => Some(BinaryOp::BitshiftLeft),
            TokenType::BitshiftRight => Some(BinaryOp::BitshiftRight),
            _ => None,
        }
    }

    pub fn to_assign_op(&self) -> Option<AssignOp> {
        match self {
            TokenType::Assign => Some(AssignOp::Assign),
            TokenType::PlusEquals => Some(AssignOp::Plus),
            TokenType::MinusEquals => Some(AssignOp::Minus),
            TokenType::TimesEquals => Some(AssignOp::Times),
            TokenType::DivEquals => Some(AssignOp::Div),
            TokenType::ModEquals => Some(AssignOp::Mod),
            TokenType::BitwiseAndEquals => Some(AssignOp::BitwiseAnd),
            TokenType::BitwiseOrEquals => Some(AssignOp::BitwiseOr),
            TokenType::BitwiseXorEquals => Some(AssignOp::BitwiseXor),
            TokenType::BitshiftLeftEquals => Some(AssignOp::BitshiftLeft),
            TokenType::BitshiftRightEquals => Some(AssignOp::BitshiftRight),
            _ => None,
        }
    }

    pub fn to_basic_type(&self) -> Option<BasicType> {
        match self {
            TokenType::TypeI8 => Some(BasicType::I8),
            TokenType::TypeU8 => Some(BasicType::U8),
            TokenType::TypeI16 => Some(BasicType::I16),
            TokenType::TypeU16 => Some(BasicType::U16),
            TokenType::TypeI32 => Some(BasicType::I32),
            TokenType::TypeU32 => Some(BasicType::U32),
            TokenType::TypeI64 => Some(BasicType::I64),
            TokenType::TypeU64 => Some(BasicType::U64),
            TokenType::TypeF32 => Some(BasicType::F32),
            TokenType::TypeF64 => Some(BasicType::F64),
            TokenType::TypeBool => Some(BasicType::Bool),
            TokenType::TypeString => Some(BasicType::String),
            _ => None,
        }
    }
}

impl BinaryOp {
    pub fn precedence(&self) -> u32 {
        match self {
            BinaryOp::LogicAnd | BinaryOp::LogicOr => 0,
            BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessEquals
            | BinaryOp::GreaterEquals
            | BinaryOp::IsEquals
            | BinaryOp::NotEquals => 1,
            BinaryOp::Plus | BinaryOp::Minus => 2,
            BinaryOp::Times | BinaryOp::Div | BinaryOp::Mod => 3,
            BinaryOp::BitwiseAnd | BinaryOp::BitwiseOr | BinaryOp::BitwiseXor => 4,
            BinaryOp::BitshiftLeft | BinaryOp::BitshiftRight => 5,
        }
    }
}
