use super::span::Span;
use super::token::Token;
use super::FileID;

pub struct ParseErrorData {
    pub file_id: FileID,
    pub ctx: ParseCtx,
    pub expected: Vec<Token>,
    pub got_token: (Token, Span),
}

#[derive(Copy, Clone)]
pub(super) enum ParseError {
    ExpectIdent(ParseCtx),
    ExpectToken(ParseCtx, Token),
    DeclMatch,
    TypeMatch,
    ForAssignOp,
    LitMatch,
    LitInt,
    LitFloat,
    ElseMatch,
    FieldInit,
}

macro_rules! parse_ctx_impl {
    ($($variant:ident as $string:expr)+) => {
        #[derive(Copy, Clone)]
        pub enum ParseCtx {
            $($variant,)+
        }
        impl ParseCtx {
            pub fn as_str(&self) -> &'static str {
                match *self {
                    $(ParseCtx::$variant => $string,)+
                }
            }
        }
    };
}

parse_ctx_impl! {
    Decl        as "declaration"
    ModDecl     as "module declaration"
    UseDecl     as "use declaration"
    ProcDecl    as "procedure declaration"
    ProcParam   as "procedure parameter"
    EnumDecl    as "enum declaration"
    EnumVariant as "enum variant"
    UnionDecl   as "union declaration"
    UnionMember as "union member"
    StructDecl  as "struct declaration"
    StructField as "struct field"
    ConstDecl   as "const declaration"
    GlobalDecl  as "global declaration"

    Path        as "path"
    Type        as "type"
    UnitType    as "unit type"
    ArraySlice  as "array slice type"
    ArrayStatic as "array static type"

    Break       as "break statement"
    Continue    as "continue statement"
    Return      as "return statement"
    ForLoop     as "for loop statement"
    VarDecl     as "variable declaration"
    VarAssign   as "variable assignment"

    Expr        as "expression"
    Lit         as "literal expression"
    LitInt      as "integer literal"
    LitFloat    as "float literal"
    If          as "if expression"
    Block       as "block expression"
    Match       as "match expression"
    MatchArm    as "match arm"
    Sizeof      as "sizeof expression"
    ProcCall    as "procedure call"
    StructInit  as "struct initializer"
    ArrayInit   as "array initializer"
    ExprField   as "field expression"
    ExprIndex   as "index expression"
}

impl ParseErrorData {
    pub(super) fn new(error: ParseError, file_id: FileID, got_token: (Token, Span)) -> Self {
        Self {
            file_id,
            ctx: error.ctx(),
            expected: error.expected(),
            got_token,
        }
    }
}

impl ParseError {
    fn ctx(&self) -> ParseCtx {
        match *self {
            ParseError::ExpectIdent(ctx) => ctx,
            ParseError::ExpectToken(ctx, _) => ctx,
            ParseError::DeclMatch => ParseCtx::Decl,
            ParseError::TypeMatch => ParseCtx::Type,
            ParseError::ForAssignOp => ParseCtx::ForLoop,
            ParseError::LitMatch => ParseCtx::Lit,
            ParseError::LitInt => ParseCtx::LitInt,
            ParseError::LitFloat => ParseCtx::LitFloat,
            ParseError::ElseMatch => ParseCtx::If,
            ParseError::FieldInit => ParseCtx::StructInit,
        }
    }

    fn expected(&self) -> Vec<Token> {
        match *self {
            ParseError::ExpectIdent(_) => vec![Token::Ident],
            ParseError::ExpectToken(_, token) => vec![token],
            ParseError::DeclMatch => vec![
                Token::KwMod,
                Token::KwUse,
                Token::KwProc,
                Token::KwEnum,
                Token::KwUnion,
                Token::KwStruct,
                Token::KwConst,
                Token::KwGlobal,
            ],
            ParseError::TypeMatch => vec![
                Token::Ident,
                Token::KwSuper,
                Token::KwPackage,
                Token::OpenParen,
                Token::OpenBracket,
            ],
            ParseError::ForAssignOp => Self::all_assign_ops(),
            ParseError::LitMatch => Self::all_literal_tokens(),
            ParseError::LitInt => {
                let mut expected = Self::all_integer_types();
                expected.extend(Self::all_float_types());
                expected
            }
            ParseError::LitFloat => Self::all_float_types(),
            ParseError::ElseMatch => vec![Token::KwIf, Token::OpenBlock],
            ParseError::FieldInit => vec![Token::Colon, Token::Comma, Token::CloseBlock],
        }
    }

    fn all_literal_tokens() -> Vec<Token> {
        vec![
            Token::KwNull,
            Token::KwTrue,
            Token::KwFalse,
            Token::IntLit,
            Token::FloatLit,
            Token::CharLit,
            Token::StringLit,
        ]
    }

    fn all_integer_types() -> Vec<Token> {
        vec![
            Token::KwS8,
            Token::KwS16,
            Token::KwS32,
            Token::KwS64,
            Token::KwSsize,
            Token::KwU8,
            Token::KwU16,
            Token::KwU32,
            Token::KwU64,
            Token::KwUsize,
        ]
    }

    fn all_float_types() -> Vec<Token> {
        vec![Token::KwF32, Token::KwF64]
    }

    fn all_assign_ops() -> Vec<Token> {
        vec![
            Token::AssignAdd,
            Token::AssignSub,
            Token::AssignMul,
            Token::AssignDiv,
            Token::AssignRem,
            Token::AssignBitAnd,
            Token::AssignBitOr,
            Token::AssignBitXor,
            Token::AssignShl,
            Token::AssignShr,
        ]
    }
}
