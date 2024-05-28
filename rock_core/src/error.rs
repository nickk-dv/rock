use crate::session::FileID;
use crate::text::TextRange;

pub enum ResultComp<T> {
    Ok((T, Vec<WarningComp>)),
    Err(DiagnosticCollection),
}

pub struct DiagnosticCollection {
    errors: Vec<ErrorComp>,
    warnings: Vec<WarningComp>,
}

pub struct ErrorComp(Diagnostic);
pub struct WarningComp(Diagnostic);
pub struct Info;

pub struct Diagnostic {
    message: StringOrStr,
    kind: DiagnosticKind,
}

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

pub enum StringOrStr {
    String(String),
    Str(&'static str),
}

impl<T> ResultComp<T> {
    pub fn new(value: T, diagnostics: DiagnosticCollection) -> ResultComp<T> {
        if diagnostics.errors.is_empty() {
            ResultComp::Ok((value, diagnostics.warnings))
        } else {
            ResultComp::Err(diagnostics)
        }
    }

    pub fn from_error(result: Result<T, ErrorComp>) -> ResultComp<T> {
        match result {
            Ok(value) => ResultComp::Ok((value, vec![])),
            Err(error) => ResultComp::Err(DiagnosticCollection::new().join_errors(vec![error])),
        }
    }

    pub fn from_errors(result: Result<T, Vec<ErrorComp>>) -> ResultComp<T> {
        match result {
            Ok(value) => ResultComp::Ok((value, vec![])),
            Err(errors) => ResultComp::Err(DiagnosticCollection::new().join_errors(errors)),
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, ResultComp::Ok(..))
    }

    pub fn is_err(&self) -> bool {
        matches!(self, ResultComp::Err(..))
    }

    pub fn into_result(
        self,
        mut warnings_prev: Vec<WarningComp>,
    ) -> Result<(T, Vec<WarningComp>), DiagnosticCollection> {
        match self {
            ResultComp::Ok((value, warnings)) => {
                if warnings_prev.is_empty() {
                    Ok((value, warnings))
                } else {
                    warnings_prev.extend(warnings);
                    Ok((value, warnings_prev))
                }
            }
            ResultComp::Err(mut diagnostics) => {
                if warnings_prev.is_empty() {
                    Err(diagnostics)
                } else {
                    warnings_prev.extend(diagnostics.warnings);
                    diagnostics.warnings = warnings_prev;
                    Err(diagnostics)
                }
            }
        }
    }
}

impl DiagnosticCollection {
    pub fn new() -> DiagnosticCollection {
        DiagnosticCollection {
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn from_result(
        result: Result<Vec<WarningComp>, DiagnosticCollection>,
    ) -> DiagnosticCollection {
        match result {
            Ok(warnings) => DiagnosticCollection::new().join_warnings(warnings),
            Err(diagnostics) => diagnostics,
        }
    }

    pub fn errors(&self) -> &[ErrorComp] {
        &self.errors
    }
    pub fn warnings(&self) -> &[WarningComp] {
        &self.warnings
    }
    pub fn warnings_moveout(self) -> Vec<WarningComp> {
        self.warnings
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
