use super::span::*;

pub struct TokenList {
    tokens: Vec<Token>,
    spans: Vec<Span>,
}

#[derive(Copy, Clone)]
pub enum Token {
    IntLit,   // 10
    FloatLit, // 1.0
    Ident,    // name

    KwBool,  // bool
    KwS8,    // s8
    KwS16,   // s16
    KwS32,   // s32
    KwS64,   // s64
    KwSsize, // ssize
    KwU8,    // u8
    KwU16,   // u16
    KwU32,   // u32
    KwU64,   // u64
    KwUsize, // usize
    KwF32,   // f32
    KwF64,   // f64
    KwChar,  // char

    KwPub,      // pub
    KwMod,      // mod
    KwImport,   // import
    KwSuper,    // super
    KwPackage,  // package
    KwEnum,     // enum
    KwUnion,    // union
    KwStruct,   // struct
    KwFor,      // for
    KwDefer,    // defer
    KwBreak,    // break
    KwContinue, // continue
    KwReturn,   // return
    KwAs,       // as
    KwIf,       // if
    KwElse,     // else
    KwMatch,    // match
    KwSizeof,   // sizeof

    Bang,         // !
    Quote2,       // "
    Hash,         // #
    Dollar,       // $
    Percent,      // %
    Ampersand,    // &
    Quote,        // '
    ParenOpen,    // (
    ParenClose,   // )
    Star,         // *
    Plus,         // -
    Comma,        // ,
    Minus,        // -
    Dot,          // .
    ForwSlash,    // /
    Colon,        // :
    Semicolon,    // ;
    Less,         // <
    Equals,       // =
    Greater,      // >
    Question,     // ?
    At,           // @
    BracketOpen,  // [
    BackSlash,    // \
    BracketClose, // ]
    Caret,        // ^
    Underscore,   // _
    Backtick,     // `
    CurlyOpen,    // {
    Pipe,         // |
    CurlyClose,   // }
    Tilde,        // ~

    BinShl,       // <<
    BinShr,       // <<
    BinIsEq,      // ==
    BinNotEq,     // !=
    BinLessEq,    // <=
    BinGreaterEq, // >=
    BinLogicAnd,  // &&
    BinLogicOr,   // ||

    AssignAdd,    // +=
    AssignSub,    // -=
    AssignMul,    // *=
    AssignDiv,    // /=
    AssignRem,    // %=
    AssignBitAnd, // &=
    AssignBitOr,  // |=
    AssignBitXor, // ^=
    AssignShl,    // <<=
    AssignShr,    // >>=
}

impl TokenList {
    pub fn new(cap: usize) -> Self {
        Self {
            tokens: Vec::with_capacity(cap),
            spans: Vec::with_capacity(cap),
        }
    }

    pub fn add(&mut self, token: Token, span: Span) {
        self.tokens.push(token);
        self.spans.push(span);
    }

    pub fn token(&self, index: usize) -> Token {
        unsafe { *self.tokens.get_unchecked(index) }
    }

    pub fn span(&self, index: usize) -> Span {
        unsafe { *self.spans.get_unchecked(index) }
    }
}

#[derive(Copy, Clone)]
pub enum UnOp {
    Neg,      // -
    BitNot,   // ~
    LogicNot, // !
    Addr,     // &
    Deref,    // *
}

#[derive(Copy, Clone)]
pub enum BinOp {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Rem, // %

    BitAnd, // &
    BitOr,  // |
    BitXor, // ^
    Shl,    // <<
    Shr,    // >>

    IsEq,      // ==
    NotEq,     // !=
    Less,      // <
    LessEq,    // <=
    Greater,   // >
    GreaterEq, // >=

    LogicAnd, // &&
    LogicOr,  // ||
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    BinAssign(BinOp),
}
