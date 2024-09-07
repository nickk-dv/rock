use crate::ast::{AssignOp, BasicType, BinOp, UnOp};
use crate::support::{IndexID, ID};
use crate::text::TextRange;

pub struct TokenList {
    tokens: Vec<Token>,
    token_ranges: Vec<TextRange>,
    trivias: Vec<Trivia>,
    trivia_ranges: Vec<TextRange>,
    ints: Vec<u64>,
    chars: Vec<char>,
    strings: Vec<(String, bool)>,
}

impl TokenList {
    pub fn new(cap: usize) -> TokenList {
        TokenList {
            tokens: Vec::with_capacity(cap),
            token_ranges: Vec::with_capacity(cap),
            trivias: Vec::new(),
            trivia_ranges: Vec::new(),
            ints: Vec::new(),
            chars: Vec::new(),
            strings: Vec::new(),
        }
    }

    pub fn token(&self, id: ID<Token>) -> Token {
        *self.tokens.id_get(id)
    }
    pub fn token_range(&self, id: ID<Token>) -> TextRange {
        self.token_ranges[id.raw_index()]
    }
    pub fn token_and_range(&self, id: ID<Token>) -> (Token, TextRange) {
        (*self.tokens.id_get(id), self.token_ranges[id.raw_index()])
    }

    pub fn trivia(&self, id: ID<Trivia>) -> Trivia {
        *self.trivias.id_get(id)
    }
    pub fn trivia_range(&self, id: ID<Trivia>) -> TextRange {
        self.trivia_ranges[id.raw_index()]
    }
    pub fn trivia_and_range(&self, id: ID<Trivia>) -> (Trivia, TextRange) {
        (*self.trivias.id_get(id), self.trivia_ranges[id.raw_index()])
    }
    pub fn trivia_count(&self) -> usize {
        self.trivias.len()
    }

    pub fn int(&self, id: ID<u64>) -> u64 {
        *self.ints.id_get(id)
    }
    pub fn char(&self, id: ID<char>) -> char {
        *self.chars.id_get(id)
    }
    pub fn string(&self, id: ID<(String, bool)>) -> (&str, bool) {
        let (string, c_string) = self.strings.id_get(id);
        (string, *c_string)
    }

    pub fn add_token(&mut self, token: Token, range: TextRange) {
        self.tokens.push(token);
        self.token_ranges.push(range);
    }
    pub fn add_trivia(&mut self, trivia: Trivia, range: TextRange) {
        self.trivias.push(trivia);
        self.trivia_ranges.push(range);
    }
    pub fn add_int(&mut self, int: u64, range: TextRange) {
        self.tokens.push(Token::IntLit);
        self.token_ranges.push(range);
        self.ints.push(int);
    }
    pub fn add_char(&mut self, ch: char, range: TextRange) {
        self.tokens.push(Token::CharLit);
        self.token_ranges.push(range);
        self.chars.push(ch);
    }
    pub fn add_string(&mut self, string: String, c_string: bool, range: TextRange) {
        self.tokens.push(Token::StringLit);
        self.token_ranges.push(range);
        self.strings.push((string, c_string));
    }
}

#[derive(Clone, Copy)]
pub struct TokenSet(u128);

impl TokenSet {
    pub const fn new(tokens: &[Token]) -> TokenSet {
        let mut bitset = 0u128;
        let mut i = 0;
        while i < tokens.len() {
            bitset |= 1u128 << tokens[i] as u8;
            i += 1;
        }
        TokenSet(bitset)
    }
    #[inline]
    pub const fn empty() -> TokenSet {
        TokenSet(0)
    }
    #[inline]
    pub const fn combine(self, other: TokenSet) -> TokenSet {
        TokenSet(self.0 | other.0)
    }
    #[inline]
    pub const fn contains(&self, token: Token) -> bool {
        self.0 & 1u128 << token as u8 != 0
    }
}

#[derive(Copy, Clone)]
pub enum Trivia {
    Whitespace,
    LineComment,
    BlockComment,
}

/// Defines a DSL-like macro that automates token definition and conversions.
///
/// `token_gen` generates `Token` enum itself and various conversions.  
/// `token_from_char` maps char to token.  
/// `token_glue_extend` defines token glueing rules.
///
/// `T` macro is also generated and allows to reference tokens  
/// without directly using `Token` enum: `T![,] T![:] T![pub]`

#[rustfmt::skip]
macro_rules! token_gen {
    {
    $(
        [$token:tt] | $string:literal | $name:ident |
        $(KW $mark:tt)?
        $(BIN[$bin_op:expr])?
        $(UN[$un_op:expr])?
        $(ASSIGN[$assign_op:expr])?
        $(BASIC[$basic_ty:expr])?
    )+
    } => {
        macro_rules! T {
            $( [$token] => [Token::$name]; )+
        }
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
            pub fn as_keyword(ident: &str) -> Option<Token> {
                match ident {
                    $( $string => token_gen_arms!(@KW_RES $name $(KW $mark)?), )+
                    _ => None,
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
        }
    };
}

#[rustfmt::skip]
macro_rules! token_gen_arms {
    (@KW_RES $name:ident)                 => { None };
    (@UN_RES)                             => { None };
    (@BIN_RES)                            => { None };
    (@ASSIGN_RES)                         => { None };
    (@BASIC_RES)                          => { None };
    (@KW_RES $name:ident KW $mark:tt)     => { Some(Token::$name) };
    (@UN_RES UN[$un_op:expr])             => { Some($un_op) };
    (@BIN_RES BIN[$bin_op:expr])          => { Some($bin_op) };
    (@ASSIGN_RES ASSIGN[$assign_op:expr]) => { Some($assign_op) };
    (@BASIC_RES BASIC[$basic_ty:expr])    => { Some($basic_ty) };
}

#[rustfmt::skip]
macro_rules! token_from_char {
    {
    $(
        $ch:literal => $to:expr
    )+
    } => {
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
            pub const fn $name(c: char, token: Token) -> Option<Token> {
                match c {
                    $(
                        $ch => match token {
                            $( $from => Some($to), )+
                            _ => None,
                        },
                    )+
                    _ => None,
                }
            }
        }
    };
}

#[rustfmt::skip]
token_gen! {
    // special tokens
    [eof]           | "end of file"    | Eof          |
    [ident]         | "identifier"     | Ident        |
    [int_lit]       | "int literal"    | IntLit       |
    [float_lit]     | "float literal"  | FloatLit     |
    [char_lit]      | "char literal"   | CharLit      |
    [string_lit]    | "string literal" | StringLit    |

    // keyword items
    [pub]      | "pub"      | KwPub      | KW.
    [proc]     | "proc"     | KwProc     | KW.
    [enum]     | "enum"     | KwEnum     | KW.
    [struct]   | "struct"   | KwStruct   | KW.
    [const]    | "const"    | KwConst    | KW.
    [global]   | "global"   | KwGlobal   | KW.
    [import]   | "import"   | KwImport   | KW.

    // keyword statements
    [break]    | "break"    | KwBreak    | KW.
    [continue] | "continue" | KwContinue | KW.
    [return]   | "return"   | KwReturn   | KW.
    [defer]    | "defer"    | KwDefer    | KW.
    [for]      | "for"      | KwFor      | KW.
    [let]      | "let"      | KwLet      | KW.
    [mut]      | "mut"      | KwMut      | KW.

    // keyword expressions
    [null]     | "null"     | KwNull     | KW.
    [true]     | "true"     | KwTrue     | KW.
    [false]    | "false"    | KwFalse    | KW.
    [if]       | "if"       | KwIf       | KW.
    [else]     | "else"     | KwElse     | KW.
    [match]    | "match"    | KwMatch    | KW.
    [match2]   | "match2"   | KwMatch2   | KW.
    [_]        | "_"        | KwDiscard  | KW.
    [as]       | "as"       | KwAs       | KW.
    [sizeof]   | "sizeof"   | KwSizeof   | KW.

    // keyword basic types
    [s8]       | "s8"       | KwS8       | KW. BASIC[BasicType::S8]
    [s16]      | "s16"      | KwS16      | KW. BASIC[BasicType::S16]
    [s32]      | "s32"      | KwS32      | KW. BASIC[BasicType::S32]
    [s64]      | "s64"      | KwS64      | KW. BASIC[BasicType::S64]
    [ssize]    | "ssize"    | KwSsize    | KW. BASIC[BasicType::Ssize]
    [u8]       | "u8"       | KwU8       | KW. BASIC[BasicType::U8]
    [u16]      | "u16"      | KwU16      | KW. BASIC[BasicType::U16]
    [u32]      | "u32"      | KwU32      | KW. BASIC[BasicType::U32]
    [u64]      | "u64"      | KwU64      | KW. BASIC[BasicType::U64]
    [usize]    | "usize"    | KwUsize    | KW. BASIC[BasicType::Usize]
    [f32]      | "f32"      | KwF32      | KW. BASIC[BasicType::F32]
    [f64]      | "f64"      | KwF64      | KW. BASIC[BasicType::F64]
    [bool]     | "bool"     | KwBool     | KW. BASIC[BasicType::Bool]
    [char]     | "char"     | KwChar     | KW. BASIC[BasicType::Char]
    [rawptr]   | "rawptr"   | KwRawptr   | KW. BASIC[BasicType::Rawptr]
    [void]     | "void"     | KwVoid     | KW. BASIC[BasicType::Void]
    [never]    | "never"    | KwNever    | KW. BASIC[BasicType::Never]

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
    [-]      | "-"      | Minus        | BIN[BinOp::Sub] UN[UnOp::Neg]
    [*]      | "*"      | Star         | BIN[BinOp::Mul]
    [/]      | "/"      | ForwSlash    | BIN[BinOp::Div]
    [%]      | "%"      | Percent      | BIN[BinOp::Rem]
    [&]      | "&"      | Ampersand    | BIN[BinOp::BitAnd]
    [|]      | "|"      | Pipe         | BIN[BinOp::BitOr]
    [^]      | "^"      | Caret        | BIN[BinOp::BitXor]
    [<<]     | "<<"     | Shl          | BIN[BinOp::BitShl]
    [>>]     | ">>"     | Shr          | BIN[BinOp::BitShr]
    [==]     | "=="     | IsEq         | BIN[BinOp::IsEq]
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

#[rustfmt::skip]
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

#[rustfmt::skip]
token_glue_extend! {
    fn glue_double,
    (T![.] => T![..]) if '.'
    (T![-] => T![->])
    (T![>] => T![>>]) if '>'
    (T![<] => T![<<]) if '<'
    (T![&] => T![&&]) if '&'
    (T![|] => T![||]) if '|'

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
    (T![>] => T![>=]) if '='
}

#[rustfmt::skip]
token_glue_extend! {
    fn glue_triple,
    (T![..] => T!["..<"]) if '<'
    (T![..] => T!["..="])
    (T![<<] => T![<<=])
    (T![>>] => T![>>=]) if '='
}

pub(crate) use T;

impl Token {
    pub fn as_bool(self) -> Option<bool> {
        match self {
            T![true] => Some(true),
            T![false] => Some(false),
            _ => None,
        }
    }
}
