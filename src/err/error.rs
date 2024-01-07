use crate::{
    ast::{
        ast::Module,
        span::Span,
        token::{Token, TokenSpan},
    },
    mem::P,
};

pub enum Error {
    Parse(ParseErrorData),
    Check(CheckErrorData),
    FileIO(FileIOErrorData),
    Internal(InternalErrorData),
}

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

    pub fn file_io(error: FileIOError) -> Self {
        Self::FileIO(FileIOErrorData::from(error))
    }

    pub fn internal(error: InternalError) -> Self {
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
pub enum ParseError {
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

pub enum CheckError {
    ParseSrcDirMissing,
    ParseLibFileMissing,
    ParseMainFileMissing,
    ParseModBothPathsExist,
    ParseModBothPathsMissing,
    ParseModCycle,

    ModRedefinition,
    SymbolRedefinition,
    ProcParamRedefinition,
    EnumVariantRedefinition,
    StructFieldRedefinition,

    MainProcMissing,
    MainProcVariadic,
    MainProcExternal,
    MainProcHasParams,
    MainProcWrongRetType,

    ImportModuleAccessMissing,
    SuperUsedFromRootModule,
    ModuleIsPrivate,
    ModuleNotFoundInScope,
    ModuleNotDeclaredInPath,
    ImportFromItself,
    ImportItself,
    ImportWildcardExists,
    ImportSymbolNotDefined,
    ImportSymbolIsPrivate,
    ImportSymbolAlreadyDefined,
    ImporySymbolAlreadyImported,

    ModuleSymbolConflit,
}

pub(super) struct ParseErrorData {
    pub(super) source: P<Module>,
    pub(super) context: ParseContext,
    pub(super) expected: Vec<Token>,
    pub(super) got_token: TokenSpan,
}

pub(super) struct CheckErrorData {
    pub(super) message: &'static str,
    pub(super) help: Option<&'static str>,
    pub(super) no_source: bool,
    pub(super) source: P<Module>,
    pub(super) span: Span,
    pub(super) info: Vec<CheckErrorInfoData>,
}

pub(super) struct CheckErrorInfoData {
    pub(super) source: P<Module>,
    pub(super) span: Span,
    pub(super) marker: &'static str,
}

pub(super) struct FileIOErrorData {
    pub(super) message: &'static str,
    pub(super) help: Option<&'static str>,
}

pub(super) struct InternalErrorData {
    pub(super) message: &'static str,
    pub(super) help: Option<&'static str>,
}

pub enum FileIOError {
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
            FileIOError::DirRead    => Self::new("file io error", None),
            FileIOError::DirCreate  => Self::new("file io error", None),
            FileIOError::FileRead   => Self::new("file io error", None),
            FileIOError::FileCreate => Self::new("file io error", None),
            FileIOError::FileWrite  => Self::new("file io error", None),
        }
    }
}
pub enum InternalError {
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
    fn new(error: CheckError, no_source: bool, source: P<Module>, span: Span) -> Self {
        let message = Self::message(error);
        CheckErrorData {
            message: message.0,
            help: message.1,
            no_source,
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

    pub fn to_err(self) -> Error {
        Error::Check(self)
    }

    fn message(error: CheckError) -> (&'static str, Option<&'static str>) {
        match error {
            CheckError::ParseSrcDirMissing =>          ("missing `src` directory", Some("make sure that current directory is set to the project directory before running compiler commands")),
            CheckError::ParseLibFileMissing =>         ("missing `src/lib.lang` file", Some("the root module `lib.lang` of library package must exist")), //@unstable file ext .lang
            CheckError::ParseMainFileMissing =>        ("missing `src/main.lang` file", Some("the root module `main.lang` of executable package must exist")), //@unstable file ext .lang
            CheckError::ParseModBothPathsExist =>      ("both module filepaths exist", Some("only one filepath may exist:")),
            CheckError::ParseModBothPathsMissing =>    ("both module filepaths are missing", Some("at least one filepath must exist:")),
            CheckError::ParseModCycle =>               ("module definition results in a cycle", Some("module paths that form a cycle:")),

            CheckError::ModRedefinition =>             ("module redefinition", None),
            CheckError::SymbolRedefinition =>          ("symbol redefinition", None),
            CheckError::ProcParamRedefinition =>       ("procedure parameter redefinition", None),
            CheckError::EnumVariantRedefinition =>     ("enum variant redefinition", None),
            CheckError::StructFieldRedefinition =>     ("struct field redefinition", None),
            
            CheckError::MainProcMissing =>             ("main procedure is not found in src/main.lang", Some("define the entry point `main :: () -> s32 { return 0; }`")), //@unstable file ext .lang
            CheckError::MainProcVariadic =>            ("main procedure cannot be variadic", Some("remove `..` from input parameters")),
            CheckError::MainProcExternal =>            ("main procedure cannot be external", Some("remove `c_call` directive")), //@unstable directive name
            CheckError::MainProcHasParams =>           ("main procedure cannot have input parameters", Some("remove input parameters")),
            CheckError::MainProcWrongRetType =>        ("main procedure must return `s32`", Some("change return type to `-> s32`")),
            
            CheckError::ImportModuleAccessMissing =>   ("import missing module access path", Some("specify module access path before the import target")),
            CheckError::SuperUsedFromRootModule =>     ("using `super` in the root module", Some("`super` refers to the parent module, which doesnt exist for the root module")),
            CheckError::ModuleIsPrivate =>             ("module is private", None),
            CheckError::ModuleNotFoundInScope =>       ("module is not found in this scope", None),
            CheckError::ModuleNotDeclaredInPath =>     ("module is not declared in referenced module path", None),
            CheckError::ImportFromItself =>            ("importing from itself is redundant", Some("remove this import")),
            CheckError::ImportItself =>                ("importing module into itself is redundant", Some("remove this import")),
            CheckError::ImportWildcardExists =>        ("wildcard import of module already exists", Some("remove this import")),
            CheckError::ImportSymbolNotDefined =>      ("imported symbol is not defined in target module", None),
            CheckError::ImportSymbolIsPrivate =>       ("imported symbol is private", Some("cannot import symbols declared without `pub` keyword")),
            CheckError::ImportSymbolAlreadyDefined =>  ("imported symbol is already defined", None),
            CheckError::ImporySymbolAlreadyImported => ("imported symbol is already imported", Some("remove this symbol import")),

            CheckError::ModuleSymbolConflit =>         ("this module name conflits with others in scope", None), 
        }
    }
}
