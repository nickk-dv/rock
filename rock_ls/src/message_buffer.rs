use super::message::{Message, Notification, Request};
use lsp_server as lsp;
use std::time::Duration;

pub struct MessageBuffer {
    messages: Vec<Message>,
}

pub enum Action {
    Stop,
    Collect,
    Handle(Vec<Message>),
}

impl MessageBuffer {
    pub fn new() -> MessageBuffer {
        MessageBuffer {
            messages: Vec::new(),
        }
    }

    pub fn receive(&mut self, conn: &lsp::Connection) -> Action {
        let message = if self.messages.is_empty() {
            conn.receiver.recv().ok()
        } else {
            let pause = Duration::from_millis(100);
            conn.receiver.recv_timeout(pause).ok()
        };

        match message {
            Some(lsp::Message::Request(req)) => self.handle_request(conn, req),
            Some(lsp::Message::Response(resp)) => self.handle_response(resp),
            Some(lsp::Message::Notification(not)) => self.handle_notification(not),
            None => self.handle_user_pause(),
        }
    }

    fn handle_request(&mut self, conn: &lsp::Connection, req: lsp::Request) -> Action {
        if conn.handle_shutdown(&req).expect("shutdown") {
            return Action::Stop;
        }
        if let Some(message) = Request::new(req) {
            self.messages.push(Message::CompileProject);
            self.messages.push(message);
            Action::Handle(self.take_messages())
        } else {
            Action::Collect
        }
    }

    fn handle_response(&self, _: lsp::Response) -> Action {
        Action::Collect
    }

    fn handle_notification(&mut self, not: lsp::Notification) -> Action {
        if let Some(message) = Notification::new(not) {
            self.messages.push(message);
        }
        Action::Collect
    }

    fn handle_user_pause(&mut self) -> Action {
        self.messages.push(Message::CompileProject);
        return Action::Handle(self.take_messages());
    }

    fn take_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.messages)
    }
}
