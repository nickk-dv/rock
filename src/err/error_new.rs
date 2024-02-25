use crate::check::SourceLoc;

#[derive(Clone)]
pub struct CompError {
    pub src: SourceLoc,
    pub msg: Message,
    pub context: Vec<ErrorContext>,
}

#[derive(Clone)]
pub enum ErrorContext {
    Message { msg: Message },
    MessageSource { ctx_src: SourceLoc, msg: Message },
}

#[derive(Clone)]
pub enum Message {
    Str(&'static str),
    String(String),
}

impl CompError {
    pub fn new(src: SourceLoc, msg: Message) -> Self {
        Self {
            src,
            msg,
            context: Vec::new(),
        }
    }

    pub fn context(mut self, ctx: ErrorContext) -> Self {
        self.context.push(ctx);
        self
    }

    pub fn add_context(&mut self, ctx: ErrorContext) {
        self.context.push(ctx);
    }
}

impl Message {
    pub fn as_str(&self) -> &str {
        match self {
            Message::Str(str) => str,
            Message::String(string) => string.as_str(),
        }
    }
}
