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
    Context {
        main: DiagnosticContext,
        info: Option<Info>,
    },
    ContextVec {
        main: DiagnosticContext,
        info_vec: Vec<Info>,
    },
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
        let data = DiagnosticData::Context {
            main: DiagnosticContext::new_empty(src),
            info,
        };
        Error(Diagnostic::new(msg, data))
    }
    pub fn new_info_vec(
        msg: impl Into<StringOrStr>,
        ctx_msg: impl Into<StringOrStr>,
        src: SourceRange,
        info_vec: Vec<Info>,
    ) -> Error {
        let data = DiagnosticData::ContextVec {
            main: DiagnosticContext::new(ctx_msg, src),
            info_vec,
        };
        Error(Diagnostic::new(msg, data))
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }
}

impl Warning {
    pub fn new(msg: impl Into<StringOrStr>, src: SourceRange, info: Option<Info>) -> Warning {
        let data = DiagnosticData::Context {
            main: DiagnosticContext::new_empty(src),
            info,
        };
        Warning(Diagnostic::new(msg, data))
    }

    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }
}

impl Diagnostic {
    fn new(msg: impl Into<StringOrStr>, data: DiagnosticData) -> Diagnostic {
        Diagnostic {
            msg: msg.into(),
            data,
        }
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
        DiagnosticContext {
            msg: msg.into(),
            src,
        }
    }
    fn new_empty(src: SourceRange) -> DiagnosticContext {
        DiagnosticContext {
            msg: StringOrStr::Str(""),
            src,
        }
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

//==================== COLLECTION BUFFERS ====================

#[derive(Default)]
pub struct ErrorBuffer {
    collected: bool,
    errors: Vec<Error>,
}

#[derive(Default)]
pub struct WarningBuffer {
    collected: bool,
    warnings: Vec<Warning>,
}

#[derive(Default)]
pub struct ErrorWarningBuffer {
    collected: bool,
    errors: Vec<Error>,
    warnings: Vec<Warning>,
}

impl ErrorBuffer {
    pub fn result<T>(self, value: T) -> Result<T, ErrorBuffer> {
        if self.errors.is_empty() {
            let _ = self.collect();
            Ok(value)
        } else {
            Err(self)
        }
    }

    pub fn collect(mut self) -> Vec<Error> {
        self.collected = true;
        std::mem::take(&mut self.errors)
    }

    pub fn join_e(&mut self, err: ErrorBuffer) {
        let errors = err.collect();
        self.errors.extend(errors);
    }
}

impl WarningBuffer {
    pub fn collect(mut self) -> Vec<Warning> {
        self.collected = true;
        std::mem::take(&mut self.warnings)
    }

    pub fn join_w(&mut self, warn: WarningBuffer) {
        let warnings = warn.collect();
        self.warnings.extend(warnings);
    }
}

impl ErrorWarningBuffer {
    pub fn result<T>(self, value: T) -> Result<(T, WarningBuffer), ErrorWarningBuffer> {
        if self.errors.is_empty() {
            let (_, warnings) = self.collect();
            let mut warn = WarningBuffer::default();
            warn.warnings = warnings;
            Ok((value, warn))
        } else {
            Err(self)
        }
    }

    pub fn collect(mut self) -> (Vec<Error>, Vec<Warning>) {
        self.collected = true;
        let errors = std::mem::take(&mut self.errors);
        let warnings = std::mem::take(&mut self.warnings);
        (errors, warnings)
    }

    pub fn join_e(&mut self, err: ErrorBuffer) {
        let errors = err.collect();
        self.errors.extend(errors);
    }
    pub fn join_w(&mut self, warn: WarningBuffer) {
        let warnings = warn.collect();
        self.warnings.extend(warnings);
    }
    pub fn join_ew(&mut self, errw: ErrorWarningBuffer) {
        let (errors, warnings) = errw.collect();
        self.errors.extend(errors);
        self.warnings.extend(warnings);
    }
}

impl From<Error> for ErrorBuffer {
    fn from(error: Error) -> ErrorBuffer {
        let mut err = ErrorBuffer::default();
        err.error(error);
        err
    }
}

impl From<Error> for ErrorWarningBuffer {
    fn from(error: Error) -> ErrorWarningBuffer {
        let mut errw = ErrorWarningBuffer::default();
        errw.error(error);
        errw
    }
}

impl From<ErrorBuffer> for ErrorWarningBuffer {
    fn from(err: ErrorBuffer) -> ErrorWarningBuffer {
        let mut errw = ErrorWarningBuffer::default();
        errw.errors = err.collect();
        errw
    }
}

impl Drop for ErrorBuffer {
    fn drop(&mut self) {
        if !self.collected {
            panic!(
                "internal: ErrorBuffer was not collected! E:{}",
                self.errors.len(),
            );
        }
    }
}
impl Drop for WarningBuffer {
    fn drop(&mut self) {
        if !self.collected {
            panic!(
                "internal: WarningBuffer was not collected! W:{}",
                self.warnings.len(),
            );
        }
    }
}
impl Drop for ErrorWarningBuffer {
    fn drop(&mut self) {
        if !self.collected {
            panic!(
                "internal: ErrorWarningBuffer was not collected! E:{} W:{}",
                self.errors.len(),
                self.warnings.len(),
            );
        }
    }
}

//==================== COLLECTION TRAITS ====================

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
