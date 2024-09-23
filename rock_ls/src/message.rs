use lsp_server::{Connection, RequestId};
use lsp_types as lsp;
use lsp_types::notification::{self, Notification as NotificationTrait};
use lsp_types::request::{self, Request as RequestTrait};
use std::path::PathBuf;
use std::time::Duration;

pub struct MessageBuffer {
    messages: Vec<Message>,
}

pub enum Action {
    Stop,
    Collect,
    Handle(Vec<Message>),
}

pub enum Message {
    Request(RequestId, Request),
    Notification(Notification),
    CompileProject,
}

pub enum Request {
    Completion(lsp::CompletionParams),
    GotoDefinition(lsp::GotoDefinitionParams),
    Format(lsp::DocumentFormattingParams),
    Hover(lsp::HoverParams),
    SemanticTokens(lsp::SemanticTokensParams),
}

pub enum Notification {
    SourceFileChanged { path: PathBuf, text: String },
    SourceFileClosed { path: PathBuf },
}

impl MessageBuffer {
    pub fn new() -> MessageBuffer {
        MessageBuffer {
            messages: Vec::new(),
        }
    }

    pub fn receive(&mut self, conn: &Connection) -> Action {
        let message = if self.messages.is_empty() {
            conn.receiver.recv().ok()
        } else {
            let pause = Duration::from_millis(150);
            conn.receiver.recv_timeout(pause).ok()
        };

        match message {
            Some(lsp_server::Message::Request(req)) => self.handle_request(conn, req),
            Some(lsp_server::Message::Response(resp)) => self.handle_response(resp),
            Some(lsp_server::Message::Notification(not)) => self.handle_notification(not),
            None => self.handle_user_pause(),
        }
    }

    fn handle_request(&mut self, conn: &Connection, req: lsp_server::Request) -> Action {
        if conn.handle_shutdown(&req).expect("shutdown") {
            return Action::Stop;
        }
        if let Some(message) = extract_request(req) {
            self.messages.push(Message::CompileProject);
            self.messages.push(message);
            Action::Handle(self.take_messages())
        } else {
            Action::Collect
        }
    }

    fn handle_response(&self, _: lsp_server::Response) -> Action {
        Action::Collect
    }

    fn handle_notification(&mut self, not: lsp_server::Notification) -> Action {
        if let Some(message) = extract_notification(not) {
            self.messages.push(message);
        }
        Action::Collect
    }

    fn handle_user_pause(&mut self) -> Action {
        self.messages.push(Message::CompileProject);
        Action::Handle(self.take_messages())
    }

    fn take_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }
}

fn extract_request(request: lsp_server::Request) -> Option<Message> {
    use request::{
        Completion, Formatting, GotoDefinition, HoverRequest, SemanticTokensFullRequest,
    };

    let id = request.id.clone();
    let request = match request.method.as_str() {
        Completion::METHOD => {
            let params = cast_request::<Completion>(request);
            Request::Completion(params)
        }
        GotoDefinition::METHOD => {
            let params = cast_request::<GotoDefinition>(request);
            Request::GotoDefinition(params)
        }
        Formatting::METHOD => {
            let params = cast_request::<Formatting>(request);
            Request::Format(params)
        }
        HoverRequest::METHOD => {
            let params = cast_request::<HoverRequest>(request);
            Request::Hover(params)
        }
        SemanticTokensFullRequest::METHOD => {
            let params = cast_request::<SemanticTokensFullRequest>(request);
            Request::SemanticTokens(params)
        }
        _ => {
            eprintln!("[UNKNOWN REQUEST RECEIVED]: {}", request.method);
            return None;
        }
    };
    Some(Message::Request(id, request))
}

fn extract_notification(notification: lsp_server::Notification) -> Option<Message> {
    use notification::{DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument};

    let notification = match notification.method.as_str() {
        DidOpenTextDocument::METHOD => {
            let params = cast_notification::<DidOpenTextDocument>(notification);
            Notification::SourceFileChanged {
                path: super::uri_to_path(&params.text_document.uri),
                text: params.text_document.text,
            }
        }
        DidChangeTextDocument::METHOD => {
            let params = cast_notification::<DidChangeTextDocument>(notification);
            Notification::SourceFileChanged {
                path: super::uri_to_path(&params.text_document.uri),
                text: params.content_changes.into_iter().last()?.text,
            }
        }
        DidCloseTextDocument::METHOD => {
            let params = cast_notification::<DidCloseTextDocument>(notification);
            Notification::SourceFileClosed {
                path: super::uri_to_path(&params.text_document.uri),
            }
        }
        _ => return None,
    };
    Some(Message::Notification(notification))
}

fn cast_request<R>(request: lsp_server::Request) -> R::Params
where
    R: RequestTrait,
    R::Params: serde::de::DeserializeOwned,
{
    let (_, params) = request.extract(R::METHOD).expect("cast request");
    params
}

fn cast_notification<N>(notification: lsp_server::Notification) -> N::Params
where
    N: NotificationTrait,
    N::Params: serde::de::DeserializeOwned,
{
    notification.extract(N::METHOD).expect("cast notification")
}
