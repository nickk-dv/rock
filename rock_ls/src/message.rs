use lsp_server as lsr;
use lsp_types as lsp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub struct MessageBuffer {
    messages: Vec<Message>,
}

pub enum Action {
    Collect,
    Shutdown,
    Handle(Vec<Message>),
}

pub enum Message {
    Request(lsr::RequestId, Request),
    Notification(Notification),
}

pub enum Request {
    Format(PathBuf),
    SemanticTokens(PathBuf),
    InlayHints(PathBuf, lsp::Range),
    GotoDefinition(PathBuf, lsp::Position),
    ShowSyntaxTree(PathBuf),
}

pub enum Notification {
    FileSaved,
    FileCreate(lsp::CreateFilesParams),
    FileDelete(lsp::DeleteFilesParams),
    FileRename(lsp::RenameFilesParams),
    FileChanged(lsp::DidChangeTextDocumentParams),
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
            let text_edit = matches!(&message, Message::Notification(Notification::FileChanged(_)));
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
            let params = cast_req::<r::Formatting>(req);
            Request::Format(super::uri_to_path(&params.text_document.uri))
        }
        r::SemanticTokensFullRequest::METHOD => {
            let params = cast_req::<r::SemanticTokensFullRequest>(req);
            Request::SemanticTokens(super::uri_to_path(&params.text_document.uri))
        }
        r::InlayHintRequest::METHOD => {
            let params = cast_req::<r::InlayHintRequest>(req);
            Request::InlayHints(super::uri_to_path(&params.text_document.uri), params.range)
        }
        r::GotoDefinition::METHOD => {
            let params = cast_req::<r::GotoDefinition>(req);
            let doc = &params.text_document_position_params;
            Request::GotoDefinition(super::uri_to_path(&doc.text_document.uri), doc.position)
        }
        CustomShowSyntaxTree::METHOD => {
            let params = cast_req::<CustomShowSyntaxTree>(req);
            Request::ShowSyntaxTree(super::uri_to_path(&params.text_document.uri))
        }
        _ => return None,
    };
    Some(Message::Request(req_id, req))
}

fn parse_notification(not: lsr::Notification) -> Option<Message> {
    use lsp::notification::{self as n, Notification as NotificationTrait};

    let not = match not.method.as_str() {
        n::DidSaveTextDocument::METHOD => Notification::FileSaved,
        n::DidCreateFiles::METHOD => Notification::FileCreate(cast_not::<n::DidCreateFiles>(not)),
        n::DidDeleteFiles::METHOD => Notification::FileDelete(cast_not::<n::DidDeleteFiles>(not)),
        n::DidRenameFiles::METHOD => Notification::FileRename(cast_not::<n::DidRenameFiles>(not)),
        n::DidChangeTextDocument::METHOD => {
            Notification::FileChanged(cast_not::<n::DidChangeTextDocument>(not))
        }
        _ => return None,
    };
    Some(Message::Notification(not))
}

fn cast_req<R: lsp::request::Request>(req: lsr::Request) -> R::Params {
    req.extract(R::METHOD).expect("cast request").1
}

fn cast_not<N: lsp::notification::Notification>(not: lsr::Notification) -> N::Params {
    not.extract(N::METHOD).expect("cast notification")
}
