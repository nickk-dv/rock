#![forbid(unsafe_code)]

mod message;

use lsp_server::{Connection, RequestId};
use lsp_types as lsp;
use lsp_types::notification::{self, Notification as NotificationTrait};
use message::{Action, Message, MessageBuffer, Notification, Request};
use rock_core::support::ID;
use rock_core::syntax::syntax_tree::{Node, NodeOrToken, SyntaxTree};
use rock_core::token::{SemanticToken, Token, Trivia};
use std::collections::HashMap;

fn main() {
    if !check_args() {
        return;
    };
    let (conn, io_threads) = Connection::stdio();
    let _ = initialize_handshake(&conn);

    server_loop(&conn);

    drop(conn);
    io_threads.join().expect("io_threads joined");
}

fn check_args() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let started = match args.get(0) {
        Some(first) => first == "lsp",
        _ => false,
    };

    let message = r#"`rock_ls` is a language server
its started by your editor or editor extension
you do not need to run `rock_ls` manually"#;
    if !started {
        eprintln!("{message}");
    }
    started
}

fn initialize_handshake(conn: &Connection) -> lsp::InitializeParams {
    let capabilities = lsp::ServerCapabilities {
        position_encoding: None,
        text_document_sync: Some(lsp::TextDocumentSyncCapability::Options(
            lsp::TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(lsp::TextDocumentSyncKind::FULL),
                will_save: Some(false),
                will_save_wait_until: Some(false),
                save: Some(lsp::TextDocumentSyncSaveOptions::SaveOptions(
                    lsp::SaveOptions {
                        include_text: Some(false),
                    },
                )),
            },
        )),
        selection_range_provider: None,
        hover_provider: None,
        //@re-enable when supported
        //hover_provider: Some(lsp::HoverProviderCapability::Simple(true)),
        completion_provider: Some(lsp::CompletionOptions {
            resolve_provider: None,
            trigger_characters: Some(vec![".".into()]),
            all_commit_characters: None,
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: None,
            },
            completion_item: None,
        }),
        signature_help_provider: None,
        definition_provider: None,
        //@re-enable when supported
        //definition_provider: Some(lsp::OneOf::Left(true)),
        type_definition_provider: None,
        implementation_provider: None,
        references_provider: None,
        document_highlight_provider: None,
        document_symbol_provider: None,
        workspace_symbol_provider: None,
        code_action_provider: None,
        code_lens_provider: None,
        document_formatting_provider: Some(lsp::OneOf::Left(true)),
        document_range_formatting_provider: None,
        document_on_type_formatting_provider: None,
        rename_provider: None,
        document_link_provider: None,
        color_provider: None,
        folding_range_provider: None,
        declaration_provider: None,
        execute_command_provider: None,
        workspace: None,
        call_hierarchy_provider: None,
        semantic_tokens_provider: Some(
            lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(
                lsp::SemanticTokensOptions {
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    legend: lsp::SemanticTokensLegend {
                        token_types: vec![
                            lsp::SemanticTokenType::NAMESPACE,
                            lsp::SemanticTokenType::TYPE,
                            lsp::SemanticTokenType::PARAMETER,
                            lsp::SemanticTokenType::VARIABLE,
                            lsp::SemanticTokenType::PROPERTY,
                            lsp::SemanticTokenType::ENUM_MEMBER,
                            lsp::SemanticTokenType::FUNCTION,
                            lsp::SemanticTokenType::KEYWORD,
                            lsp::SemanticTokenType::COMMENT,
                            lsp::SemanticTokenType::NUMBER,
                            lsp::SemanticTokenType::STRING,
                        ],
                        token_modifiers: vec![],
                    },
                    range: None,
                    full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
                },
            ),
        ),
        moniker_provider: None,
        linked_editing_range_provider: None,
        inline_value_provider: None,
        inlay_hint_provider: None,
        diagnostic_provider: None,
        experimental: None,
    };

    let capabilities_json = serde_json::to_value(capabilities).expect("capabilities to json");
    let initialize_params_json = conn.initialize(capabilities_json).expect("lsp initialize");
    serde_json::from_value(initialize_params_json).expect("initialize_params from json")
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

fn server_loop(conn: &Connection) {
    let mut buffer = MessageBuffer::new();
    let mut context = ServerContext::new();

    loop {
        match buffer.receive(conn) {
            Action::Stop => break,
            Action::Collect => continue,
            Action::Handle(messages) => handle_messages(conn, &mut context, messages),
        }
    }
}

fn handle_messages(conn: &Connection, context: &mut ServerContext, messages: Vec<Message>) {
    for message in messages {
        match message {
            Message::Request(id, req) => handle_request(conn, context, id.clone(), req),
            Message::Notification(not) => handle_notification(context, not),
            Message::CompileProject => handle_compile_project(conn, context),
        }
    }
}

fn handle_request(conn: &Connection, context: &mut ServerContext, id: RequestId, req: Request) {
    match &req {
        Request::Completion(params) => {}
        Request::GotoDefinition(params) => {}
        Request::Format(params) => {
            send_response_error(conn, id, None);
            /*
            let uri = params.text_document.uri;
            let path = uri_to_path(&uri);

            if let Some(source) = context.files_in_memory.get(&path) {
                //@random ModuleID used
                if let Ok(formatted) = rock_core::format::format(source, ModuleID::new_raw(0)) {
                    let line_count = source.lines().count() as u32;
                    context.files_in_memory.insert(path, formatted.clone());

                    //@send the more presice lsp::TextDocumentEdit with uri?
                    let text_edit = lsp::TextEdit {
                        range: lsp::Range::new(
                            lsp::Position::new(0, 0),
                            lsp::Position::new(line_count, 0),
                        ),
                        new_text: formatted,
                    };

                    let json = serde_json::to_value(vec![text_edit]).expect("json value");
                    send_response(conn, id, json);
                } else {
                    send_response_error(conn, id, None);
                }
            } else {
                send_response_error(conn, id, None);
            }
            */
        }
        Request::Hover(params) => {
            let path = uri_to_path(&params.text_document_position_params.text_document.uri);
            eprintln!("[Handle] Request::Hover\n - document: {:?}", &path);
        }
        Request::SemanticTokens(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::SemanticTokens\n - document: {:?}", &path);
        }
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

fn handle_compile_project(conn: &Connection, context: &ServerContext) {
    use std::time::Instant;
    let start_time = Instant::now();
    let publish_diagnostics = run_diagnostics(conn, context);
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

fn send_response(conn: &Connection, id: RequestId, result: serde_json::Value) {
    let response = lsp_server::Response::new_ok(id, result);
    send(conn, response);
}

fn send_response_error(conn: &Connection, id: RequestId, with_message: Option<String>) {
    let response = if let Some(message) = with_message {
        lsp_server::Response::new_err(id, lsp_server::ErrorCode::RequestFailed as i32, message)
    } else {
        lsp_server::Response::new_err(
            id,
            lsp_server::ErrorCode::ServerCancelled as i32,
            "quiet ignore".into(),
        )
    };
    send(conn, response);
}

fn send<Content: Into<lsp_server::Message>>(conn: &Connection, msg: Content) {
    conn.sender.send(msg.into()).expect("send message");
}

use rock_core::error::{
    Diagnostic, DiagnosticCollection, DiagnosticKind, DiagnosticSeverity, SourceRange, WarningComp,
};
use rock_core::hir_lower;
use rock_core::intern::{InternName, InternPool};
use rock_core::session::{ModuleID, Session};
use rock_core::syntax::ast_build;
use rock_core::text;

use lsp::{DiagnosticRelatedInformation, Location, Position, PublishDiagnosticsParams, Range};
use std::path::PathBuf;

fn check_impl(session: &mut Session) -> Result<Vec<WarningComp>, DiagnosticCollection> {
    let ((), warnings) = ast_build::parse(session).into_result(vec![])?;
    let (_, warnings) = hir_lower::check(session).into_result(warnings)?;
    Ok(warnings)
}

fn uri_to_path(uri: &lsp::Url) -> PathBuf {
    uri.to_file_path().expect("uri to pathbuf")
}

fn url_from_path(path: &PathBuf) -> lsp::Url {
    match lsp::Url::from_file_path(path) {
        Ok(url) => url,
        Err(()) => panic!("failed to convert `{}` to url", path.to_string_lossy()),
    }
}

fn severity_convert(severity: DiagnosticSeverity) -> Option<lsp::DiagnosticSeverity> {
    match severity {
        DiagnosticSeverity::Info => Some(lsp::DiagnosticSeverity::HINT),
        DiagnosticSeverity::Error => Some(lsp::DiagnosticSeverity::ERROR),
        DiagnosticSeverity::Warning => Some(lsp::DiagnosticSeverity::WARNING),
    }
}

fn source_to_range_and_path<'s, 's_ref: 's>(
    session: &'s_ref Session<'s>,
    source: SourceRange,
) -> (Range, &'s PathBuf) {
    let module = session.pkg_storage.module(source.module_id());

    let start_location =
        text::find_text_location(&module.source, source.range().start(), &module.line_ranges);
    let end_location =
        text::find_text_location(&module.source, source.range().end(), &module.line_ranges);

    let range = Range::new(
        Position::new(start_location.line() - 1, start_location.col() - 1),
        Position::new(end_location.line() - 1, end_location.col() - 1),
    );

    (range, &module.path)
}

fn create_diagnostic<'src>(
    session: &'src Session,
    diagnostic: &Diagnostic,
    severity: DiagnosticSeverity,
) -> Option<(lsp::Diagnostic, &'src PathBuf)> {
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

    let diagnostic = lsp::Diagnostic::new(
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

//@use something like this state later
//@store compiler diagnostics and convert on send only
struct Feedback {
    messages: Vec<lsp::Diagnostic>,
    diagnostics: HashMap<PathBuf, Vec<lsp::Diagnostic>>,
}

impl Feedback {
    fn new() -> Feedback {
        Feedback {
            messages: Vec::new(),
            diagnostics: HashMap::with_capacity(64),
        }
    }
}

fn run_diagnostics(conn: &Connection, context: &ServerContext) -> Vec<PublishDiagnosticsParams> {
    //@not used now, only sending one session error
    let mut messages = Vec::<lsp::Diagnostic>::new();
    let mut diagnostics_map = HashMap::new();

    let mut session = match Session::new(false, Some(&context.files_in_memory)) {
        Ok(value) => value,
        Err(error) => {
            let params = lsp::ShowMessageParams {
                typ: lsp::MessageType::ERROR,
                message: error.diagnostic().message().as_str().to_string(),
            };
            let notification = lsp_server::Notification {
                method: notification::ShowMessage::METHOD.into(),
                params: serde_json::to_value(params).unwrap(),
            };
            send(conn, notification);
            return vec![];
        }
    };

    let check_result = check_impl(&mut session);
    let diagnostics = DiagnosticCollection::from_result(check_result);

    for module_id in session.pkg_storage.module_ids() {
        let path = session.pkg_storage.module(module_id).path.clone();
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

    diagnostics_map
        .into_iter()
        .map(|(path, diagnostics)| {
            PublishDiagnosticsParams::new(url_from_path(&path), diagnostics, None)
        })
        .collect()
}

struct SemanticTokenBuilder {
    curr_line: u32,
    module_id: ModuleID,
    token_ids: Vec<ID<Token>>,
    tokens: Vec<lsp::SemanticToken>,
}

fn semantic_tokens(
    tree: &SyntaxTree,
    session: &Session,
    module_id: ModuleID,
) -> Vec<lsp::SemanticToken> {
    let mut builder = SemanticTokenBuilder {
        curr_line: 0,
        module_id,
        //@temp semantic token count estimate
        token_ids: Vec::with_capacity(tree.tokens().token_count() / 2),
        tokens: Vec::with_capacity(tree.tokens().token_count() / 2),
    };
    semantic_token_visit_node(&mut builder, tree, session, tree.root());
    builder.tokens
}

fn semantic_token_visit_node(
    builder: &mut SemanticTokenBuilder,
    tree: &SyntaxTree,
    session: &Session,
    node: &Node,
) {
    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                let node = tree.node(node_id);
                semantic_token_visit_node(builder, tree, session, node);
            }
            NodeOrToken::Token(token_id) => {
                let (token, range) = tree.tokens().token_and_range(token_id);

                let semantic = match token {
                    Token::Eof => continue,
                    Token::Ident => SemanticToken::Property, //@depends on node.kind
                    Token::IntLit | Token::FloatLit => SemanticToken::Number,
                    Token::CharLit | Token::StringLit => SemanticToken::String,
                    Token::KwPub
                    | Token::KwProc
                    | Token::KwEnum
                    | Token::KwStruct
                    | Token::KwConst
                    | Token::KwGlobal
                    | Token::KwImport
                    | Token::KwBreak
                    | Token::KwContinue
                    | Token::KwReturn
                    | Token::KwDefer
                    | Token::KwFor
                    | Token::KwLet
                    | Token::KwMut => SemanticToken::Keyword,
                    Token::KwNull | Token::KwTrue | Token::KwFalse => SemanticToken::Number,
                    Token::KwIf | Token::KwElse | Token::KwMatch => SemanticToken::Keyword,
                    Token::KwDiscard => SemanticToken::Property,
                    Token::KwAs | Token::KwSizeof => SemanticToken::Keyword,
                    Token::KwS8
                    | Token::KwS16
                    | Token::KwS32
                    | Token::KwS64
                    | Token::KwSsize
                    | Token::KwU8
                    | Token::KwU16
                    | Token::KwU32
                    | Token::KwU64
                    | Token::KwUsize
                    | Token::KwF32
                    | Token::KwF64
                    | Token::KwBool
                    | Token::KwChar
                    | Token::KwRawptr
                    | Token::KwVoid
                    | Token::KwNever => SemanticToken::Type,
                    _ => continue,
                };

                let line_ranges = session
                    .pkg_storage
                    .module(builder.module_id)
                    .line_ranges
                    .as_slice();
                let mut delta_line: u32 = 0;

                while range.start() >= line_ranges[builder.curr_line as usize].end() {
                    builder.curr_line += 1;
                    delta_line += 1;
                }

                let delta_start = if delta_line == 0 {
                    if let Some(&prev_id) = builder.token_ids.last() {
                        let prev_range = tree.tokens().token_range(prev_id);
                        range.start() - prev_range.start()
                    } else {
                        range.start() - line_ranges[builder.curr_line as usize].start()
                    }
                } else {
                    range.start() - line_ranges[builder.curr_line as usize].start()
                };

                builder.token_ids.push(token_id);
                builder.tokens.push(lsp::SemanticToken {
                    delta_line,
                    delta_start: delta_start.into(),
                    length: range.len(), //@byte length or utf8 chars len?
                    token_type: semantic as u32,
                    token_modifiers_bitset: 0,
                });
            }
            NodeOrToken::Trivia(id) => {
                let (trivia, range) = tree.tokens().trivia_and_range(id);

                //@block comment is an overlapping token
                let semantic = match trivia {
                    Trivia::Whitespace => continue,
                    Trivia::LineComment | Trivia::BlockComment => SemanticToken::Comment,
                };
                //@test comments later
            }
        }
    }
}
