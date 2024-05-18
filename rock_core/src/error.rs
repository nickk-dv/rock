use crate::session::FileID;
use crate::text::TextRange;

//@improve how results work and cleanup joining of errors / warnings 18.05.24
// it works but tedious to write
// also warnings when joined in correctly will lose their original order which is important
pub struct DiagnosticCollection {
    errors: Vec<ErrorComp>,
    warnings: Vec<WarningComp>,
}

#[derive(Clone)] //@remove when possible
pub struct ErrorComp(Diagnostic);
#[derive(Clone)] //@remove when possible
pub struct WarningComp(Diagnostic);
pub struct Info;

#[derive(Clone)] //@remove when possible
pub struct Diagnostic {
    message: StringOrStr,
    kind: DiagnosticKind,
}

#[derive(Clone)] //@remove when possible
pub enum DiagnosticKind {
    Message,
    Context {
        main: DiagnosticContext,
        info: Option<DiagnosticContext>,
    },
    ContextVec {
        main: DiagnosticContext,
        info: Vec<DiagnosticContext>,
    },
}

#[derive(Clone)] //@remove when possible
pub struct DiagnosticContext {
    message: StringOrStr,
    source: SourceRange,
}

#[derive(Copy, Clone)]
pub enum DiagnosticSeverity {
    Info,
    Error,
    Warning,
}

#[derive(Copy, Clone)]
pub struct SourceRange {
    range: TextRange,
    file_id: FileID,
}

#[derive(Clone)] //@remove when possible
pub enum StringOrStr {
    String(String),
    Str(&'static str),
}

impl DiagnosticCollection {
    pub fn new() -> DiagnosticCollection {
        DiagnosticCollection {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }
    pub fn errors(&self) -> &[ErrorComp] {
        &self.errors
    }
    pub fn warnings(&self) -> &[WarningComp] {
        &self.warnings
    }

    pub fn error(&mut self, error: ErrorComp) {
        self.errors.push(error);
    }
    pub fn warning(&mut self, warning: WarningComp) {
        self.warnings.push(warning);
    }

    #[must_use]
    pub fn join_errors(mut self, errors: Vec<ErrorComp>) -> DiagnosticCollection {
        self.errors.extend(errors);
        self
    }
    #[must_use]
    pub fn join_warnings(mut self, warnings: Vec<WarningComp>) -> DiagnosticCollection {
        self.warnings.extend(warnings);
        self
    }
    #[must_use]
    pub fn join_collection(mut self, other: DiagnosticCollection) -> DiagnosticCollection {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self
    }

    pub fn result<T>(self, value: T) -> Result<(T, Vec<WarningComp>), DiagnosticCollection> {
        if self.errors.is_empty() {
            Ok((value, self.warnings))
        } else {
            Err(self)
        }
    }
}

impl ErrorComp {
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }

    pub fn message(msg: impl Into<StringOrStr>) -> ErrorComp {
        ErrorComp(Diagnostic::new(msg.into(), DiagnosticKind::Message))
    }

    pub fn new(
        msg: impl Into<StringOrStr>,
        src: SourceRange,
        info: Option<DiagnosticContext>,
    ) -> ErrorComp {
        ErrorComp(Diagnostic::new(
            msg.into(),
            DiagnosticKind::Context {
                main: DiagnosticContext::new("".into(), src),
                info,
            },
        ))
    }

    pub fn new_detailed(
        msg: impl Into<StringOrStr>,
        ctx_msg: impl Into<StringOrStr>,
        src: SourceRange,
        info: Option<DiagnosticContext>,
    ) -> ErrorComp {
        ErrorComp(Diagnostic::new(
            msg.into(),
            DiagnosticKind::Context {
                main: DiagnosticContext::new(ctx_msg.into(), src),
                info,
            },
        ))
    }
}

impl WarningComp {
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }

    pub fn message(msg: impl Into<StringOrStr>) -> WarningComp {
        WarningComp(Diagnostic::new(msg.into(), DiagnosticKind::Message))
    }

    pub fn new(
        msg: impl Into<StringOrStr>,
        src: SourceRange,
        info: Option<DiagnosticContext>,
    ) -> WarningComp {
        WarningComp(Diagnostic::new(
            msg.into(),
            DiagnosticKind::Context {
                main: DiagnosticContext::new("".into(), src),
                info,
            },
        ))
    }

    pub fn new_detailed(
        msg: impl Into<StringOrStr>,
        ctx_msg: impl Into<StringOrStr>,
        src: SourceRange,
        info: Option<DiagnosticContext>,
    ) -> WarningComp {
        WarningComp(Diagnostic::new(
            msg.into(),
            DiagnosticKind::Context {
                main: DiagnosticContext::new(ctx_msg.into(), src),
                info,
            },
        ))
    }
}

impl Info {
    pub fn new(msg: impl Into<StringOrStr>, source: SourceRange) -> Option<DiagnosticContext> {
        Some(DiagnosticContext::new(msg.into(), source))
    }
}

impl Diagnostic {
    fn new(message: StringOrStr, kind: DiagnosticKind) -> Diagnostic {
        Diagnostic { message, kind }
    }
    pub fn message(&self) -> &StringOrStr {
        &self.message
    }
    pub fn kind(&self) -> &DiagnosticKind {
        &self.kind
    }
}

impl DiagnosticContext {
    fn new(message: StringOrStr, source: SourceRange) -> DiagnosticContext {
        DiagnosticContext { message, source }
    }
    pub fn message(&self) -> &str {
        self.message.as_str()
    }
    pub fn source(&self) -> SourceRange {
        self.source
    }
}

impl SourceRange {
    pub fn new(range: TextRange, file_id: FileID) -> SourceRange {
        SourceRange { range, file_id }
    }
    pub fn range(&self) -> TextRange {
        self.range
    }
    pub fn file_id(&self) -> FileID {
        self.file_id
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
