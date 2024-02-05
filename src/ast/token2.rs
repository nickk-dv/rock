use super::span::*;

pub struct TokenList {
    pub tokens: Vec<Token>, //@make private
    pub spans: Vec<Span>,   //@make private
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

macro_rules! token_gen {
    ($($variant_name:ident as $string_lit:expr ; $($kw:ident)? ,)+) => {
        #[derive(Copy, Clone, PartialEq)]
        pub enum Token {
            $($variant_name),+
        }

        impl Token {
            pub fn to_str(token: Token) -> &'static str {
                token.as_str()
            }

            pub fn as_str(&self) -> &'static str {
                match *self {
                    $(Token::$variant_name => $string_lit,)+
                }
            }

            pub fn keyword_from_str(s: &str) -> Option<Token> {
                match s {
                    $($string_lit => token_kw_arm!($variant_name $($kw)?),)+
                    _ => None,
                }
            }
        }
    };
}

macro_rules! token_kw_arm {
    ($variant_name:ident $kw:ident) => {
        Some(Token::$variant_name)
    };
    ($variant_name:ident) => {
        None
    };
}

macro_rules! token_glue {
    ($name:ident, $($to:ident as $ch:expr,)+) => {
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

macro_rules! token_glue_double {
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

token_gen! {
    Eof          as "end of file"   ;,
    Error        as "error token"   ;,
    IntLit       as "int literal"   ;,
    FloatLit     as "float literal" ;,
    Ident        as "identifier"    ;,

    KwBool       as "bool"     ;kw,
    KwS8         as "s8"       ;kw,
    KwS16        as "s16"      ;kw,
    KwS32        as "s32"      ;kw,
    KwS64        as "s64"      ;kw,
    KwSsize      as "ssize"    ;kw,
    KwU8         as "u8"       ;kw,
    KwU16        as "u16"      ;kw,
    KwU32        as "u32"      ;kw,
    KwU64        as "u64"      ;kw,
    KwUsize      as "usize"    ;kw,
    KwF32        as "f32"      ;kw,
    KwF64        as "f64"      ;kw,
    KwChar       as "char"     ;kw,

    KwPub        as "pub"      ;kw,
    KwMod        as "mod"      ;kw,
    KwImport     as "import"   ;kw,
    KwSuper      as "super"    ;kw,
    KwPackage    as "package"  ;kw,
    KwEnum       as "enum"     ;kw,
    KwUnion      as "union"    ;kw,
    KwStruct     as "struct"   ;kw,

    KwFor        as "for"      ;kw,
    KwDefer      as "defer"    ;kw,
    KwBreak      as "break"    ;kw,
    KwContinue   as "continue" ;kw,
    KwReturn     as "return"   ;kw,

    KwAs         as "as"       ;kw,
    KwIf         as "if"       ;kw,
    KwElse       as "else"     ;kw,
    KwMatch      as "match"    ;kw,
    KwSizeof     as "sizeof"   ;kw,
    KwNull       as "null"     ;kw,
    KwTrue       as "true"     ;kw,
    KwFalse      as "false"    ;kw,

    Bang         as "!"  ;,
    Quote2       as "\"" ;,
    Hash         as "#"  ;,
    Dollar       as "$"  ;,
    Percent      as "%"  ;,
    Ampersand    as "&"  ;,
    Quote        as "\'" ;,
    ParenOpen    as "("  ;,
    ParenClose   as ")"  ;,
    Star         as "*"  ;,
    Plus         as "+"  ;,
    Comma        as ","  ;,
    Minus        as "-"  ;,
    Dot          as "."  ;,
    ForwSlash    as "/"  ;,
    Colon        as ":"  ;,
    Semicolon    as ";"  ;,
    Less         as "<"  ;,
    Equals       as "="  ;,
    Greater      as ">"  ;,
    Question     as "?"  ;,
    At           as "@"  ;,
    BracketOpen  as "["  ;,
    BackSlash    as "/"  ;,
    BracketClose as "]"  ;,
    Caret        as "^"  ;,
    Underscore   as "_"  ;kw,
    Backtick     as "`"  ;,
    CurlyOpen    as "{"  ;,
    Pipe         as "|"  ;,
    CurlyClose   as "}"  ;,
    Tilde        as "~"  ;,

    DotDot       as ".." ;,
    ColonColon   as "::" ;,
    ArrowThin    as "->" ;,
    ArrowWide    as "=>" ;,

    BinShl       as "<<" ;,
    BinShr       as ">>" ;,
    BinIsEq      as "==" ;,
    BinNotEq     as "!=" ;,
    BinLessEq    as "<=" ;,
    BinGreaterEq as ">=" ;,
    BinLogicAnd  as "&&" ;,
    BinLogicOr   as "||" ;,

    AssignAdd    as "+=" ;,
    AssignSub    as "-=" ;,
    AssignMul    as "*=" ;,
    AssignDiv    as "/=" ;,
    AssignRem    as "%=" ;,
    AssignBitAnd as "&=" ;,
    AssignBitOr  as "|=" ;,
    AssignBitXor as "^=" ;,
    AssignShl    as "<<=" ;,
    AssignShr    as ">>=" ;,
}

token_glue! {
    glue,
    Bang         as '!',
    Quote2       as '\"',
    Hash         as '#',
    Dollar       as '$',
    Percent      as '%',
    Ampersand    as '&',
    Quote        as '\'',
    ParenOpen    as '(',
    ParenClose   as ')',
    Star         as '*',
    Plus         as '+',
    Comma        as ',',
    Minus        as '-',
    Dot          as '.',
    ForwSlash    as '/',
    Colon        as ':',
    Semicolon    as ';',
    Less         as '<',
    Equals       as '=',
    Greater      as '>',
    Question     as '?',
    At           as '@',
    BracketOpen  as '[',
    BackSlash    as '/',
    BracketClose as ']',
    Caret        as '^',
    Underscore   as '_',
    Backtick     as '`',
    CurlyOpen    as '{',
    Pipe         as '|',
    CurlyClose   as '}',
    Tilde        as '~',
}

token_glue_double! {
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

token_glue_double! {
    glue3,
    ('=') BinShl => AssignShl,
    ('=') BinShr => AssignShr,
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
    Assign,           // =
    BinAssign(BinOp), // =op
}
