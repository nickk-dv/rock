use crate::session::FileID;
use crate::text::TextRange;

pub struct ErrorComp {
    message: ErrorMessage,
    severity: ErrorSeverity,
    data: ErrorData,
}

pub enum ErrorMessage {
    Str(&'static str),
    String(String),
}

#[derive(Copy, Clone, PartialEq)]
pub enum ErrorSeverity {
    Info,
    Error,
    Warning,
}

#[derive(Copy, Clone)]
pub struct SourceRange {
    range: TextRange,
    file_id: FileID,
}

pub struct ErrorContext {
    message: ErrorMessage,
    source: SourceRange,
}

pub enum ErrorData {
    None,
    Context {
        main: ErrorContext,
        info: Option<ErrorContext>,
    },
}

impl ErrorComp {
    pub fn message(msg: impl Into<ErrorMessage>) -> ErrorComp {
        ErrorComp {
            message: msg.into(),
            severity: ErrorSeverity::Error,
            data: ErrorData::None,
        }
    }

    pub fn info(msg: impl Into<ErrorMessage>, src: SourceRange) -> Option<ErrorContext> {
        Some(ErrorContext {
            message: msg.into(),
            source: src,
        })
    }

    pub fn error(
        msg: impl Into<ErrorMessage>,
        src: SourceRange,
        info: Option<ErrorContext>,
    ) -> ErrorComp {
        ErrorComp::new_internal(ErrorSeverity::Error, msg, None::<&str>, src, info)
    }

    pub fn error_detailed(
        msg: impl Into<ErrorMessage>,
        src_msg: impl Into<ErrorMessage>,
        src: SourceRange,
        info: Option<ErrorContext>,
    ) -> ErrorComp {
        ErrorComp::new_internal(ErrorSeverity::Error, msg, Some(src_msg), src, info)
    }

    pub fn warning(
        msg: impl Into<ErrorMessage>,
        src: SourceRange,
        info: Option<ErrorContext>,
    ) -> ErrorComp {
        ErrorComp::new_internal(ErrorSeverity::Warning, msg, None::<&str>, src, info)
    }

    pub fn warning_detailed(
        msg: impl Into<ErrorMessage>,
        src_msg: impl Into<ErrorMessage>,
        src: SourceRange,
        info: Option<ErrorContext>,
    ) -> ErrorComp {
        ErrorComp::new_internal(ErrorSeverity::Warning, msg, Some(src_msg), src, info)
    }

    fn new_internal(
        severity: ErrorSeverity,
        msg: impl Into<ErrorMessage>,
        src_msg: Option<impl Into<ErrorMessage>>,
        src: SourceRange,
        info: Option<ErrorContext>,
    ) -> ErrorComp {
        let main_ctx = ErrorContext {
            message: src_msg.map(|m| m.into()).unwrap_or_else(|| "".into()),
            source: src,
        };
        ErrorComp {
            message: msg.into(),
            severity,
            data: ErrorData::Context {
                main: main_ctx,
                info,
            },
        }
    }

    pub fn get_message(&self) -> &str {
        self.message.as_str()
    }
    pub fn get_severity(&self) -> ErrorSeverity {
        self.severity
    }
    pub fn get_data(&self) -> &ErrorData {
        &self.data
    }
}

impl ErrorMessage {
    fn as_str(&self) -> &str {
        match self {
            ErrorMessage::Str(string) => string,
            ErrorMessage::String(string) => string,
        }
    }
}

impl From<&'static str> for ErrorMessage {
    fn from(value: &'static str) -> ErrorMessage {
        ErrorMessage::Str(value)
    }
}

impl From<String> for ErrorMessage {
    fn from(value: String) -> ErrorMessage {
        ErrorMessage::String(value)
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

impl ErrorContext {
    pub fn message(&self) -> &str {
        self.message.as_str()
    }
    pub fn source(&self) -> SourceRange {
        self.source
    }
}
