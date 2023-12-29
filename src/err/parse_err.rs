use crate::ast::token::*;

pub struct ParseError {
    pub context: ParseContext,
    pub expected: Vec<Token>,
    pub got_token: TokenSpan,
}

pub enum ParserError {
    Ident(ParseContext),
    TypeMatch,
    DeclMatch,
    ImportTargetMatch,
    StmtMatch,
    PrimaryExprIdent,
    PrimaryExprMatch,
    AccessMatch,
    LiteralMatch,
    ExpectToken(ParseContext, Token),
    ExpectAssignOp(ParseContext),
}

#[derive(Copy, Clone)]
pub enum ParseContext {
    ModuleAccess,
    Type,
    CustomType,
    ArraySliceType,
    ArrayStaticType,
    Decl,
    ModDecl,
    ProcDecl,
    ProcParam,
    EnumDecl,
    EnumVariant,
    StructDecl,
    StructField,
    GlobalDecl,
    ImportDecl,
    Stmt,
    If,
    For,
    Block,
    Defer,
    Break,
    Switch,
    SwitchCase,
    Return,
    Continue,
    VarDecl,
    VarAssign,
    Expr,
    Var,
    Access,
    ArrayAccess,
    Enum,
    Cast,
    Sizeof,
    Literal,
    ProcCall,
    ArrayInit,
    StructInit,
}

impl ParseError {
    fn new(got_token: TokenSpan, context: ParseContext, expected: Vec<Token>) -> Self {
        Self {
            context,
            expected,
            got_token,
        }
    }
}

impl ParserError {
    pub fn to_parse_error(&self, token: TokenSpan) -> ParseError {
        match self {
            ParserError::Ident(context) => ParseError::new(token, *context, vec![Token::Ident]),
            ParserError::TypeMatch => ParseError::new(
                token,
                ParseContext::Type,
                vec![Token::Ident, Token::OpenBracket],
            ),
            ParserError::DeclMatch => ParseError::new(
                token,
                ParseContext::Decl,
                vec![Token::Ident, Token::KwPub, Token::KwImport],
            ),
            ParserError::ImportTargetMatch => ParseError::new(
                token,
                ParseContext::ImportDecl,
                vec![Token::Ident, Token::Times, Token::OpenBracket],
            ),
            ParserError::StmtMatch => ParseError::new(
                token,
                ParseContext::Stmt,
                vec![
                    Token::KwIf,
                    Token::KwFor,
                    Token::OpenBlock,
                    Token::KwDefer,
                    Token::KwBreak,
                    Token::KwSwitch,
                    Token::KwReturn,
                    Token::KwContinue,
                    Token::Ident,
                ],
            ),
            ParserError::PrimaryExprIdent => {
                ParseError::new(token, ParseContext::Expr, vec![Token::Ident])
            }
            ParserError::PrimaryExprMatch => {
                let mut expected = vec![
                    Token::Ident,
                    Token::KwSuper,
                    Token::KwPackage,
                    Token::Dot,
                    Token::OpenBracket,
                    Token::OpenBlock,
                    Token::KwCast,
                    Token::KwSizeof,
                ];
                expected.extend(ParserError::all_literal_tokens());
                ParseError::new(token, ParseContext::Expr, expected)
            }
            ParserError::AccessMatch => ParseError::new(
                token,
                ParseContext::Access,
                vec![Token::Dot, Token::OpenBracket],
            ),
            ParserError::LiteralMatch => ParseError::new(
                token,
                ParseContext::Literal,
                ParserError::all_literal_tokens(),
            ),
            ParserError::ExpectToken(context, expected) => {
                ParseError::new(token, *context, vec![*expected])
            }
            ParserError::ExpectAssignOp(context) => {
                ParseError::new(token, *context, ParserError::all_assign_op_tokens())
            }
        }
    }

    fn all_literal_tokens() -> Vec<Token> {
        vec![
            Token::LitNull,
            Token::LitInt(u64::default()),
            Token::LitFloat(f64::default()),
            Token::LitChar(char::default()),
            Token::LitString,
        ]
    }

    fn all_assign_op_tokens() -> Vec<Token> {
        vec![
            Token::Assign,
            Token::PlusEq,
            Token::MinusEq,
            Token::TimesEq,
            Token::DivEq,
            Token::ModEq,
            Token::BitAndEq,
            Token::BitOrEq,
            Token::BitXorEq,
            Token::ShlEq,
            Token::ShrEq,
        ]
    }
}

impl ParseContext {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseContext::ModuleAccess => "module access",
            ParseContext::Type => "type signature",
            ParseContext::CustomType => "custom type",
            ParseContext::ArraySliceType => "array slice type",
            ParseContext::ArrayStaticType => "static array type",
            ParseContext::Decl => "declaration",
            ParseContext::ModDecl => "module declaration",
            ParseContext::ProcDecl => "procedure declaration",
            ParseContext::ProcParam => "procedure parameter",
            ParseContext::EnumDecl => "enum declaration",
            ParseContext::EnumVariant => "enum variant",
            ParseContext::StructDecl => "struct declaration",
            ParseContext::StructField => "struct field",
            ParseContext::GlobalDecl => "global declaration",
            ParseContext::ImportDecl => "import declaration",
            ParseContext::Stmt => "statement",
            ParseContext::If => "if statement",
            ParseContext::For => "for loop statement",
            ParseContext::Block => "statement block",
            ParseContext::Defer => "defer statement",
            ParseContext::Break => "break statement",
            ParseContext::Switch => "switch statement",
            ParseContext::SwitchCase => "switch cast",
            ParseContext::Return => "return statement",
            ParseContext::Continue => "continue statement",
            ParseContext::VarDecl => "variable declaration",
            ParseContext::VarAssign => "variable assignment",
            ParseContext::Expr => "expression",
            ParseContext::Var => "variable",
            ParseContext::Access => "access chain",
            ParseContext::ArrayAccess => "array access",
            ParseContext::Enum => "enum expression",
            ParseContext::Cast => "cast expression",
            ParseContext::Sizeof => "sizeof expression",
            ParseContext::Literal => "literal",
            ParseContext::ProcCall => "procedure call",
            ParseContext::ArrayInit => "array initializer",
            ParseContext::StructInit => "struct initializer",
        }
    }
}
