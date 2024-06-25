mod token_gen;
pub mod token_list;

use crate::ast::{AssignOp, BasicType, BinOp, UnOp};

#[rustfmt::skip]
token_gen::token_gen! {
    // special tokens
    [eof]           | "end of file"    | Eof          |
    [error]         | "error token"    | Error        |
    [whitespace]    | "whitespace"     | Whitespace   |
    [line_comment]  | "line comment"   | LineComment  |
    [block_comment] | "block comment"  | BlockComment |
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
    [f16]      | "f16"      | KwF16      | KW. BASIC[BasicType::F16]
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
    ["..<"]  | "..<"    | Range        | BIN[BinOp::Range]
    ["..="]  | "..="    | RangeInc     | BIN[BinOp::RangeInc]

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
token_gen::token_from_char! {
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
token_gen::token_glue_extend! {
    glue_double,
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
token_gen::token_glue_extend! {
    glue_triple,
    (T![..] => T!["..<"]) if '<'
    (T![..] => T!["..="])
    (T![<<] => T![<<=])
    (T![>>] => T![>>=]) if '='
}

pub(super) use T;

impl Token {
    pub fn as_bool(self) -> Option<bool> {
        match self {
            T![true] => Some(true),
            T![false] => Some(false),
            _ => None,
        }
    }
}
