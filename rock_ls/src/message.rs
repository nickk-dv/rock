use lsp_server as lsp;
use lsp_types::notification::{self, Notification as NotificationTrait};
use lsp_types::request::{self, Request as RequestTrait};
use std::path::PathBuf;

pub enum Message {
    Request(lsp::RequestId, Request),
    Notification(Notification),
    CompileProject,
}

pub enum Request {
    Completion(lsp_types::CompletionParams),
    GotoDefinition(lsp_types::GotoDefinitionParams),
    Format(lsp_types::DocumentFormattingParams),
    Hover(lsp_types::HoverParams),
}

pub enum Notification {
    SourceFileChanged { path: PathBuf, text: String },
    SourceFileClosed { path: PathBuf },
}

impl Request {
    pub fn new(request: lsp::Request) -> Option<Message> {
        use request::{Completion, Formatting, GotoDefinition, HoverRequest};

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
            _ => return None,
        };
        Some(Message::Request(id, request))
    }
}

impl Notification {
    pub fn new(notification: lsp::Notification) -> Option<Message> {
        use notification::{DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument};

        let notification = match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params = cast_notification::<DidOpenTextDocument>(notification);
                Notification::SourceFileChanged {
                    path: super::uri_to_path(params.text_document.uri),
                    text: params.text_document.text,
                }
            }
            DidChangeTextDocument::METHOD => {
                let params = cast_notification::<DidChangeTextDocument>(notification);
                Notification::SourceFileChanged {
                    path: super::uri_to_path(params.text_document.uri),
                    text: params.content_changes.into_iter().last()?.text,
                }
            }
            DidCloseTextDocument::METHOD => {
                let params = cast_notification::<DidCloseTextDocument>(notification);
                Notification::SourceFileClosed {
                    path: super::uri_to_path(params.text_document.uri),
                }
            }
            _ => return None,
        };
        Some(Message::Notification(notification))
    }
}

fn cast_request<R>(request: lsp::Request) -> R::Params
where
    R: RequestTrait,
    R::Params: serde::de::DeserializeOwned,
{
    let (_, params) = request.extract(R::METHOD).expect("cast request");
    params
}

fn cast_notification<N>(notification: lsp::Notification) -> N::Params
where
    N: NotificationTrait,
    N::Params: serde::de::DeserializeOwned,
{
    notification.extract(N::METHOD).expect("cast notification")
}
