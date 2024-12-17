use lsp_server as lsr;
use lsp_types as lsp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub struct MessageBuffer {
    messages: Vec<Message>,
}
pub enum Action {
    Collect,
    Handle(Vec<Message>),
    Shutdown,
}
pub enum Message {
    Request(lsr::RequestId, Request),
    Notification(Notification),
}
pub enum Request {
    Format(lsp::DocumentFormattingParams),
    SemanticTokens(lsp::SemanticTokensParams),
    ShowSyntaxTree(ShowSyntaxTreeParams),
}
pub enum Notification {
    FileOpened(PathBuf, String),
    FileClosed(PathBuf),
    FileSaved(PathBuf),
    FileChanged(PathBuf, Vec<lsp::TextDocumentContentChangeEvent>),
}

//@buffering only to avoid doing semantic tokens on each edit (16.12.24).
impl MessageBuffer {
    pub fn new() -> MessageBuffer {
        MessageBuffer { messages: Vec::new() }
    }

    pub fn receive(&mut self, conn: &lsr::Connection) -> Action {
        let message = if self.messages.is_empty() {
            conn.receiver.recv().ok()
        } else {
            let pause = std::time::Duration::from_millis(250);
            conn.receiver.recv_timeout(pause).ok()
        };

        match message {
            Some(lsr::Message::Request(req)) => self.handle_request(conn, req),
            Some(lsr::Message::Response(_)) => Action::Collect,
            Some(lsr::Message::Notification(not)) => self.handle_notification(not),
            None => Action::Handle(std::mem::take(&mut self.messages)),
        }
    }

    fn handle_request(&mut self, conn: &lsr::Connection, req: lsr::Request) -> Action {
        if conn.handle_shutdown(&req).expect("shutdown") {
            Action::Shutdown
        } else if let Some(message) = parse_request(req) {
            self.messages.push(message);
            Action::Handle(std::mem::take(&mut self.messages))
        } else {
            Action::Collect
        }
    }

    fn handle_notification(&mut self, not: lsr::Notification) -> Action {
        if let Some(message) = parse_notification(not) {
            let text_edit = match &message {
                Message::Notification(Notification::FileChanged(_, _)) => true,
                _ => false,
            };
            self.messages.push(message);
            if text_edit {
                Action::Collect
            } else {
                Action::Handle(std::mem::take(&mut self.messages))
            }
        } else {
            Action::Collect
        }
    }
}

fn parse_request(req: lsr::Request) -> Option<Message> {
    use lsp::request::{self as r, Request as RequestTrait};

    let req_id = req.id.clone();
    let req = match req.method.as_str() {
        r::Formatting::METHOD => {
            let params = cast_request::<r::Formatting>(req);
            Request::Format(params)
        }
        r::SemanticTokensFullRequest::METHOD => {
            let params = cast_request::<r::SemanticTokensFullRequest>(req);
            Request::SemanticTokens(params)
        }
        CustomShowSyntaxTree::METHOD => {
            let params = cast_request::<CustomShowSyntaxTree>(req);
            Request::ShowSyntaxTree(params)
        }
        _ => return None,
    };
    Some(Message::Request(req_id, req))
}

fn parse_notification(not: lsr::Notification) -> Option<Message> {
    use lsp::notification::{self as n, Notification as NotificationTrait};

    let not = match not.method.as_str() {
        n::DidOpenTextDocument::METHOD => {
            let params = cast_notification::<n::DidOpenTextDocument>(not);
            Notification::FileOpened(
                super::uri_to_path(&params.text_document.uri),
                params.text_document.text,
            )
        }
        n::DidCloseTextDocument::METHOD => {
            let params = cast_notification::<n::DidCloseTextDocument>(not);
            Notification::FileClosed(super::uri_to_path(&params.text_document.uri))
        }
        n::DidSaveTextDocument::METHOD => {
            let params = cast_notification::<n::DidSaveTextDocument>(not);
            Notification::FileSaved(super::uri_to_path(&params.text_document.uri))
        }
        n::DidChangeTextDocument::METHOD => {
            let params = cast_notification::<n::DidChangeTextDocument>(not);
            Notification::FileChanged(
                super::uri_to_path(&params.text_document.uri),
                params.content_changes,
            )
        }
        _ => return None,
    };
    Some(Message::Notification(not))
}

fn cast_request<R>(req: lsr::Request) -> R::Params
where
    R: lsp::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    let (_, params) = req.extract(R::METHOD).expect("cast request");
    params
}

fn cast_notification<N>(not: lsr::Notification) -> N::Params
where
    N: lsp::notification::Notification,
    N::Params: serde::de::DeserializeOwned,
{
    not.extract(N::METHOD).expect("cast notification")
}

pub enum CustomShowSyntaxTree {}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowSyntaxTreeParams {
    pub text_document: lsp::TextDocumentIdentifier,
}

impl lsp::request::Request for CustomShowSyntaxTree {
    type Params = ShowSyntaxTreeParams;
    type Result = String;
    const METHOD: &str = "custom/show_syntax_tree";
}
