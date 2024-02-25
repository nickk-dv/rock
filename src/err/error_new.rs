use crate::check::SourceLoc;

#[derive(Clone)]
pub struct CompError {
    pub error: Message,
    pub context: Vec<Message>,
}

#[derive(Clone)]
pub struct Message {
    pub src: SourceLoc,
    pub message: ErrorMessage,
}

#[derive(Clone)]
pub enum ErrorMessage {
    Str(&'static str),
    String(String),
}

impl ErrorMessage {
    pub fn as_str(&self) -> &str {
        match self {
            ErrorMessage::Str(str) => str,
            ErrorMessage::String(string) => string.as_str(),
        }
    }
}
