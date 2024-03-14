pub mod ansi;
pub mod format;

use crate::text::TextRange;
use crate::vfs::FileID;

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

#[derive(Copy, Clone, PartialEq)]
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

    pub fn main_message(&self) -> (&str, ErrorSeverity) {
        (&self.message.as_str(), self.severity)
    }

    pub fn context_iter(&self) -> impl Iterator<Item = &ErrorContext> {
        self.context.iter()
    }
}

impl ErrorContext {
    pub fn message(&self) -> &str {
        match &self.message {
            Some(m) => m.as_str(),
            None => "",
        }
    }
    pub fn severity(&self) -> ErrorSeverity {
        self.severity
    }
    pub fn source(&self) -> SourceRange {
        self.source
    }
}

impl ErrorMessage {
    fn as_str(&self) -> &str {
        match self {
            ErrorMessage::String(string) => string,
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
