use super::message::Message;
use crate::ast::ast::Module;
use crate::ast::span::*;
use crate::ast::token::*;
use crate::mem::P;

#[allow(private_interfaces)]
pub enum Error {
    Parse(ParseErrorData),
    Check(CheckErrorData),
    FileIO(FileIOErrorData),
    Internal(InternalErrorData),
}

pub(super) struct ParseErrorData {
    pub(super) source: P<Module>,
    pub(super) context: ParseContext,
    pub(super) expected: Vec<Token>,
    pub(super) got_token: TokenSpan,
}

pub struct CheckErrorData {
    pub(super) message: Message,
    pub(super) no_source: bool,
    pub(super) source: P<Module>,
    pub(super) span: Span,
    pub(super) info: Vec<CheckErrorInfo>,
}

pub(super) enum CheckErrorInfo {
    InfoString(String),
    Context(CheckErrorContext),
}

pub(super) struct CheckErrorContext {
    pub(super) marker: &'static str,
    pub(super) source: P<Module>,
    pub(super) span: Span,
}

pub struct FileIOErrorData {
    pub(super) message: Message,
    pub(super) info: Vec<String>,
}

pub struct InternalErrorData {
    pub(super) message: Message,
    pub(super) info: Vec<String>,
}

#[derive(Copy, Clone)]
pub enum ParseError {
    Ident(ParseContext),
    TypeMatch,
    DeclMatch,
    ImportTargetMatch,
    StmtMatch,
    ElseMatch,
    PrimaryExprIdent,
    PrimaryExprMatch,
    AccessMatch,
    LiteralMatch,
    LiteralInteger,
    LiteralFloat,
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
    Else,
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
    LiteralInteger,
    LiteralFloat,
    ProcCall,
    ArrayInit,
    StructInit,
}

pub enum CheckError {
    ParseLibFileMissing,
    ParseMainFileMissing,
    ParseModBothPathsExist,
    ParseModBothPathsMissing,
    ParseModCycle,

    RedefinitionMod,
    RedefinitionProc,
    RedefinitionType,
    RedefinitionGlobal,

    ProcParamRedefinition,
    EnumVariantRedefinition,
    StructFieldRedefinition,

    MainProcMissing,
    MainProcVariadic,
    MainProcExternal,
    MainProcHasParams,
    MainProcWrongRetType,

    SuperUsedFromRootModule,
    ModuleFileReportedMissing,
    ModuleIsPrivate,
    ImportFromItself,
    ImportItself,
    ImportGlobExists,
    ImportSymbolNotDefined,
    ImportSymbolAlreadyImported,

    ModuleNotDeclaredInPath,
    ProcNotDeclaredInPath,
    TypeNotDeclaredInPath,
    GlobalNotDeclaredInPath,

    ProcIsPrivate,
    TypeIsPrivate,
    GlobalIsPrivate,

    ModuleNotFoundInScope,
    ProcNotFoundInScope,
    TypeNotFoundInScope,
    GlobalNotFoundInScope,
    ModuleSymbolConflict,
    ProcSymbolConflict,
    TypeSymbolConflict,
    GlobalSymbolConflict,

    DeferNested,
    BreakOutsideLoop,
    ContinueOutsideLoop,
    UnreachableStatement,
}

pub enum FileIOError {
    DirRead,
    DirCreate,
    FileRead,
    FileCreate,
    FileWrite,
    EnvCommand,
    EnvCurrentDir,
}

pub enum InternalError {}

impl Error {
    pub fn parse(error: ParseError, source: P<Module>, got_token: TokenSpan) -> Self {
        Self::Parse(ParseErrorData::new(error, source, got_token))
    }

    pub fn check(error: CheckError, source: P<Module>, span: Span) -> CheckErrorData {
        CheckErrorData::new(error, false, source, span)
    }

    pub fn check_no_src(error: CheckError) -> Self {
        Self::Check(CheckErrorData::new(error, true, P::null(), Span::new(0, 0)))
    }

    pub fn file_io(error: FileIOError) -> FileIOErrorData {
        FileIOErrorData::new(error)
    }

    pub fn internal(error: InternalError) -> InternalErrorData {
        InternalErrorData::new(error)
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
            ParseError::ElseMatch => ParseContext::Else,
            ParseError::PrimaryExprIdent => ParseContext::Expr,
            ParseError::PrimaryExprMatch => ParseContext::Expr,
            ParseError::AccessMatch => ParseContext::Access,
            ParseError::LiteralMatch => ParseContext::Literal,
            ParseError::LiteralInteger => ParseContext::LiteralInteger,
            ParseError::LiteralFloat => ParseContext::LiteralFloat,
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
            ParseError::ElseMatch => vec![Token::KwIf, Token::OpenBlock],
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
            ParseError::LiteralInteger => Self::all_integer_literal_types(),
            ParseError::LiteralFloat => Self::all_float_literal_types(),
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

    fn all_integer_literal_types() -> Vec<Token> {
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

    fn all_float_literal_types() -> Vec<Token> {
        vec![Token::KwF32, Token::KwF64]
    }
}

impl ParseContext {
    pub(super) fn as_str(&self) -> &'static str {
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
            ParseContext::Else => "else statement",
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
            ParseContext::LiteralInteger => "integer literal",
            ParseContext::LiteralFloat => "float literal",
            ParseContext::ProcCall => "procedure call",
            ParseContext::ArrayInit => "array initializer",
            ParseContext::StructInit => "struct initializer",
        }
    }
}

impl CheckErrorData {
    fn new(error: CheckError, no_source: bool, source: P<Module>, span: Span) -> Self {
        CheckErrorData {
            message: error.into(),
            no_source,
            source,
            span,
            info: Vec::new(),
        }
    }

    pub fn context(mut self, marker: &'static str, source: P<Module>, span: Span) -> Self {
        self.info.push(CheckErrorInfo::Context(CheckErrorContext {
            marker,
            source,
            span,
        }));
        self
    }

    pub fn info(mut self, info: String) -> Self {
        self.info.push(CheckErrorInfo::InfoString(info));
        self
    }
}

impl Into<Error> for CheckErrorData {
    fn into(self) -> Error {
        Error::Check(self)
    }
}

impl Into<Error> for FileIOErrorData {
    fn into(self) -> Error {
        Error::FileIO(self)
    }
}

impl Into<Error> for InternalErrorData {
    fn into(self) -> Error {
        Error::Internal(self)
    }
}

impl FileIOErrorData {
    fn new(error: FileIOError) -> Self {
        Self {
            message: error.into(),
            info: Vec::new(),
        }
    }

    pub fn info(mut self, info: String) -> Self {
        self.info.push(info);
        self
    }
}

impl InternalErrorData {
    fn new(error: InternalError) -> Self {
        Self {
            message: error.into(),
            info: Vec::new(),
        }
    }

    pub fn info(mut self, info: String) -> Self {
        self.info.push(info);
        self
    }
}
