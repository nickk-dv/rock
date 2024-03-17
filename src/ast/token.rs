use super::ast::{AssignOp, BasicType, BinOp, Mut, UnOp};
use super::token_gen;

#[rustfmt::skip]
token_gen::token_gen! {
    // special tokens
    [eof]        | "end of file"    | Eof        |
    [error]      | "error token"    | Error      |
    [whitespace] | "whitespace"     | Whitespace |
    [ident]      | "identifier"     | Ident      |
    [int_lit]    | "int literal"    | IntLit     |
    [float_lit]  | "float literal"  | FloatLit   |
    [char_lit]   | "char literal"   | CharLit    |
    [string_lit] | "string literal" | StringLit  |

    // keyword general
    [pub]      | "pub"      | KwPub      | KW.
    [super]    | "super"    | KwSuper    | KW.
    [package]  | "package"  | KwPackage  | KW.

    // keyword declarations
    [use]      | "use"      | KwUse      | KW.
    [mod]      | "mod"      | KwMod      | KW.
    [proc]     | "proc"     | KwProc     | KW.
    [enum]     | "enum"     | KwEnum     | KW.
    [union]    | "union"    | KwUnion    | KW.
    [struct]   | "struct"   | KwStruct   | KW.
    [const]    | "const"    | KwConst    | KW.
    [global]   | "global"   | KwGlobal   | KW.

    // keyword statements
    [let]      | "let"      | KwLet      | KW.
    [mut]      | "mut"      | KwMut      | KW.
    [break]    | "break"    | KwBreak    | KW.
    [continue] | "continue" | KwContinue | KW.
    [return]   | "return"   | KwReturn   | KW.
    [for]      | "for"      | KwFor      | KW.
    [defer]    | "defer"    | KwDefer    | KW.

    // keyword expressions
    [null]     | "null"     | KwNull     | KW.
    [true]     | "true"     | KwTrue     | KW.
    [false]    | "false"    | KwFalse    | KW.
    [if]       | "if"       | KwIf       | KW.
    [else]     | "else"     | KwElse     | KW.
    [match]    | "match"    | KwMatch    | KW.
    [as]       | "as"       | KwAs       | KW.
    [sizeof]   | "sizeof"   | KwSizeof   | KW.

    // keyword basic types
    [bool]     | "bool"     | KwBool     | KW. BASIC[BasicType::Bool]
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
    [char]     | "char"     | KwChar     | KW. BASIC[BasicType::Char]
    [rawptr]   | "rawptr"   | Rawptr     | KW. BASIC[BasicType::Rawptr]

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
    [=>]     | "=>"     | ArrowWide    |

    // un op tokens
    [~]      | "~"      | Tilde        | UN[UnOp::BitNot]
    [!]      | "!"      | Bang         | UN[UnOp::LogicNot]

    // bin op tokens
    [+]      | "+"      | Plus         | BIN[BinOp::Add]
    [-]      | "-"      | Minus        | BIN[BinOp::Sub] UN[UnOp::Neg]
    [*]      | "*"      | Star         | BIN[BinOp::Mul] UN[UnOp::Deref]
    [/]      | "/"      | ForwSlash    | BIN[BinOp::Div]
    [%]      | "%"      | Percent      | BIN[BinOp::Rem]
    [&]      | "&"      | Ampersand    | BIN[BinOp::BitAnd] UN[UnOp::Addr(Mut::Immutable)]
    [|]      | "|"      | Pipe         | BIN[BinOp::BitOr]
    [^]      | "^"      | Caret        | BIN[BinOp::BitXor]
    [<<]     | "<<"     | BinShl       | BIN[BinOp::BitShl]
    [>>]     | ">>"     | BinShr       | BIN[BinOp::BitShr]
    [==]     | "=="     | BinIsEq      | BIN[BinOp::CmpIsEq]
    [!=]     | "!="     | BinNotEq     | BIN[BinOp::CmpNotEq]
    [<]      | "<"      | Less         | BIN[BinOp::CmpLt]
    [<=]     | "<="     | BinLessEq    | BIN[BinOp::CmpLtEq]
    [>]      | ">"      | Greater      | BIN[BinOp::CmpGt]
    [>=]     | ">="     | BinGreaterEq | BIN[BinOp::CmpGtEq]
    [&&]     | "&&"     | BinLogicAnd  | BIN[BinOp::LogicAnd]
    [||]     | "||"     | BinLogicOr   | BIN[BinOp::LogicOr]

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
    (T![<] => T![<<]) if '<'
    (T![&] => T![&&]) if '&'
    (T![|] => T![||]) if '|'

    (T![-] => T![->])
    (T![=] => T![=>])
    (T![>] => T![>>]) if '>'

    (T![=] => T![==])
    (T![!] => T![!=])
    (T![<] => T![<=])
    (T![>] => T![>=])
    (T![+] => T![+=])
    (T![-] => T![-=])
    (T![*] => T![*=])
    (T![/] => T![/=])
    (T![%] => T![%=])
    (T![&] => T![&=])
    (T![|] => T![|=])
    (T![^] => T![^=]) if '='
}

#[rustfmt::skip]
token_gen::token_glue_extend! {
    glue_triple,
    (T![<<] => T![<<=])
    (T![>>] => T![>>=]) if '='
}

pub(super) use T;
