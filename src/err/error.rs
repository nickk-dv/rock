use super::message::Message;
use crate::ast::parse_error::ParseErrorData;
use crate::ast::span::*;
use crate::ast::token::*;
use crate::ast::FileID;

#[allow(private_interfaces)]
pub enum Error {
    Parse(ParseErrorData),
    Check(CheckErrorData),
    FileIO(FileIOErrorData),
    Internal(InternalErrorData),
}

pub struct CheckErrorData {
    pub(super) message: Message,
    pub(super) no_source: bool,
    pub(super) file_id: FileID,
    pub(super) span: Span,
    pub(super) info: Vec<CheckErrorInfo>,
}

pub(super) enum CheckErrorInfo {
    InfoString(String),
    Context(CheckErrorContext),
}

pub(super) struct CheckErrorContext {
    pub(super) marker: &'static str,
    pub(super) file_id: FileID,
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
    DeclMatchKw,
    ForAssignOp,
    ImportTargetMatch,
    ElseMatch,
    PrimaryExprMatch,
    LiteralMatch,
    LiteralInteger,
    LiteralFloat,
    FieldInit,
    ExpectToken(ParseContext, Token),
}

#[derive(Copy, Clone)]
pub enum ParseContext {
    ModulePath,
    Type,
    UnitType,
    CustomType,
    ArraySlice,
    ArrayStatic,
    Decl,
    ModDecl,
    ProcDecl,
    ProcParam,
    EnumDecl,
    EnumVariant,
    UnionDecl,
    UnionMember,
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
    Match,
    MatchArm,
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

    StructInitGotEnumType,

    DeferNested,
    BreakOutsideLoop,
    ContinueOutsideLoop,
    UnreachableStatement,

    VarLocalAlreadyDeclared,
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
    pub fn parse(data: ParseErrorData) -> Self {
        Self::Parse(data)
    }

    pub fn check(error: CheckError, file_id: FileID, span: Span) -> CheckErrorData {
        CheckErrorData::new(error, false, file_id, span)
    }

    pub fn check_no_src(error: CheckError) -> Self {
        Self::Check(CheckErrorData::new(error, true, FileID(0), Span::new(0, 0)))
    }

    pub fn file_io(error: FileIOError) -> FileIOErrorData {
        FileIOErrorData::new(error)
    }

    pub fn internal(error: InternalError) -> InternalErrorData {
        InternalErrorData::new(error)
    }
}

#[macro_export]
macro_rules! error_check {
    ($err:expr, $span:expr, $file_id:expr $(=> $marker:expr, $span_ctx:expr, $file_id_ctx:expr)* ) => {{
        let check_error: Error = CheckErrorData::new($err, false, $file_id, $span)
        $(.context($marker, $file_id_ctx, $span_ctx))* .into();
        check_error
    }};
}

impl CheckErrorData {
    fn new(error: CheckError, no_source: bool, file_id: FileID, span: Span) -> Self {
        CheckErrorData {
            message: error.into(),
            no_source,
            file_id,
            span,
            info: Vec::new(),
        }
    }

    pub fn context(mut self, marker: &'static str, file_id: FileID, span: Span) -> Self {
        self.info.push(CheckErrorInfo::Context(CheckErrorContext {
            marker,
            file_id,
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
