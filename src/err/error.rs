use crate::{
    ast::{
        ast::Module,
        span::Span,
        token::{Token, TokenSpan},
    },
    mem::P,
};

enum Error {
    Parse(ParseErrorData),
    Check(CheckErrorData),
    FileIO(FileIOErrorData),
    Internal(InternalErrorData),
}

impl Error {
    fn parse(error: ParseError, source: P<Module>, got_token: TokenSpan) -> Self {
        Self::Parse(ParseErrorData::new(error, source, got_token))
    }

    fn check(error: CheckError, source: P<Module>, span: Span) -> CheckErrorData {
        CheckErrorData::new(error, source, span)
    }

    fn file_io(error: FileIOError) -> Self {
        Self::FileIO(FileIOErrorData::from(error))
    }

    fn internal(error: InternalError) -> Self {
        Self::Internal(InternalErrorData::from(error))
    }
}

impl ParseErrorData {
    fn new(error: ParseError, source: P<Module>, got_token: TokenSpan) -> Self {
        Self {
            source,
            context: Self::error_context(error),
            expected: Self::error_expected(error),
            got_token,
        }
    }

    fn error_context(error: ParseError) -> ParseContext {
        match error {
            ParseError::Ident(c) => c,
            ParseError::TypeMatch => ParseContext::Type,
            ParseError::DeclMatch => ParseContext::Decl,
            ParseError::ImportTargetMatch => ParseContext::ImportDecl,
            ParseError::StmtMatch => ParseContext::Stmt,
            ParseError::PrimaryExprIdent => ParseContext::Expr,
            ParseError::PrimaryExprMatch => ParseContext::Expr,
            ParseError::AccessMatch => ParseContext::Access,
            ParseError::LiteralMatch => ParseContext::Literal,
            ParseError::ExpectToken(c, ..) => c,
            ParseError::ExpectAssignOp(c) => c,
        }
    }

    fn error_expected(error: ParseError) -> Vec<Token> {
        match error {
            ParseError::Ident(..) => vec![Token::Ident],
            ParseError::TypeMatch => vec![Token::Ident, Token::OpenBracket],
            ParseError::DeclMatch => vec![Token::Ident, Token::KwPub, Token::KwImport],
            ParseError::ImportTargetMatch => vec![Token::Ident, Token::Times, Token::OpenBlock],
            ParseError::StmtMatch => vec![
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
            ParseError::PrimaryExprIdent => vec![Token::Ident],
            ParseError::PrimaryExprMatch => {
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
                expected.extend(Self::all_literal_tokens());
                expected
            }
            ParseError::AccessMatch => vec![Token::Dot, Token::OpenBracket],
            ParseError::LiteralMatch => Self::all_literal_tokens(),
            ParseError::ExpectToken(.., t) => vec![t],
            ParseError::ExpectAssignOp(..) => vec![
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
            ],
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
}

#[derive(Copy, Clone)]
enum ParseError {
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
enum ParseContext {
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

enum CheckError {
    SymbolRedefinition,
    ProcParamRedefinition,
    EnumVariantRedefinition,
    StructFieldRedefinition,
}

struct ParseErrorData {
    source: P<Module>,
    context: ParseContext,
    expected: Vec<Token>,
    got_token: TokenSpan,
}

struct CheckErrorData {
    message: &'static str,
    help: Option<&'static str>,
    source: P<Module>,
    span: Span,
    info: Vec<CheckErrorInfoData>,
}

struct CheckErrorInfoData {
    source: P<Module>,
    span: Span,
    marker: &'static str,
}

struct FileIOErrorData {
    message: &'static str,
    help: Option<&'static str>,
}

struct InternalErrorData {
    message: &'static str,
    help: Option<&'static str>,
}

impl ParseErrorData {}

enum FileIOError {
    DirRead,
    DirCreate,
    FileRead,
    FileCreate,
    FileWrite,
}

impl FileIOErrorData {
    fn new(message: &'static str, help: Option<&'static str>) -> Self {
        Self { message, help }
    }

    fn from(error: FileIOError) -> Self {
        match error {
            FileIOError::DirRead => Self::new("file io error", None),
            FileIOError::DirCreate => Self::new("file io error", None),
            FileIOError::FileRead => Self::new("file io error", None),
            FileIOError::FileCreate => Self::new("file io error", None),
            FileIOError::FileWrite => Self::new("file io error", None),
        }
    }
}

enum InternalError {
    Internal,
}

impl InternalErrorData {
    fn new(message: &'static str, help: Option<&'static str>) -> Self {
        Self { message, help }
    }

    fn from(error: InternalError) -> Self {
        match error {
            InternalError::Internal => Self::new("internal error", None),
        }
    }
}

impl CheckErrorData {
    fn new(error: CheckError, source: P<Module>, span: Span) -> Self {
        let message = Self::message(error);
        CheckErrorData {
            message: message.0,
            help: message.1,
            source,
            span,
            info: Vec::new(),
        }
    }

    fn info(mut self, source: P<Module>, span: Span, marker: &'static str) -> Self {
        self.info.push(CheckErrorInfoData {
            source,
            span,
            marker,
        });
        self
    }

    fn to_err(self) -> Error {
        Error::Check(self)
    }

    fn message(error: CheckError) -> (&'static str, Option<&'static str>) {
        match error {
            CheckError::SymbolRedefinition => ("error", None),
            CheckError::ProcParamRedefinition => ("error", None),
            CheckError::EnumVariantRedefinition => ("error", None),
            CheckError::StructFieldRedefinition => ("error", None),
        }
    }
}
