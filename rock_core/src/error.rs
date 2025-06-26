use crate::session::ModuleID;
use crate::text::TextRange;

pub struct Error(Diagnostic);
pub struct Warning(Diagnostic);
pub struct Info(DiagnosticContext);

pub struct Diagnostic {
    msg: StringOrStr,
    data: DiagnosticData,
}

pub enum DiagnosticData {
    Message,
    Context { main: DiagnosticContext, info: Option<Info> },
    ContextVec { main: DiagnosticContext, info_vec: Vec<Info> },
}

pub struct DiagnosticContext {
    msg: StringOrStr,
    src: SourceRange,
}

pub enum StringOrStr {
    String(String),
    Str(&'static str),
}

#[derive(Copy, Clone)]
pub struct SourceRange {
    range: TextRange,
    module_id: ModuleID,
}

#[derive(Copy, Clone)]
pub enum Severity {
    Info,
    Error,
    Warning,
}

impl Info {
    pub fn new(msg: impl Into<StringOrStr>, src: SourceRange) -> Option<Info> {
        Some(Info(DiagnosticContext::new(msg, src)))
    }
    pub fn new_val(msg: impl Into<StringOrStr>, src: SourceRange) -> Info {
        Info(DiagnosticContext::new(msg, src))
    }

    pub fn context(&self) -> &DiagnosticContext {
        &self.0
    }
}

impl Error {
    pub fn message(msg: impl Into<StringOrStr>) -> Error {
        let data = DiagnosticData::Message;
        Error(Diagnostic::new(msg, data))
    }
    pub fn new(msg: impl Into<StringOrStr>, src: SourceRange, info: Option<Info>) -> Error {
        let data = DiagnosticData::Context { main: DiagnosticContext::new_empty(src), info };
        Error(Diagnostic::new(msg, data))
    }
    pub fn new_info_vec(
        msg: impl Into<StringOrStr>,
        ctx_msg: impl Into<StringOrStr>,
        src: SourceRange,
        info_vec: Vec<Info>,
    ) -> Error {
        let data =
            DiagnosticData::ContextVec { main: DiagnosticContext::new(ctx_msg, src), info_vec };
        Error(Diagnostic::new(msg, data))
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }
}

impl Warning {
    pub fn new(msg: impl Into<StringOrStr>, src: SourceRange, info: Option<Info>) -> Warning {
        let data = DiagnosticData::Context { main: DiagnosticContext::new_empty(src), info };
        Warning(Diagnostic::new(msg, data))
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }
}

impl Diagnostic {
    fn new(msg: impl Into<StringOrStr>, data: DiagnosticData) -> Diagnostic {
        Diagnostic { msg: msg.into(), data }
    }

    pub fn msg(&self) -> &StringOrStr {
        &self.msg
    }
    pub fn data(&self) -> &DiagnosticData {
        &self.data
    }
}

impl DiagnosticContext {
    fn new(msg: impl Into<StringOrStr>, src: SourceRange) -> DiagnosticContext {
        DiagnosticContext { msg: msg.into(), src }
    }
    fn new_empty(src: SourceRange) -> DiagnosticContext {
        DiagnosticContext { msg: StringOrStr::Str(""), src }
    }

    pub fn msg(&self) -> &str {
        self.msg.as_str()
    }
    pub fn src(&self) -> SourceRange {
        self.src
    }
}

impl SourceRange {
    pub fn new(module_id: ModuleID, range: TextRange) -> SourceRange {
        SourceRange { range, module_id }
    }
    pub fn range(&self) -> TextRange {
        self.range
    }
    pub fn module_id(&self) -> ModuleID {
        self.module_id
    }
}

impl StringOrStr {
    pub fn as_str(&self) -> &str {
        match self {
            StringOrStr::Str(string) => string,
            StringOrStr::String(string) => string,
        }
    }
}

impl From<&'static str> for StringOrStr {
    fn from(value: &'static str) -> StringOrStr {
        StringOrStr::Str(value)
    }
}

impl From<String> for StringOrStr {
    fn from(value: String) -> StringOrStr {
        StringOrStr::String(value)
    }
}

#[derive(Default)]
pub struct ErrorBuffer {
    pub errors: Vec<Error>,
}

#[derive(Default)]
pub struct WarningBuffer {
    pub warnings: Vec<Warning>,
}

#[derive(Default)]
pub struct ErrorWarningBuffer {
    pub errors: Vec<Error>,
    pub warnings: Vec<Warning>,
}

impl ErrorBuffer {
    pub fn collect(self) -> Vec<Error> {
        self.errors
    }
    pub fn clear(&mut self) {
        self.errors.clear();
    }
    pub fn join_e(&mut self, err: ErrorBuffer) {
        let errors = err.collect();
        self.errors.extend(errors);
    }
    pub fn result<T>(self, value: T) -> Result<T, ErrorBuffer> {
        if self.errors.is_empty() {
            Ok(value)
        } else {
            Err(self)
        }
    }
}

impl ErrorWarningBuffer {
    pub fn collect(self) -> (Vec<Error>, Vec<Warning>) {
        (self.errors, self.warnings)
    }
    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }
}

pub trait ErrorSink {
    fn error(&mut self, error: Error);
    fn error_count(&self) -> usize;

    fn did_error(&self, prev: usize) -> bool {
        prev < self.error_count()
    }
}
pub trait WarningSink {
    fn warning(&mut self, warning: Warning);
    fn warning_count(&self) -> usize;
}

impl ErrorSink for ErrorBuffer {
    fn error(&mut self, error: Error) {
        self.errors.push(error);
    }
    fn error_count(&self) -> usize {
        self.errors.len()
    }
}
impl WarningSink for WarningBuffer {
    fn warning(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }
    fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}
impl ErrorSink for ErrorWarningBuffer {
    fn error(&mut self, error: Error) {
        self.errors.push(error);
    }
    fn error_count(&self) -> usize {
        self.errors.len()
    }
}
impl WarningSink for ErrorWarningBuffer {
    fn warning(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }
    fn warning_count(&self) -> usize {
        self.warnings.len()
    }
}
