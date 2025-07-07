use crate::ast::{AssignOp, BasicType, BinOp, UnOp};
use crate::intern::LitID;
use crate::text::{TextOffset, TextRange};
use std::mem::transmute;

crate::define_id!(pub TokenID);
crate::define_id!(pub TriviaID);

// `token_data` encoding:
// [normal]  TextOffset(start)
// [special] token_encode index
//
// `token_encode` encoding:
// [ident]      TextRange
// [int_lit]    TextRange + u64
// [float_lit]  TextRange + f64
// [char_lit]   TextRange + char + pad(4)
// [string_lit] TextRange + StringLit
pub struct TokenList {
    tokens: Vec<Token>,
    token_data: Vec<u32>,
    token_encode: Vec<u64>,
    trivias: Vec<Trivia>,
    trivia_ranges: Vec<TextRange>,
}

impl TokenList {
    pub fn new(source: &str) -> TokenList {
        TokenList {
            tokens: Vec::with_capacity(source.len() / 8),
            token_data: Vec::with_capacity(source.len() / 8),
            token_encode: Vec::with_capacity(source.len() / 16),
            trivias: Vec::with_capacity(source.len() / 16),
            trivia_ranges: Vec::with_capacity(source.len() / 16),
        }
    }

    #[inline(always)]
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
    #[inline(always)]
    pub fn token(&self, id: TokenID) -> Token {
        self.tokens[id.index()]
    }
    #[inline(always)]
    pub fn token_and_range(&self, id: TokenID) -> (Token, TextRange) {
        (self.tokens[id.index()], self.token_range(id))
    }
    pub fn token_range(&self, id: TokenID) -> TextRange {
        let token = self.tokens[id.index()];
        match token {
            T![ident] | T![int_lit] | T![float_lit] | T![char_lit] | T![string_lit] => {
                let index = self.token_data[id.index()];
                let decode = self.token_encode[index as usize];
                unsafe { transmute::<u64, TextRange>(decode) }
            }
            _ => {
                let start = self.token_data[id.index()];
                let len = NORMAL_TOKEN_LEN[token as usize];
                TextRange::new(start.into(), (start + len as u32).into())
            }
        }
    }

    #[inline(always)]
    pub fn trivia_count(&self) -> usize {
        self.trivias.len()
    }
    #[inline(always)]
    pub fn trivia(&self, id: TriviaID) -> Trivia {
        self.trivias[id.index()]
    }
    #[inline(always)]
    pub fn trivia_range(&self, id: TriviaID) -> TextRange {
        self.trivia_ranges[id.index()]
    }
    #[inline(always)]
    pub fn trivia_and_range(&self, id: TriviaID) -> (Trivia, TextRange) {
        (self.trivias[id.index()], self.trivia_ranges[id.index()])
    }

    #[inline(always)]
    pub fn int(&self, id: TokenID) -> u64 {
        let index = self.token_data[id.index()];
        self.token_encode[index as usize + 1]
    }
    #[inline(always)]
    pub fn float(&self, id: TokenID) -> f64 {
        let index = self.token_data[id.index()];
        let decode = self.token_encode[index as usize + 1];
        f64::from_bits(decode)
    }
    #[inline(always)]
    pub fn char(&self, id: TokenID) -> char {
        let index = self.token_data[id.index()];
        let decode = self.token_encode[index as usize + 1];
        unsafe { transmute::<u32, char>(decode as u32) }
    }
    #[inline(always)]
    pub fn string(&self, id: TokenID) -> LitID {
        let index = self.token_data[id.index()];
        let decode = self.token_encode[index as usize + 1];
        unsafe { transmute::<u64, (LitID, u32)>(decode).0 }
    }

    #[inline(always)]
    pub fn add_token(&mut self, token: Token, start: TextOffset) {
        self.tokens.push(token);
        self.token_data.push(start.into());
    }
    #[inline(always)]
    pub fn add_ident(&mut self, range: TextRange) {
        self.tokens.push(Token::Ident);
        self.token_data.push(self.token_encode.len() as u32);
        let encode = unsafe { transmute::<TextRange, u64>(range) };
        self.token_encode.push(encode);
    }
    #[inline(always)]
    pub fn add_int(&mut self, int: u64, range: TextRange) {
        self.tokens.push(Token::IntLit);
        self.token_data.push(self.token_encode.len() as u32);
        let encode = unsafe { transmute::<TextRange, u64>(range) };
        self.token_encode.push(encode);
        self.token_encode.push(int);
    }
    #[inline(always)]
    pub fn add_float(&mut self, float: f64, range: TextRange) {
        self.tokens.push(Token::FloatLit);
        self.token_data.push(self.token_encode.len() as u32);
        let encode_range = unsafe { transmute::<TextRange, u64>(range) };
        self.token_encode.push(encode_range);
        self.token_encode.push(float.to_bits());
    }
    #[inline(always)]
    pub fn add_char(&mut self, ch: char, range: TextRange) {
        self.tokens.push(Token::CharLit);
        self.token_data.push(self.token_encode.len() as u32);
        let encode_range = unsafe { transmute::<TextRange, u64>(range) };
        self.token_encode.push(encode_range);
        self.token_encode.push(ch as u64);
    }
    #[inline(always)]
    pub fn add_string(&mut self, id: LitID, range: TextRange) {
        self.tokens.push(Token::StringLit);
        self.token_data.push(self.token_encode.len() as u32);
        let encode_range = unsafe { transmute::<TextRange, u64>(range) };
        let encode_string = unsafe { transmute::<(LitID, u32), u64>((id, 0)) };
        self.token_encode.push(encode_range);
        self.token_encode.push(encode_string);
    }
    #[inline(always)]
    pub fn add_trivia(&mut self, trivia: Trivia, range: TextRange) {
        self.trivias.push(trivia);
        self.trivia_ranges.push(range);
    }
}

#[derive(Copy, Clone)]
pub enum Trivia {
    Whitespace,
    LineComment,
    DocComment,
    ModComment,
}

#[derive(Copy, Clone)]
pub enum SemanticToken {
    Namespace,
    Type,
    Parameter,
    Variable,
    Property,
    EnumMember,
    Function,
    Keyword,
    Comment,
    Number,
    String,
    Operator,
}

#[derive(Clone, Copy)]
pub struct TokenSet {
    mask: u128,
}

impl TokenSet {
    pub const fn new(tokens: &[Token]) -> TokenSet {
        let mut mask = 0u128;
        let mut i = 0;
        while i < tokens.len() {
            mask |= 1u128 << tokens[i] as u8;
            i += 1;
        }
        TokenSet { mask }
    }
    #[inline]
    pub const fn empty() -> TokenSet {
        TokenSet { mask: 0 }
    }
    #[inline]
    pub const fn combine(self, other: TokenSet) -> TokenSet {
        TokenSet { mask: self.mask | other.mask }
    }
    #[inline]
    pub const fn contains(&self, token: Token) -> bool {
        self.mask & (1u128 << token as u8) != 0
    }
}

/// `token_gen` generates `Token` enum itself and various conversions.  
/// `token_from_char` maps char to token.  
/// `token_glue_extend` defines token glueing rules.
/// `T` macro is also generated and allows to reference tokens  
/// without directly using `Token` enum: `T![,] T![:] T![pub]`
#[rustfmt::skip]
macro_rules! token_gen {
    {
    $(
        [$token:tt] | $string:literal | $name:ident |
        $(UN[$un_op:expr])?
        $(BIN[$bin_op:expr])?
        $(ASSIGN[$assign_op:expr])?
        $(BASIC[$basic_ty:expr])?
        $(BOOL[$value:expr])?
    )+
    } => {
        #[macro_export]
        macro_rules! T {
            $( [$token] => [Token::$name]; )+
        }

        /// length in bytes of `normal` tokens,
        /// `special` tokens will have length of 0.
        const NORMAL_TOKEN_LEN: [u8; 96] = {
            let mut token_len: [u8; 96] = [
                $($string.len() as u8,)+
            ];
            token_len[T![eof] as usize] = 0;
            token_len[T![ident] as usize] = 0;
            token_len[T![int_lit] as usize] = 0;
            token_len[T![float_lit] as usize] = 0;
            token_len[T![char_lit] as usize] = 0;
            token_len[T![string_lit] as usize] = 0;
            token_len
        };

        #[derive(Copy, Clone, PartialEq)]
        pub enum Token {
            $( $name, )+
        }
        impl Token {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Token::$name => $string, )+
                }
            }
            pub const fn as_un_op(self) -> Option<UnOp> {
                match self {
                    $( Token::$name => token_gen_arms!(@UN_RES $(UN[$un_op])?), )+
                }
            }
            pub const fn as_bin_op(self) -> Option<BinOp> {
                match self {
                    $( Token::$name => token_gen_arms!(@BIN_RES $(BIN[$bin_op])?), )+
                }
            }
            pub const fn as_assign_op(self) -> Option<AssignOp> {
                match self {
                    $( Token::$name => token_gen_arms!(@ASSIGN_RES $(ASSIGN[$assign_op])?), )+
                }
            }
            pub const fn as_basic_type(self) -> Option<BasicType> {
                match self {
                    $( Token::$name => token_gen_arms!(@BASIC_RES $(BASIC[$basic_ty])?), )+
                }
            }
            pub const fn as_bool(self) -> Option<bool> {
                match self {
                    $( Token::$name => token_gen_arms!(@BOOL_RES $(BOOL[$value])?), )+
                }
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! token_gen_arms {
    (@UN_RES)                             => { None };
    (@UN_RES UN[$un_op:expr])             => { Some($un_op) };
    (@BIN_RES)                            => { None };
    (@BIN_RES BIN[$bin_op:expr])          => { Some($bin_op) };
    (@ASSIGN_RES)                         => { None };
    (@ASSIGN_RES ASSIGN[$assign_op:expr]) => { Some($assign_op) };
    (@BASIC_RES)                          => { None };
    (@BASIC_RES BASIC[$basic_ty:expr])    => { Some($basic_ty) };
    (@BOOL_RES)                           => { None };
    (@BOOL_RES BOOL[$value:expr])         => { Some($value) };
}

#[rustfmt::skip]
macro_rules! token_from_char {
    {
    $(
        $ch:literal => $to:expr
    )+
    } => {
        pub const TOKEN_BYTE_TO_SINGLE: [Token; 128] = {
            let mut table = [Token::Eof; 128];
            $(table[$ch as usize] = $to;)+
            table
        };

        impl Token {
            pub const fn from_char(c: char) -> Option<Token> {
                match c {
                    $(
                        $ch => Some($to),
                    )+
                    _ => None,
                }
            }
        }
    };
}

#[rustfmt::skip]
macro_rules! token_glue_extend {
    {
    fn $name:ident,
    $(
        $( ($from:pat => $to:expr) )+ if $ch:literal
    )+
    } => {
        impl Token {
            #[inline(always)]
            pub const fn $name(c: u8, token: Token) -> Token {
                match c {
                    $(
                        $ch => match token {
                            $( $from => $to, )+
                            _ => token,
                        },
                    )+
                    _ => token,
                }
            }
        }
    };
}

token_gen! {
    // special tokens
    [eof]           | "end of file"    | Eof          |
    [ident]         | "identifier"     | Ident        |
    [int_lit]       | "int literal"    | IntLit       |
    [float_lit]     | "float literal"  | FloatLit     |
    [char_lit]      | "char literal"   | CharLit      |
    [string_lit]    | "string literal" | StringLit    |

    // keyword items
    [proc]     | "proc"     | KwProc     |
    [enum]     | "enum"     | KwEnum     |
    [struct]   | "struct"   | KwStruct   |
    [import]   | "import"   | KwImport   |

    // keyword statements
    [break]    | "break"    | KwBreak    |
    [continue] | "continue" | KwContinue |
    [return]   | "return"   | KwReturn   |
    [defer]    | "defer"    | KwDefer    |
    [for]      | "for"      | KwFor      |
    [in]       | "in"       | KwIn       |
    [let]      | "let"      | KwLet      |
    [mut]      | "mut"      | KwMut      |
    [zeroed]   | "zeroed"   | KwZeroed   |
    [undefined]| "undefined"| KwUndefined|

    // keyword expressions
    [null]     | "null"     | KwNull     |
    [true]     | "true"     | KwTrue     | BOOL[true]
    [false]    | "false"    | KwFalse    | BOOL[false]
    [if]       | "if"       | KwIf       |
    [else]     | "else"     | KwElse     |
    [match]    | "match"    | KwMatch    |
    [as]       | "as"       | KwAs       |
    [_]        | "_"        | KwDiscard  |

    // keyword basic types
    [s8]       | "s8"       | KwS8       | BASIC[BasicType::S8]
    [s16]      | "s16"      | KwS16      | BASIC[BasicType::S16]
    [s32]      | "s32"      | KwS32      | BASIC[BasicType::S32]
    [s64]      | "s64"      | KwS64      | BASIC[BasicType::S64]
    [ssize]    | "ssize"    | KwSsize    | BASIC[BasicType::Ssize]
    [u8]       | "u8"       | KwU8       | BASIC[BasicType::U8]
    [u16]      | "u16"      | KwU16      | BASIC[BasicType::U16]
    [u32]      | "u32"      | KwU32      | BASIC[BasicType::U32]
    [u64]      | "u64"      | KwU64      | BASIC[BasicType::U64]
    [usize]    | "usize"    | KwUsize    | BASIC[BasicType::Usize]
    [f32]      | "f32"      | KwF32      | BASIC[BasicType::F32]
    [f64]      | "f64"      | KwF64      | BASIC[BasicType::F64]
    [bool]     | "bool"     | KwBool     | BASIC[BasicType::Bool]
    [bool16]   | "bool16"   | KwBool16   | BASIC[BasicType::Bool16]
    [bool32]   | "bool32"   | KwBool32   | BASIC[BasicType::Bool32]
    [bool64]   | "bool64"   | KwBool64   | BASIC[BasicType::Bool64]
    [string]   | "string"   | KwString   | BASIC[BasicType::String]
    [cstring]  | "cstring"  | KwCstring  | BASIC[BasicType::CString]
    [char]     | "char"     | KwChar     | BASIC[BasicType::Char]
    [void]     | "void"     | KwVoid     | BASIC[BasicType::Void]
    [never]    | "never"    | KwNever    | BASIC[BasicType::Never]
    [rawptr]   | "rawptr"   | KwRawptr   | BASIC[BasicType::Rawptr]

    // single punctuation
    [.]      | "."      | Dot          |
    [,]      | ","      | Comma        |
    [:]      | ":"      | Colon        |
    [;]      | ";"      | Semicolon    |
    [#]      | "#"      | Hash         |
    ['(']    | "("      | ParenOpen    |
    [')']    | ")"      | ParenClose   |
    ['[']    | "["      | BracketOpen  |
    [']']    | "]"      | BracketClose |
    ['{']    | "{"      | BlockOpen    |
    ['}']    | "}"      | BlockClose   |

    // double punctuation
    [..]     | ".."     | DotDot       |
    [->]     | "->"     | ArrowThin    |

    // triple punctuation
    ["..<"]  | "..<"    | Range        |
    ["..="]  | "..="    | RangeInc     |

    // un op tokens
    [~]      | "~"      | Tilde        | UN[UnOp::BitNot]
    [!]      | "!"      | Bang         | UN[UnOp::LogicNot]

    // bin op tokens
    [+]      | "+"      | Plus         | BIN[BinOp::Add]
    [-]      | "-"      | Minus        | UN[UnOp::Neg] BIN[BinOp::Sub]
    [*]      | "*"      | Star         | BIN[BinOp::Mul]
    [/]      | "/"      | ForwSlash    | BIN[BinOp::Div]
    [%]      | "%"      | Percent      | BIN[BinOp::Rem]
    [&]      | "&"      | Ampersand    | BIN[BinOp::BitAnd]
    [|]      | "|"      | Pipe         | BIN[BinOp::BitOr]
    [^]      | "^"      | Caret        | BIN[BinOp::BitXor]
    [<<]     | "<<"     | Shl          | BIN[BinOp::BitShl]
    [>>]     | ">>"     | Shr          | BIN[BinOp::BitShr]
    [==]     | "=="     | Eq           | BIN[BinOp::Eq]
    [!=]     | "!="     | NotEq        | BIN[BinOp::NotEq]
    [<]      | "<"      | Less         | BIN[BinOp::Less]
    [<=]     | "<="     | LessEq       | BIN[BinOp::LessEq]
    [>]      | ">"      | Greater      | BIN[BinOp::Greater]
    [>=]     | ">="     | GreaterEq    | BIN[BinOp::GreaterEq]
    [&&]     | "&&"     | LogicAnd     | BIN[BinOp::LogicAnd]
    [||]     | "||"     | LogicOr      | BIN[BinOp::LogicOr]

    // assign op tokens
    [=]      | "="      | Equals       | ASSIGN[AssignOp::Assign]
    [+=]     | "+="     | AssignAdd    | ASSIGN[AssignOp::Bin(BinOp::Add)]
    [-=]     | "-="     | AssignSub    | ASSIGN[AssignOp::Bin(BinOp::Sub)]
    [*=]     | "*="     | AssignMul    | ASSIGN[AssignOp::Bin(BinOp::Mul)]
    [/=]     | "/="     | AssignDiv    | ASSIGN[AssignOp::Bin(BinOp::Div)]
    [%=]     | "%="     | AssignRem    | ASSIGN[AssignOp::Bin(BinOp::Rem)]
    [&=]     | "&="     | AssignBitAnd | ASSIGN[AssignOp::Bin(BinOp::BitAnd)]
    [|=]     | "|="     | AssignBitOr  | ASSIGN[AssignOp::Bin(BinOp::BitOr)]
    [^=]     | "^="     | AssignBitXor | ASSIGN[AssignOp::Bin(BinOp::BitXor)]
    [<<=]    | "<<="    | AssignShl    | ASSIGN[AssignOp::Bin(BinOp::BitShl)]
    [>>=]    | ">>="    | AssignShr    | ASSIGN[AssignOp::Bin(BinOp::BitShr)]
}

token_from_char! {
    '.' => T![.]
    ',' => T![,]
    ':' => T![:]
    ';' => T![;]
    '#' => T![#]
    '(' => T!['(']
    ')' => T![')']
    '[' => T!['[']
    ']' => T![']']
    '{' => T!['{']
    '}' => T!['}']

    '~' => T![~]
    '!' => T![!]

    '+' => T![+]
    '-' => T![-]
    '*' => T![*]
    '/' => T![/]
    '%' => T![%]
    '&' => T![&]
    '|' => T![|]
    '^' => T![^]
    '<' => T![<]
    '>' => T![>]

    '=' => T![=]
}

token_glue_extend! {
    fn glue_double,
    (T![.] => T![..]) if b'.'
    (T![-] => T![->])
    (T![>] => T![>>]) if b'>'
    (T![<] => T![<<]) if b'<'
    (T![&] => T![&&]) if b'&'
    (T![|] => T![||]) if b'|'

    (T![+] => T![+=])
    (T![-] => T![-=])
    (T![*] => T![*=])
    (T![/] => T![/=])
    (T![%] => T![%=])
    (T![&] => T![&=])
    (T![|] => T![|=])
    (T![^] => T![^=])
    (T![=] => T![==])
    (T![!] => T![!=])
    (T![<] => T![<=])
    (T![>] => T![>=]) if b'='
}

token_glue_extend! {
    fn glue_triple,
    (T![..] => T!["..<"]) if b'<'
    (T![..] => T!["..="])
    (T![<<] => T![<<=])
    (T![>>] => T![>>=]) if b'='
}

pub(crate) use T;
