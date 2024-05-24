#![forbid(unsafe_code)]

use lsp_server::{Connection, ExtractError, Message, Response};
use lsp_types::notification::{self, Notification};
use lsp_types::request::{self, Request};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(lsp_types::ServerCapabilities {
        text_document_sync: Some(lsp_types::TextDocumentSyncCapability::Kind(
            lsp_types::TextDocumentSyncKind::INCREMENTAL,
        )),
        completion_provider: Some(lsp_types::CompletionOptions {
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap();

    let initialization_params = match connection.initialize(server_capabilities) {
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

    main_loop(connection)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: Connection) -> Result<(), Box<dyn Error + Sync + Send>> {
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                eprintln!("\nGOT REQUEST: {req:?}\n");
                handle_request(&connection, req);
            }
            Message::Response(resp) => {
                eprintln!("\nGOT RESPONSE: {resp:?}\n");
                handle_responce(&connection, resp);
            }
            Message::Notification(not) => {
                eprintln!("\nGOT NOTIFICATION: {not:?}\n");
                handle_notification(&connection, not);
            }
        }
    }
    Ok(())
}

fn send<Content: Into<Message>>(conn: &Connection, msg: Content) {
    conn.sender.send(msg.into()).expect("send failed");
}

fn cast_req<P>(
    req: lsp_server::Request,
) -> Result<(lsp_server::RequestId, P::Params), ExtractError<lsp_server::Request>>
where
    P: lsp_types::request::Request,
    P::Params: serde::de::DeserializeOwned,
{
    req.extract(P::METHOD)
}

fn cast_not<P>(
    not: lsp_server::Notification,
) -> Result<P::Params, ExtractError<lsp_server::Notification>>
where
    P: notification::Notification,
    P::Params: serde::de::DeserializeOwned,
{
    not.extract(P::METHOD)
}

fn handle_request(conn: &Connection, req: lsp_server::Request) {
    match req.method.as_str() {
        request::Completion::METHOD => {
            let (id, params) = cast_req::<request::Completion>(req).unwrap();
        }
        _ => {}
    }
}

fn handle_responce(conn: &Connection, resp: lsp_server::Response) {}

fn handle_notification(conn: &Connection, not: lsp_server::Notification) {
    match not.method.as_str() {
        notification::Cancel::METHOD => {
            let params = cast_not::<notification::Cancel>(not).unwrap();
        }
        notification::DidChangeTextDocument::METHOD => {
            let params = cast_not::<notification::DidChangeTextDocument>(not).unwrap();
        }
        notification::DidSaveTextDocument::METHOD => {
            let params = cast_not::<notification::DidSaveTextDocument>(not).unwrap();

            let publish_diagnostics = run_diagnostics();
            for publish in publish_diagnostics.iter() {
                send(
                    conn,
                    lsp_server::Notification::new(
                        notification::PublishDiagnostics::METHOD.into(),
                        publish,
                    ),
                );
            }
        }
        _ => {}
    }
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

fn url_from_path(path: &PathBuf) -> lsp_types::Url {
    match lsp_types::Url::from_file_path(path) {
        Ok(url) => url,
        Err(()) => panic!("failed to convert `{}` to url", path.to_string_lossy()),
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
    let message = diagnostic.message();

    let (main, info) = match diagnostic.kind() {
        DiagnosticKind::Message => return None, //@some diagnostic messages dont have source for example session errors or manifest errors
        DiagnosticKind::Context { main, info } => (main, info),
        DiagnosticKind::ContextVec { main, info } => panic!("diagnostic info vec not supported"),
    };

    let (main_range, main_path) = source_to_range_and_path(&session, main.source());

    let mut diagnostic = lsp_types::Diagnostic::new_simple(main_range, message.as_str().into());
    diagnostic.severity = match severity {
        DiagnosticSeverity::Info => Some(lsp_types::DiagnosticSeverity::HINT),
        DiagnosticSeverity::Error => Some(lsp_types::DiagnosticSeverity::ERROR),
        DiagnosticSeverity::Warning => Some(lsp_types::DiagnosticSeverity::WARNING),
    };

    if let Some(info) = info {
        let (info_range, info_path) = source_to_range_and_path(&session, info.source());

        diagnostic.related_information = Some(vec![DiagnosticRelatedInformation {
            location: Location::new(url_from_path(info_path), info_range),
            message: info.message().to_string(),
        }]);
    }

    Some((diagnostic, main_path))
}

fn run_diagnostics() -> Vec<PublishDiagnosticsParams> {
    use std::collections::HashMap;

    //@session errors ignored, its not a correct way to have context in ls server
    // this is a temporary full compilation run
    let session = Session::new()
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
