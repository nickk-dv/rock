#![forbid(unsafe_code)]

mod message;
mod message_buffer;

use lsp_server as lsp;
use lsp_types::notification::{self, Notification as NotificationTrait};
use message::{Message, Notification, Request};
use message_buffer::{Action, MessageBuffer};
use std::collections::HashMap;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (conn, io_threads) = lsp::Connection::stdio();

    let server_capabilities = serde_json::to_value(lsp_types::ServerCapabilities {
        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
            lsp_types::TextDocumentSyncKind::FULL,
        )),
        completion_provider: Some(lsp_types::CompletionOptions {
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap();

    let initialization_params = match conn.initialize(server_capabilities) {
        Ok(it) => it,
        Err(e) => {
            if e.channel_is_disconnected() {
                io_threads.join()?;
            }
            return Err(e.into());
        }
    };
    let params: lsp_types::InitializeParams =
        serde_json::from_value(initialization_params).unwrap();

    main_loop(&conn);
    io_threads.join()?;
    Ok(())
}

struct ServerContext {
    files_in_memory: HashMap<PathBuf, String>,
}

impl ServerContext {
    fn new() -> ServerContext {
        ServerContext {
            files_in_memory: HashMap::new(),
        }
    }
}

fn main_loop(conn: &lsp::Connection) {
    let mut buffer = MessageBuffer::new();
    let mut context = ServerContext::new();

    loop {
        match buffer.receive(&conn) {
            Action::Stop => break,
            Action::Collect => continue,
            Action::Handle(messages) => handle_messages(&conn, &mut context, messages),
        }
    }
}

fn handle_messages(conn: &lsp::Connection, context: &mut ServerContext, messages: Vec<Message>) {
    for message in messages {
        match message {
            Message::Request(id, req) => handle_request(conn, id.clone(), req),
            Message::Notification(not) => handle_notification(context, not),
            Message::CompileProject => handle_compile_project(conn, context),
        }
    }
}

fn handle_request(conn: &lsp::Connection, id: lsp_server::RequestId, req: Request) {
    match req {
        Request::Completion(params) => {}
        Request::GotoDefinition(params) => {}
        Request::Format(params) => {}
        Request::Hover(params) => {}
    }
}

fn handle_notification(context: &mut ServerContext, not: Notification) {
    match not {
        Notification::SourceFileChanged { path, text } => {
            context.files_in_memory.insert(path, text);
        }
        Notification::SourceFileClosed { path } => {
            context.files_in_memory.remove(&path);
        }
    }
}

fn handle_compile_project(conn: &lsp::Connection, context: &ServerContext) {
    use std::time::Instant;
    let start_time = Instant::now();
    let publish_diagnostics = run_diagnostics(context);
    let elapsed_time = start_time.elapsed();
    eprintln!(
        "run diagnostics: {} ms",
        elapsed_time.as_secs_f64() * 1000.0
    );

    for publish in publish_diagnostics.iter() {
        send(
            conn,
            lsp_server::Notification::new(notification::PublishDiagnostics::METHOD.into(), publish),
        );
    }
}

fn send<Content: Into<lsp_server::Message>>(conn: &lsp::Connection, msg: Content) {
    conn.sender.send(msg.into()).expect("send failed");
}

use rock_core::ast_parse;
use rock_core::error::{
    Diagnostic, DiagnosticCollection, DiagnosticKind, DiagnosticSeverity, SourceRange, WarningComp,
};
use rock_core::hir_lower;
use rock_core::session::Session;
use rock_core::text;

use lsp_types::{
    DiagnosticRelatedInformation, Location, Position, PublishDiagnosticsParams, Range,
};
use std::path::PathBuf;

fn check_impl(session: &Session) -> Result<Vec<WarningComp>, DiagnosticCollection> {
    let (ast, warnings) = ast_parse::parse(session).into_result(vec![])?;
    let (_, warnings) = hir_lower::check(ast, session).into_result(warnings)?;
    Ok(warnings)
}

fn uri_to_path(uri: lsp_types::Url) -> PathBuf {
    uri.to_file_path().expect("uri to pathbuf")
}

fn url_from_path(path: &PathBuf) -> lsp_types::Url {
    match lsp_types::Url::from_file_path(path) {
        Ok(url) => url,
        Err(()) => panic!("failed to convert `{}` to url", path.to_string_lossy()),
    }
}

fn severity_convert(severity: DiagnosticSeverity) -> Option<lsp_types::DiagnosticSeverity> {
    match severity {
        DiagnosticSeverity::Info => Some(lsp_types::DiagnosticSeverity::HINT),
        DiagnosticSeverity::Error => Some(lsp_types::DiagnosticSeverity::ERROR),
        DiagnosticSeverity::Warning => Some(lsp_types::DiagnosticSeverity::WARNING),
    }
}

fn source_to_range_and_path(session: &Session, source: SourceRange) -> (Range, &PathBuf) {
    let file = session.file(source.file_id());

    let start_location =
        text::find_text_location(&file.source, source.range().start(), &file.line_ranges);
    let end_location =
        text::find_text_location(&file.source, source.range().end(), &file.line_ranges);

    let range = Range::new(
        Position::new(start_location.line() - 1, start_location.col() - 1),
        Position::new(end_location.line() - 1, end_location.col() - 1),
    );

    (range, &file.path)
}

fn create_diagnostic<'src>(
    session: &'src Session,
    diagnostic: &Diagnostic,
    severity: DiagnosticSeverity,
) -> Option<(lsp_types::Diagnostic, &'src PathBuf)> {
    let (main, related_info) = match diagnostic.kind() {
        DiagnosticKind::Message => return None, //@some diagnostic messages dont have source for example session errors or manifest errors
        DiagnosticKind::Context { main, info } => {
            if let Some(info) = info {
                let (info_range, info_path) = source_to_range_and_path(session, info.source());
                let related_info = DiagnosticRelatedInformation {
                    location: Location::new(url_from_path(info_path), info_range),
                    message: info.message().to_string(),
                };
                (main, Some(vec![related_info]))
            } else {
                (main, None)
            }
        }
        DiagnosticKind::ContextVec { main, info_vec } => {
            let mut related_infos = Vec::with_capacity(info_vec.len());
            for info in info_vec {
                let (info_range, info_path) = source_to_range_and_path(session, info.source());
                let related_info = DiagnosticRelatedInformation {
                    location: Location::new(url_from_path(info_path), info_range),
                    message: info.message().to_string(),
                };
                related_infos.push(related_info);
            }
            (main, Some(related_infos))
        }
    };

    let (main_range, main_path) = source_to_range_and_path(session, main.source());

    let mut message = diagnostic.message().as_str().to_string();
    if !main.message().is_empty() {
        message.push('\n');
        message += main.message();
    }

    let diagnostic = lsp_types::Diagnostic::new(
        main_range,
        severity_convert(severity),
        None,
        None,
        message,
        related_info,
        None,
    );

    Some((diagnostic, main_path))
}

fn run_diagnostics(context: &ServerContext) -> Vec<PublishDiagnosticsParams> {
    //@session errors ignored, its not a correct way to have context in ls server
    // this is a temporary full compilation run
    let session = Session::new(false, Some(&context.files_in_memory))
        .map_err(|_| Result::<(), ()>::Err(()))
        .expect("lsp session errors cannot be handled");
    let check_result = check_impl(&session);
    let diagnostics = DiagnosticCollection::from_result(check_result);

    // assign empty diagnostics
    let mut diagnostics_map = HashMap::new();
    for file_id in session.file_ids() {
        let path = session.file(file_id).path.clone();
        diagnostics_map.insert(path, Vec::new());
    }

    // generate diagnostics
    for warning in diagnostics.warnings() {
        if let Some((diagnostic, main_path)) =
            create_diagnostic(&session, warning.diagnostic(), DiagnosticSeverity::Warning)
        {
            match diagnostics_map.get_mut(main_path) {
                Some(diagnostics) => diagnostics.push(diagnostic),
                None => {
                    diagnostics_map.insert(main_path.clone(), vec![diagnostic]);
                }
            }
        }
    }

    for error in diagnostics.errors() {
        if let Some((diagnostic, main_path)) =
            create_diagnostic(&session, error.diagnostic(), DiagnosticSeverity::Error)
        {
            match diagnostics_map.get_mut(main_path) {
                Some(diagnostics) => diagnostics.push(diagnostic),
                None => {
                    diagnostics_map.insert(main_path.clone(), vec![diagnostic]);
                }
            }
        }
    }

    //@not using any document versioning
    diagnostics_map
        .into_iter()
        .map(|(path, diagnostics)| {
            PublishDiagnosticsParams::new(url_from_path(&path), diagnostics, None)
        })
        .collect()
}
