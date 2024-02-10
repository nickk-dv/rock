use super::ast2::{AssignOp, BinOp, UnOp};
use super::span::*;

pub struct TokenList {
    tokens: Vec<Token>,
    spans: Vec<Span>,
}

#[derive(Clone, Copy)]
pub struct TokenIndex(u32);

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
    pub fn len(&self) -> usize {
        self.tokens.len()
    }
    pub fn cap(&self) -> usize {
        self.tokens.capacity()
    }
}

impl TokenIndex {
    pub fn new(index: usize) -> Self {
        Self(index as u32)
    }
    pub fn index(&self) -> usize {
        self.0 as usize
    }
}

/// Token enum and conversions
macro_rules! token_impl {
    ($(
        $variant:ident as $string:expr
        $(=> KW $mark:tt)?
        $(=> UN $un:expr)?
        $(=> BIN $bin:expr)?
        $(=> ASSIGN $assign:expr)?
    )+) => {
        #[derive(Copy, Clone, PartialEq)]
        pub enum Token {
            $($variant),+
        }
        impl Token {
            pub fn to_str(token: Token) -> &'static str {
                token.as_str()
            }
            pub fn as_str(&self) -> &'static str {
                match *self {
                    $(Token::$variant => $string,)+
                }
            }
            pub fn as_keyword(source: &str) -> Option<Token> {
                match source {
                    $($string => token_impl!(@KW_ARM $variant $(=> KW $mark)?), )+
                    _ => None,
                }
            }
            pub fn as_un_op(&self) -> Option<UnOp> {
                match *self {
                    $(Token::$variant => token_impl!(@UN_ARM $(=> UN $un)?), )+
                }
            }
            pub fn as_bin_op(&self) -> Option<BinOp> {
                match *self {
                    $(Token::$variant => token_impl!(@BIN_ARM $(=> BIN $bin)?), )+
                }
            }
            pub fn as_assign_op(&self) -> Option<AssignOp> {
                match *self {
                    $(Token::$variant => token_impl!(@ASSIGN_ARM $(=> ASSIGN $assign)?), )+
                }
            }
        }
    };
    (@KW_ARM $variant:ident => KW $mark:tt) => { Some(Token::$variant) };
    (@KW_ARM $variant:ident) => { None };
    (@UN_ARM => UN $un:expr) => { Some($un) };
    (@UN_ARM) => { None };
    (@BIN_ARM => BIN $bin:expr) => { Some($bin) };
    (@BIN_ARM) => { None };
    (@ASSIGN_ARM => ASSIGN $assign:expr) => { Some($assign) };
    (@ASSIGN_ARM) => { None };
}

/// Token glue for creating 1 character tokens
macro_rules! token_glue {
    ($name:ident, $($to:ident as $ch:expr)+) => {
        impl Token {
            pub fn $name(c: char) -> Option<Token> {
                match c {
                    $($ch => Some(Token::$to),)+
                    _ => None,
                }
            }
        }
    };
}

/// Token glue for extending 1-2 character tokens
macro_rules! token_glue_extend {
    ($name:ident, $( ($ch:expr) $($from:ident => $to:ident,)+ )+ ) => {
        impl Token {
            pub fn $name(c: char, token: Token) -> Option<Token> {
                match c {
                    $(
                        $ch => match token {
                            $(Token::$from => Some(Token::$to),)+
                            _ => None,
                        },
                    )+
                    _ => None,
                }
            }
        }
    };
}

token_impl! {
    Eof          as "end of file"
    Error        as "error token"
    IntLit       as "int literal"
    FloatLit     as "float literal"
    Ident        as "identifier"

    KwBool       as "bool"     => KW.
    KwS8         as "s8"       => KW.
    KwS16        as "s16"      => KW.
    KwS32        as "s32"      => KW.
    KwS64        as "s64"      => KW.
    KwSsize      as "ssize"    => KW.
    KwU8         as "u8"       => KW.
    KwU16        as "u16"      => KW.
    KwU32        as "u32"      => KW.
    KwU64        as "u64"      => KW.
    KwUsize      as "usize"    => KW.
    KwF32        as "f32"      => KW.
    KwF64        as "f64"      => KW.
    KwChar       as "char"     => KW.

    KwPub        as "pub"      => KW.
    KwMut        as "mut"      => KW.
    KwMod        as "mod"      => KW.
    KwImport     as "import"   => KW.
    KwSuper      as "super"    => KW.
    KwPackage    as "package"  => KW.
    KwEnum       as "enum"     => KW.
    KwUnion      as "union"    => KW.
    KwStruct     as "struct"   => KW.

    KwFor        as "for"      => KW.
    KwDefer      as "defer"    => KW.
    KwBreak      as "break"    => KW.
    KwContinue   as "continue" => KW.
    KwReturn     as "return"   => KW.

    KwAs         as "as"       => KW.
    KwIf         as "if"       => KW.
    KwElse       as "else"     => KW.
    KwMatch      as "match"    => KW.
    KwSizeof     as "sizeof"   => KW.
    KwNull       as "null"     => KW.
    KwTrue       as "true"     => KW.
    KwFalse      as "false"    => KW.

    Bang         as "!"  => UN UnOp::LogicNot
    Quote2       as "\""
    Hash         as "#"
    Dollar       as "$"
    Percent      as "%"  => BIN BinOp::Rem
    Ampersand    as "&"  => UN UnOp::Addr => BIN BinOp::BitAnd
    Quote        as "\'"
    ParenOpen    as "("
    ParenClose   as ")"
    Star         as "*"  => UN UnOp::Deref => BIN BinOp::Mul
    Plus         as "+"  => BIN BinOp::Add
    Comma        as ","
    Minus        as "-"  => UN UnOp::Neg => BIN BinOp::Sub
    Dot          as "."
    ForwSlash    as "/"  => BIN BinOp::Div
    Colon        as ":"
    Semicolon    as ";"
    Less         as "<"  => BIN BinOp::Less
    Equals       as "="  => ASSIGN AssignOp::Assign
    Greater      as ">"  => BIN BinOp::Greater
    Question     as "?"
    At           as "@"
    BracketOpen  as "["
    BackSlash    as "/"
    BracketClose as "]"
    Caret        as "^"  => BIN BinOp::BitXor
    Underscore   as "_"  => KW.
    Backtick     as "`"
    CurlyOpen    as "{"
    Pipe         as "|"  => BIN BinOp::BitOr
    CurlyClose   as "}"
    Tilde        as "~"  => UN UnOp::BitNot

    DotDot       as ".."
    ColonColon   as "::"
    ArrowThin    as "->"
    ArrowWide    as "=>"

    BinShl       as "<<" => BIN BinOp::Shl
    BinShr       as ">>" => BIN BinOp::Shr
    BinIsEq      as "==" => BIN BinOp::IsEq
    BinNotEq     as "!=" => BIN BinOp::NotEq
    BinLessEq    as "<=" => BIN BinOp::LessEq
    BinGreaterEq as ">=" => BIN BinOp::GreaterEq
    BinLogicAnd  as "&&" => BIN BinOp::LogicAnd
    BinLogicOr   as "||" => BIN BinOp::LogicOr

    AssignAdd    as "+="  => ASSIGN AssignOp::Bin(BinOp::Add)
    AssignSub    as "-="  => ASSIGN AssignOp::Bin(BinOp::Sub)
    AssignMul    as "*="  => ASSIGN AssignOp::Bin(BinOp::Mul)
    AssignDiv    as "/="  => ASSIGN AssignOp::Bin(BinOp::Div)
    AssignRem    as "%="  => ASSIGN AssignOp::Bin(BinOp::Rem)
    AssignBitAnd as "&="  => ASSIGN AssignOp::Bin(BinOp::BitAnd)
    AssignBitOr  as "|="  => ASSIGN AssignOp::Bin(BinOp::BitOr)
    AssignBitXor as "^="  => ASSIGN AssignOp::Bin(BinOp::BitXor)
    AssignShl    as "<<=" => ASSIGN AssignOp::Bin(BinOp::Shl)
    AssignShr    as ">>=" => ASSIGN AssignOp::Bin(BinOp::Shr)
}

token_glue! {
    glue,
    Bang         as '!'
    Quote2       as '\"'
    Hash         as '#'
    Dollar       as '$'
    Percent      as '%'
    Ampersand    as '&'
    Quote        as '\''
    ParenOpen    as '('
    ParenClose   as ')'
    Star         as '*'
    Plus         as '+'
    Comma        as ','
    Minus        as '-'
    Dot          as '.'
    ForwSlash    as '/'
    Colon        as ':'
    Semicolon    as ';'
    Less         as '<'
    Equals       as '='
    Greater      as '>'
    Question     as '?'
    At           as '@'
    BracketOpen  as '['
    BackSlash    as '/'
    BracketClose as ']'
    Caret        as '^'
    Underscore   as '_'
    Backtick     as '`'
    CurlyOpen    as '{'
    Pipe         as '|'
    CurlyClose   as '}'
    Tilde        as '~'
}

token_glue_extend! {
    glue2,
    ('.') Dot => DotDot,
    (':') Colon => ColonColon,
    ('<') Less => BinShl,
    ('&') Ampersand => BinLogicAnd,
    ('|') Pipe => BinLogicOr,
    ('>')
    Minus => ArrowThin,
    Equals => ArrowWide,
    Greater => BinShr,
    ('=')
    Equals => BinIsEq,
    Bang => BinNotEq,
    Less => BinLessEq,
    Greater => BinGreaterEq,
    Plus => AssignAdd,
    Minus => AssignSub,
    Star => AssignMul,
    ForwSlash => AssignDiv,
    Percent => AssignRem,
    Ampersand => AssignBitAnd,
    Pipe => AssignBitOr,
    Caret => AssignBitXor,
}

token_glue_extend! {
    glue3,
    ('=') BinShl => AssignShl,
    ('=') BinShr => AssignShr,
}
