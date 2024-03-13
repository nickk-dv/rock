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
    message: ErrorMessage,
    severity: ErrorSeverity,
    context: Vec<ErrorContext>,
}

pub struct ErrorContext {
    message: Option<ErrorMessage>,
    severity: ErrorSeverity,
    source: SourceRange,
}

pub enum ErrorMessage {
    String(String),
    Static(&'static str),
}

#[derive(Copy, Clone, PartialEq)] // @partial eq is not needed when new formatted output is made
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
    pub fn error<Message: Into<ErrorMessage>>(message: Message) -> Self {
        Self {
            message: message.into(),
            severity: ErrorSeverity::Error,
            context: Vec::new(),
        }
    }

    pub fn warning<Message: Into<ErrorMessage>>(message: Message) -> Self {
        Self {
            message: message.into(),
            severity: ErrorSeverity::Warning,
            context: Vec::new(),
        }
    }

    pub fn context(mut self, source: SourceRange) -> Self {
        self.push_context(None, self.severity, source);
        self
    }

    pub fn context_msg<Message: Into<ErrorMessage>>(
        mut self,
        message: Message,
        source: SourceRange,
    ) -> Self {
        self.push_context(Some(message.into()), self.severity, source);
        self
    }

    pub fn context_info<Message: Into<ErrorMessage>>(
        mut self,
        message: Message,
        source: SourceRange,
    ) -> Self {
        self.push_context(Some(message.into()), ErrorSeverity::InfoHint, source);
        self
    }

    fn push_context(
        &mut self,
        message: Option<ErrorMessage>,
        severity: ErrorSeverity,
        source: SourceRange,
    ) {
        self.context.push(ErrorContext {
            message,
            severity,
            source,
        });
    }

    pub fn error_main(&self) -> (&ErrorMessage, ErrorSeverity) {
        (&self.message, self.severity)
    }

    pub fn error_context_iter(&self) -> impl Iterator<Item = &ErrorContext> {
        self.context.iter()
    }
}

impl ErrorContext {
    pub fn message(&self) -> Option<&ErrorMessage> {
        self.message.as_ref()
    }
    pub fn severity(&self) -> ErrorSeverity {
        self.severity
    }
    pub fn source(&self) -> SourceRange {
        self.source
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
        let (message, severity) = error.error_main();
        eprintln!(
            "\n{}{}:{} {}{}{}",
            ansi::RED_BOLD,
            severity.as_str(),
            ansi::CLEAR,
            ansi::WHITE_BOLD,
            message.as_str(),
            ansi::CLEAR,
        );
        for context in error.error_context_iter() {
            range_fmt::print_simple(
                ctx.file(context.source().file_id()),
                context.source().range(),
                context.message().map(|m| m.as_str()),
                context.severity == ErrorSeverity::InfoHint,
            );
        }
    }
}
