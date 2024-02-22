use super::ast::{AssignOp, BasicType, BinOp, Mut, UnOp};

macro_rules! token_impl {
    ($(
        $variant:ident as $string:expr
        $(=> KW $mark:tt)?
        $(=> UN $un:expr)?
        $(=> BIN $bin:expr)?
        $(=> ASSIGN $assign:expr)?
        $(=> BASIC_TYPE $basic_type:expr)?
    )+) => {
        #[derive(Copy, Clone, PartialEq)]
        pub enum Token {
            $($variant),+
        }
        impl Token {
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
            pub fn as_basic_type(&self) -> Option<BasicType> {
                match *self {
                    $(Token::$variant => token_impl!(@BASIC_TYPE_ARM $(=> BASIC_TYPE $basic_type)?), )+
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
    (@BASIC_TYPE_ARM => BASIC_TYPE $basic_type:expr) => { Some($basic_type) };
    (@BASIC_TYPE_ARM) => { None };
}

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
    Ident        as "identifier"
    IntLit       as "int literal"
    FloatLit     as "float literal"
    CharLit      as "char literal"
    StringLit    as "string literal"

    KwBool       as "bool"     => KW. => BASIC_TYPE BasicType::Bool
    KwS8         as "s8"       => KW. => BASIC_TYPE BasicType::S8
    KwS16        as "s16"      => KW. => BASIC_TYPE BasicType::S16
    KwS32        as "s32"      => KW. => BASIC_TYPE BasicType::S32
    KwS64        as "s64"      => KW. => BASIC_TYPE BasicType::S64
    KwSsize      as "ssize"    => KW. => BASIC_TYPE BasicType::Ssize
    KwU8         as "u8"       => KW. => BASIC_TYPE BasicType::U8
    KwU16        as "u16"      => KW. => BASIC_TYPE BasicType::U16
    KwU32        as "u32"      => KW. => BASIC_TYPE BasicType::U32
    KwU64        as "u64"      => KW. => BASIC_TYPE BasicType::U64
    KwUsize      as "usize"    => KW. => BASIC_TYPE BasicType::Usize
    KwF32        as "f32"      => KW. => BASIC_TYPE BasicType::F32
    KwF64        as "f64"      => KW. => BASIC_TYPE BasicType::F64
    KwChar       as "char"     => KW. => BASIC_TYPE BasicType::Char
    Rawptr       as "rawptr"   => KW. => BASIC_TYPE BasicType::Rawptr

    KwPub        as "pub"      => KW.
    KwMut        as "mut"      => KW.
    KwMod        as "mod"      => KW.
    KwUse        as "use"      => KW.
    KwSuper      as "super"    => KW.
    KwPackage    as "package"  => KW.
    KwProc       as "proc"     => KW.
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
    DirCCall     as "c_call"   => KW.

    Bang         as "!"  => UN UnOp::LogicNot
    Quote2       as "\""
    Hash         as "#"
    Dollar       as "$"
    Percent      as "%"  => BIN BinOp::Rem
    Ampersand    as "&"  => UN UnOp::Addr(Mut::Immutable) => BIN BinOp::BitAnd
    Quote        as "\'"
    OpenParen    as "("
    CloseParen   as ")"
    Star         as "*"  => UN UnOp::Deref => BIN BinOp::Mul
    Plus         as "+"  => BIN BinOp::Add
    Comma        as ","
    Minus        as "-"  => UN UnOp::Neg => BIN BinOp::Sub
    Dot          as "."
    ForwSlash    as "/"  => BIN BinOp::Div
    Colon        as ":"
    Semicolon    as ";"
    Less         as "<"  => BIN BinOp::CmpLt
    Equals       as "="  => ASSIGN AssignOp::Assign
    Greater      as ">"  => BIN BinOp::CmpGt
    Question     as "?"
    At           as "@"
    OpenBracket  as "["
    BackSlash    as "\\"
    CloseBracket as "]"
    Caret        as "^"  => BIN BinOp::BitXor
    Underscore   as "_"  => KW.
    Backtick     as "`"
    OpenBlock    as "{"
    Pipe         as "|"  => BIN BinOp::BitOr
    CloseBlock   as "}"
    Tilde        as "~"  => UN UnOp::BitNot

    DotDot       as ".."
    ColonColon   as "::"
    ArrowThin    as "->"
    ArrowWide    as "=>"

    BinShl       as "<<" => BIN BinOp::BitShl
    BinShr       as ">>" => BIN BinOp::BitShr
    BinIsEq      as "==" => BIN BinOp::CmpIsEq
    BinNotEq     as "!=" => BIN BinOp::CmpNotEq
    BinLessEq    as "<=" => BIN BinOp::CmpLtEq
    BinGreaterEq as ">=" => BIN BinOp::CmpGtEq
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
    AssignShl    as "<<=" => ASSIGN AssignOp::Bin(BinOp::BitShl)
    AssignShr    as ">>=" => ASSIGN AssignOp::Bin(BinOp::BitShr)
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
    OpenParen    as '('
    CloseParen   as ')'
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
    OpenBracket  as '['
    BackSlash    as '\\'
    CloseBracket as ']'
    Caret        as '^'
    Underscore   as '_'
    Backtick     as '`'
    OpenBlock    as '{'
    Pipe         as '|'
    CloseBlock   as '}'
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
    ('=')
    BinShl => AssignShl,
    BinShr => AssignShr,
}
