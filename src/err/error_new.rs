use crate::{
    ast::{CompCtx, FileID},
    err::{ansi, range_fmt},
    text_range::TextRange,
};

// @reduce invariance of main error message
// separate root message from contexts with sources
// handle relation of severity across the message,
// usually / context match the root severity but hints are always possible
// and maybe add optional note at the bottom of the error

pub struct ErrorComp {
    inner: Vec<ErrorContext>,
}

pub struct ErrorContext {
    message: ErrorMessage,
    severity: ErrorSeverity,
    source: Option<SourceRange>,
}

pub enum ErrorMessage {
    String(String),
    Static(&'static str),
}

#[derive(Copy, Clone)]
pub enum ErrorSeverity {
    Error,
    Warning,
    InfoHint,
}

#[derive(Copy, Clone)]
pub struct SourceRange {
    range: TextRange,
    file_id: FileID,
}

impl ErrorComp {
    pub fn new(message: ErrorMessage, severity: ErrorSeverity, source: SourceRange) -> Self {
        Self {
            inner: vec![ErrorContext {
                message,
                severity,
                source: Some(source),
            }],
        }
    }

    pub fn context(
        mut self,
        message: ErrorMessage,
        severity: ErrorSeverity,
        source: Option<SourceRange>,
    ) -> Self {
        self.add_context(message, severity, source);
        self
    }

    pub fn add_context(
        &mut self,
        message: ErrorMessage,
        severity: ErrorSeverity,
        source: Option<SourceRange>,
    ) {
        self.inner.push(ErrorContext {
            message,
            severity,
            source,
        });
    }

    pub fn main_context(&self) -> &ErrorContext {
        self.inner.get(0).unwrap()
    }

    pub fn other_context(&self) -> impl Iterator<Item = &ErrorContext> {
        self.inner.iter().skip(1)
    }
}

impl ErrorMessage {
    pub fn as_str(&self) -> &str {
        match self {
            ErrorMessage::String(string) => &string,
            ErrorMessage::Static(string) => string,
        }
    }
}

impl From<String> for ErrorMessage {
    fn from(value: String) -> Self {
        ErrorMessage::String(value)
    }
}

impl From<&'static str> for ErrorMessage {
    fn from(value: &'static str) -> Self {
        ErrorMessage::Static(value)
    }
}

impl ErrorSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorSeverity::Error => "error",
            ErrorSeverity::Warning => "warning",
            ErrorSeverity::InfoHint => "hint",
        }
    }
}

impl ErrorContext {
    pub fn message(&self) -> &ErrorMessage {
        &self.message
    }
    pub fn severity(&self) -> ErrorSeverity {
        self.severity
    }
    pub fn source(&self) -> Option<SourceRange> {
        self.source
    }
}

impl SourceRange {
    pub fn new(range: TextRange, file_id: FileID) -> Self {
        Self { range, file_id }
    }
    pub fn range(&self) -> TextRange {
        self.range
    }
    pub fn file_id(&self) -> FileID {
        self.file_id
    }
}

pub fn report_check_errors_cli(ctx: &CompCtx, errors: &[ErrorComp]) {
    for error in errors {
        let main_context = error.main_context();
        eprintln!(
            "\n{}{}:{} {}{}{}",
            ansi::RED_BOLD,
            main_context.severity().as_str(),
            ansi::CLEAR,
            ansi::WHITE_BOLD,
            main_context.message.as_str(),
            ansi::CLEAR,
        );
        range_fmt::print_simple(
            ctx.file(main_context.source().unwrap().file_id()),
            main_context.source().unwrap().range(),
            None,
            false,
        );

        for context in error.other_context() {
            match context.source() {
                Some(source) => {
                    range_fmt::print_simple(
                        ctx.file(source.file_id()),
                        source.range(),
                        Some(context.message().as_str()),
                        true,
                    );
                }
                None => {
                    eprintln!("{}", context.message().as_str());
                }
            }
        }
    }
}
