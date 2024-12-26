#![forbid(unsafe_code)]

mod message;
mod text_ops;

use lsp_server::{Connection, RequestId};
use lsp_types as lsp;
use lsp_types::notification::{self, Notification as NotificationTrait};
use message::{Action, Message, MessageBuffer, Notification, Request};
use rock_core::intern::{InternPool, NameID};
use rock_core::session::FileData;
use rock_core::support::Timer;
use rock_core::syntax::format::FormatterCache;
use rock_core::syntax::syntax_kind::SyntaxKind;
use rock_core::syntax::syntax_tree::{Node, NodeOrToken, SyntaxTree};
use rock_core::token::{SemanticToken, Token, Trivia};
use std::collections::HashMap;

fn main() {
    if !check_args() {
        return;
    };
    let (conn, threads) = Connection::stdio();
    let _ = initialize_handshake(&conn);
    server_loop(&conn);
    threads.join().expect("io joined");
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
    let document_sync = lsp::TextDocumentSyncOptions {
        open_close: Some(true),
        change: Some(lsp::TextDocumentSyncKind::INCREMENTAL),
        will_save: None,
        will_save_wait_until: None,
        save: Some(lsp::TextDocumentSyncSaveOptions::SaveOptions(lsp::SaveOptions {
            include_text: None,
        })),
    };

    let semantic_tokens = lsp::SemanticTokensOptions {
        work_done_progress_options: lsp::WorkDoneProgressOptions { work_done_progress: None },
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
                lsp::SemanticTokenType::OPERATOR,
            ],
            token_modifiers: vec![],
        },
        range: None,
        full: Some(lsp::SemanticTokensFullOptions::Bool(true)),
    };

    let capabilities = lsp::ServerCapabilities {
        text_document_sync: Some(lsp::TextDocumentSyncCapability::Options(document_sync)),
        document_formatting_provider: Some(lsp::OneOf::Left(true)),
        semantic_tokens_provider: Some(
            lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(semantic_tokens),
        ),
        ..Default::default()
    };

    let init_params =
        conn.initialize(into_json(capabilities)).expect("internal: initialize failed");
    from_json(init_params)
}

#[track_caller]
fn into_json<T: serde::Serialize>(value: T) -> serde_json::Value {
    match serde_json::to_value(value) {
        Ok(value) => value,
        Err(error) => {
            let loc = core::panic::Location::caller();
            panic!("internal: json serialize failed at: {loc}\n{error}");
        }
    }
}

#[track_caller]
fn from_json<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    match serde_json::from_value(value) {
        Ok(value) => value,
        Err(error) => {
            let loc = core::panic::Location::caller();
            panic!("internal: json deserialize failed at: {loc}\n{error}");
        }
    }
}

struct ServerContext<'s> {
    session: Option<Session<'s>>,
    fmt_cache: FormatterCache,
}

impl<'s> ServerContext<'s> {
    fn new() -> ServerContext<'s> {
        ServerContext { session: None, fmt_cache: FormatterCache::new() }
    }
}

fn server_loop(conn: &Connection) {
    let mut buffer = MessageBuffer::new();
    let mut context = ServerContext::new();
    handle_compile_project(conn, &mut context);

    loop {
        match buffer.receive(conn) {
            Action::Collect => continue,
            Action::Handle(messages) => handle_messages(conn, &mut context, messages),
            Action::Shutdown => break,
        }
    }
}

fn handle_messages(conn: &Connection, context: &mut ServerContext, messages: Vec<Message>) {
    eprintln!("\n====================");
    eprintln!("[HANDLE MESSAGES] {}", messages.len());
    for message in &messages {
        match message {
            Message::Request(_, request) => match request {
                Request::Format(_) => eprintln!(" - Request::Format"),
                Request::SemanticTokens(_) => eprintln!(" - Request::SemanticTokens"),
                Request::ShowSyntaxTree(_) => eprintln!(" - Request::ShowSyntaxTree"),
            },
            Message::Notification(not) => match not {
                Notification::FileOpened { .. } => eprintln!(" - Notification::FileOpened"),
                Notification::FileChanged { .. } => eprintln!(" - Notification::FileChanged"),
                Notification::FileSaved { .. } => eprintln!(" - Notification::FileSaved"),
                Notification::FileClosed { .. } => eprintln!(" - Notification::FileClosed"),
            },
        }
    }
    eprintln!("====================\n");

    for message in messages {
        match message {
            Message::Request(id, req) => handle_request(conn, context, id.clone(), req),
            Message::Notification(not) => handle_notification(conn, context, not),
        }
    }
}

fn handle_request(conn: &Connection, context: &mut ServerContext, id: RequestId, req: Request) {
    match &req {
        Request::Format(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::Format\n - document: {:?}", &path);

            let session = match &mut context.session {
                Some(session) => session,
                None => {
                    eprintln!(" - session is None");
                    send_response_format_error(conn, id);
                    return;
                }
            };
            let module_id = match module_id_from_path(session, &path) {
                Some(module_id) => module_id,
                None => {
                    eprintln!(" - module not found by path");
                    send_response_format_error(conn, id);
                    return;
                }
            };

            //@hack always update the syntax tree before format
            let module = session.module.get(module_id);
            let file = session.vfs.file(module.file_id());
            let (tree, errors) =
                syntax::parse_tree(&file.source, module_id, true, &mut session.intern_lit);
            let _ = errors.collect();

            let module = session.module.get_mut(module_id);
            module.set_tree(tree);

            let tree = module.tree_expect();
            if !tree.complete() {
                eprintln!(" - tree is incomplete");
                send_response_format_error(conn, id);
                return;
            }
            let file = session.vfs.file(module.file_id());
            let formatted = rock_core::syntax::format::format(
                tree,
                &file.source,
                &file.line_ranges,
                &mut context.fmt_cache,
            );

            //@hack overshoot by 1 line to ignore last line chars
            let end_line = file.line_ranges.len() as u32 + 1;
            let edit_start = lsp::Position::new(0, 0);
            let edit_end = lsp::Position::new(end_line, 0);
            let edit_range = lsp::Range::new(edit_start, edit_end);

            let text_edit = lsp::TextEdit::new(edit_range, formatted);
            let json = serde_json::to_value(vec![text_edit]).unwrap();
            send_response(conn, id, json);
        }
        Request::SemanticTokens(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::SemanticTokens\n - document: {:?}", &path);

            let session = match &mut context.session {
                Some(session) => session,
                None => {
                    eprintln!(" - session is None");
                    return;
                }
            };

            let module_id = match module_id_from_path(session, &path) {
                Some(module_id) => module_id,
                None => {
                    eprintln!(" - module not found by path");
                    return;
                }
            };

            //@hack always update the syntax tree before semantic tokens
            let module = session.module.get(module_id);
            let file = session.vfs.file(module.file_id());
            let (tree, errors) =
                syntax::parse_tree(&file.source, module_id, true, &mut session.intern_lit);
            let _ = errors.collect();

            let module = session.module.get_mut(module_id);
            module.set_tree(tree);
            let tree = module.tree_expect();

            let timer = Timer::start();
            let semantic_tokens = semantic_tokens(&mut session.intern_name, file, tree);
            eprintln!("semantic tokens: {}", timer.measure_ms());

            eprintln!("[SEND: Response] SemanticTokens ({})", semantic_tokens.len());
            let result = lsp::SemanticTokens { result_id: None, data: semantic_tokens };
            send(conn, lsp_server::Response::new_ok(id, result));
        }
        Request::ShowSyntaxTree(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::ShowSyntaxTree\n - document: {:?}", &path);

            let session = match &mut context.session {
                Some(session) => session,
                None => {
                    eprintln!(" - session is None");
                    return;
                }
            };

            let module_id = match module_id_from_path(session, &path) {
                Some(module_id) => module_id,
                None => {
                    eprintln!(" - module not found by path");
                    return;
                }
            };

            //@hack always update the syntax tree
            let module = session.module.get(module_id);
            let file = session.vfs.file(module.file_id());
            let (tree, errors) =
                syntax::parse_tree(&file.source, module_id, true, &mut session.intern_lit);
            let _ = errors.collect();

            let module = session.module.get_mut(module_id);
            module.set_tree(tree);
            let tree = module.tree_expect();

            let tree_display = syntax::syntax_tree::tree_display(tree, &file.source);
            eprintln!("[SEND: Response] ShowSyntaxTree len: {}", tree_display.len());
            send(conn, lsp_server::Response::new_ok(id, tree_display));
        }
    }
}

fn handle_notification(conn: &Connection, context: &mut ServerContext, not: Notification) {
    match not {
        Notification::FileOpened(path, text) => {
            //@handle file open, send when:
            // 1) new file created
            // 2) existing file renamed
            // 3) file opened in the editor
        }
        Notification::FileClosed(path) => {
            //@handle file closed, sent when:
            // 1) existing file deleted
            // 2) existing file renamed
            // 3) file closed in the editor
        }
        Notification::FileSaved(path) => {
            handle_compile_project(conn, context);
        }
        Notification::FileChanged(path, changes) => {
            eprintln!(
                "[HANDLE] Notification::SourceFileChanged: {:?} changes: {}",
                &path,
                changes.len()
            );
            let session = match &mut context.session {
                Some(session) => session,
                None => {
                    eprintln!(" - session is missing");
                    return;
                }
            };
            let module = match module_id_from_path(session, &path) {
                Some(module_id) => session.module.get(module_id),
                None => {
                    eprintln!(" - module not found");
                    return;
                }
            };

            let file = session.vfs.file_mut(module.file_id());
            for change in changes {
                if let Some(range) = change.range {
                    let range = text_ops::file_range_to_text_range(file, range);
                    file.source.replace_range(range.as_usize(), &change.text);
                    file.line_ranges = text::find_line_ranges(&file.source); //@make incremental
                } else {
                    file.source = change.text;
                    file.line_ranges = text::find_line_ranges(&file.source);
                }
            }
        }
    }
}

fn handle_compile_project(conn: &Connection, context: &mut ServerContext) {
    eprintln!("[Handle] CompileProject");

    use std::time::Instant;
    let start_time = Instant::now();
    let publish_diagnostics = run_diagnostics(conn, context);
    let elapsed_time = start_time.elapsed();
    eprintln!("run diagnostics: {} ms", elapsed_time.as_secs_f64() * 1000.0);

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

fn send_response_format_error(conn: &Connection, id: RequestId) {
    let no_edits = serde_json::to_value::<Vec<lsp::TextEdit>>(vec![]).unwrap();
    let response = lsp_server::Response::new_ok(id, no_edits);
    send(conn, response);
}

fn send_response_error(conn: &Connection, id: RequestId, message: String) {
    let response =
        lsp_server::Response::new_err(id, lsp_server::ErrorCode::RequestFailed as i32, message);
    send(conn, response);
}

fn send<Content: Into<lsp_server::Message>>(conn: &Connection, msg: Content) {
    conn.sender.send(msg.into()).expect("send message");
}

use rock_core::error::{
    Diagnostic, DiagnosticData, ErrorWarningBuffer, Severity, SourceRange, WarningBuffer,
};
use rock_core::session::{self, ModuleID, Session};
use rock_core::syntax::ast_build;
use rock_core::text::{self, TextRange};
use rock_core::{hir_lower, syntax};

use lsp::{DiagnosticRelatedInformation, Location, Position, PublishDiagnosticsParams, Range};
use std::path::PathBuf;

fn check_impl(session: &mut Session) -> Result<WarningBuffer, ErrorWarningBuffer> {
    ast_build::parse_all(session, true)?;
    let (_, warnings) = hir_lower::check(session)?;
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

fn module_id_from_path(session: &Session, path: &PathBuf) -> Option<ModuleID> {
    if let Some(file_id) = session.vfs.path_to_file_id(&path) {
        for module_id in session.module.ids() {
            if session.module.get(module_id).file_id() == file_id {
                return Some(module_id);
            }
        }
    }
    None
}

fn severity_convert(severity: Severity) -> Option<lsp::DiagnosticSeverity> {
    match severity {
        Severity::Info => Some(lsp::DiagnosticSeverity::HINT),
        Severity::Error => Some(lsp::DiagnosticSeverity::ERROR),
        Severity::Warning => Some(lsp::DiagnosticSeverity::WARNING),
    }
}

fn source_to_range_and_path<'s, 's_ref: 's>(
    session: &'s_ref Session<'s>,
    source: SourceRange,
) -> (Range, &'s PathBuf) {
    let module = session.module.get(source.module_id());
    let file = session.vfs.file(module.file_id());

    let start_location =
        text::find_text_location(&file.source, source.range().start(), &file.line_ranges);
    let end_location =
        text::find_text_location(&file.source, source.range().end(), &file.line_ranges);

    let range = Range::new(
        Position::new(start_location.line() - 1, start_location.col() - 1),
        Position::new(end_location.line() - 1, end_location.col() - 1),
    );

    (range, file.path())
}

fn create_diagnostic<'src>(
    session: &'src Session,
    diagnostic: &Diagnostic,
    severity: Severity,
) -> Option<(lsp::Diagnostic, &'src PathBuf)> {
    let (main, related_info) = match diagnostic.data() {
        DiagnosticData::Message => return None, //@some diagnostic messages dont have source for example session errors or manifest errors
        DiagnosticData::Context { main, info } => {
            if let Some(info) = info {
                let (info_range, info_path) =
                    source_to_range_and_path(session, info.context().src());
                let related_info = DiagnosticRelatedInformation {
                    location: Location::new(url_from_path(info_path), info_range),
                    message: info.context().msg().to_string(),
                };
                (main, Some(vec![related_info]))
            } else {
                (main, None)
            }
        }
        DiagnosticData::ContextVec { main, info_vec } => {
            let mut related_infos = Vec::with_capacity(info_vec.len());
            for info in info_vec {
                let (info_range, info_path) =
                    source_to_range_and_path(session, info.context().src());
                let related_info = DiagnosticRelatedInformation {
                    location: Location::new(url_from_path(info_path), info_range),
                    message: info.context().msg().to_string(),
                };
                related_infos.push(related_info);
            }
            (main, Some(related_infos))
        }
    };

    let (main_range, main_path) = source_to_range_and_path(session, main.src());

    let mut message = diagnostic.msg().as_str().to_string();
    if !main.msg().is_empty() {
        message.push('\n');
        message += main.msg();
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

fn run_diagnostics(
    conn: &Connection,
    context: &mut ServerContext,
) -> Vec<PublishDiagnosticsParams> {
    //@not used now, only sending one session error
    let mut messages = Vec::<lsp::Diagnostic>::new();
    let mut diagnostics_map = HashMap::new();

    //re-use existing session else try to create it
    let session = if let Some(session) = &mut context.session {
        session
    } else {
        use rock_core::config::{BuildKind, Config, TargetTriple};
        let config = Config::new(TargetTriple::host(), BuildKind::Debug);

        let session = match session::create_session(config) {
            Ok(value) => value,
            Err(error) => {
                let params = lsp::ShowMessageParams {
                    typ: lsp::MessageType::ERROR,
                    message: error.diagnostic().msg().as_str().to_string(),
                };
                let not = lsp_server::Notification::new(
                    notification::ShowMessage::METHOD.to_string(),
                    params,
                );
                send(conn, not);
                return vec![];
            }
        };
        context.session = Some(session);
        context.session.as_mut().unwrap()
    };

    let (errors, warnings) = match check_impl(session) {
        Ok(warnings) => (vec![], warnings.collect()),
        Err(errw) => errw.collect(),
    };

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());
        diagnostics_map.insert(file.path().clone(), Vec::new());
    }

    // generate diagnostics
    for warning in warnings {
        if let Some((diagnostic, main_path)) =
            create_diagnostic(&session, warning.diagnostic(), Severity::Warning)
        {
            match diagnostics_map.get_mut(main_path) {
                Some(diagnostics) => diagnostics.push(diagnostic),
                None => {
                    diagnostics_map.insert(main_path.clone(), vec![diagnostic]);
                }
            }
        }
    }

    for error in errors {
        if let Some((diagnostic, main_path)) =
            create_diagnostic(&session, error.diagnostic(), Severity::Error)
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

struct SemanticTokenBuilder<'s_ref> {
    curr_line: u32,
    prev_range: Option<TextRange>,
    source: &'s_ref str,
    line_ranges: &'s_ref [TextRange],
    semantic_tokens: Vec<lsp::SemanticToken>,
    scope: ModuleScope,
}

fn semantic_tokens(
    intern_name: &mut InternPool<NameID>,
    file: &FileData,
    tree: &SyntaxTree,
) -> Vec<lsp::SemanticToken> {
    let mut builder = SemanticTokenBuilder {
        curr_line: 0,
        prev_range: None,
        source: file.source.as_str(),
        line_ranges: file.line_ranges.as_slice(),
        //@temp semantic token count estimate
        semantic_tokens: Vec::with_capacity(tree.tokens().token_count() / 2),
        scope: ModuleScope { symbols: HashMap::with_capacity(512) },
    };

    use rock_core::syntax::ast_layer::{self as cst, AstNode};
    let root = cst::SourceFile::cast(tree.root()).unwrap();

    for item in root.items(tree) {
        match item {
            cst::Item::Proc(item) => {
                if let Some(name) = item.name(tree) {
                    let range = name.ident(tree).unwrap();
                    let name_text = &builder.source[range.as_usize()];
                    let name_id = intern_name.intern(name_text);
                    builder.scope.symbols.insert(name_id, SyntaxKind::PROC_ITEM);
                }
            }
            cst::Item::Enum(item) => {
                if let Some(name) = item.name(tree) {
                    let range = name.ident(tree).unwrap();
                    let name_text = &builder.source[range.as_usize()];
                    let name_id = intern_name.intern(name_text);
                    builder.scope.symbols.insert(name_id, SyntaxKind::ENUM_ITEM);
                }
            }
            cst::Item::Struct(item) => {
                if let Some(name) = item.name(tree) {
                    let range = name.ident(tree).unwrap();
                    let name_text = &builder.source[range.as_usize()];
                    let name_id = intern_name.intern(name_text);
                    builder.scope.symbols.insert(name_id, SyntaxKind::STRUCT_ITEM);
                }
            }
            cst::Item::Const(item) => {
                if let Some(name) = item.name(tree) {
                    let range = name.ident(tree).unwrap();
                    let name_text = &builder.source[range.as_usize()];
                    let name_id = intern_name.intern(name_text);
                    builder.scope.symbols.insert(name_id, SyntaxKind::CONST_ITEM);
                }
            }
            cst::Item::Global(item) => {
                if let Some(name) = item.name(tree) {
                    let range = name.ident(tree).unwrap();
                    let name_text = &builder.source[range.as_usize()];
                    let name_id = intern_name.intern(name_text);
                    builder.scope.symbols.insert(name_id, SyntaxKind::GLOBAL_ITEM);
                }
            }
            cst::Item::Import(item) => {
                if let Some(path) = item.import_path(tree) {
                    if let Some(name) = path.names(tree).last() {
                        let range = name.ident(tree).unwrap();
                        let name_text = &builder.source[range.as_usize()];
                        let name_id = intern_name.intern(name_text);
                        builder.scope.symbols.insert(name_id, SyntaxKind::SOURCE_FILE);
                        //@"module"
                    }
                }
            }
            cst::Item::Directive(_) => {}
        }
    }

    semantic_visit_node(&mut builder, intern_name, tree, tree.root(), None);
    builder.semantic_tokens
}

fn semantic_visit_node(
    builder: &mut SemanticTokenBuilder,
    intern_name: &mut InternPool<NameID>,
    tree: &SyntaxTree,
    node: &Node,
    ident_style: Option<SemanticToken>,
) {
    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                let node = tree.node(node_id);

                use rock_core::syntax::ast_layer::{self as cst, AstNode};
                if let Some(path) = cst::Path::cast(node) {
                    let mut segments = path.segments(tree);

                    if let Some(segment) = segments.next() {
                        if let Some(name) = segment.name(tree) {
                            let range = name.ident(tree).unwrap();
                            let name_text = &builder.source[range.as_usize()];
                            let name_id = intern_name.intern(name_text);

                            let mut item_ident_style =
                                if let Some(&kind) = builder.scope.symbols.get(&name_id) {
                                    match kind {
                                        SyntaxKind::SOURCE_FILE => Some(SemanticToken::Namespace),
                                        SyntaxKind::PROC_ITEM => Some(SemanticToken::Function),
                                        SyntaxKind::ENUM_ITEM => Some(SemanticToken::Type),
                                        SyntaxKind::STRUCT_ITEM => Some(SemanticToken::Type),
                                        SyntaxKind::CONST_ITEM => Some(SemanticToken::Variable),
                                        SyntaxKind::GLOBAL_ITEM => Some(SemanticToken::Variable),
                                        _ => Some(SemanticToken::Property),
                                    }
                                } else {
                                    None
                                };
                            //@hack for Type::Custom
                            if let Some(SemanticToken::Type) = ident_style {
                                item_ident_style = ident_style;
                            }
                            semantic_visit_node(
                                builder,
                                intern_name,
                                tree,
                                name.0,
                                item_ident_style,
                            );
                        }
                        if let Some(poly_args) = segment.poly_args(tree) {
                            semantic_visit_node(builder, intern_name, tree, poly_args.0, None);
                        }
                    }

                    for segment in segments {
                        semantic_visit_node(
                            builder,
                            intern_name,
                            tree,
                            segment.0,
                            Some(ident_style.unwrap_or(SemanticToken::Property)),
                        );
                    }
                    continue;
                }

                let ident_style = match node.kind {
                    SyntaxKind::ERROR => None,
                    SyntaxKind::TOMBSTONE => None,
                    SyntaxKind::SOURCE_FILE => None,

                    SyntaxKind::PROC_ITEM => Some(SemanticToken::Function),
                    SyntaxKind::PARAM_LIST => None,
                    SyntaxKind::PARAM => Some(SemanticToken::Parameter),
                    SyntaxKind::ENUM_ITEM => Some(SemanticToken::Type),
                    SyntaxKind::VARIANT_LIST => None,
                    SyntaxKind::VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::VARIANT_FIELD_LIST => None,
                    SyntaxKind::STRUCT_ITEM => Some(SemanticToken::Type),
                    SyntaxKind::FIELD_LIST => None,
                    SyntaxKind::FIELD => Some(SemanticToken::Property),
                    SyntaxKind::CONST_ITEM => Some(SemanticToken::Variable),
                    SyntaxKind::GLOBAL_ITEM => Some(SemanticToken::Variable),
                    SyntaxKind::IMPORT_ITEM => Some(SemanticToken::Namespace),
                    SyntaxKind::IMPORT_PATH => Some(SemanticToken::Namespace),
                    SyntaxKind::IMPORT_SYMBOL_LIST => None,
                    SyntaxKind::IMPORT_SYMBOL => Some(SemanticToken::Type), //default to type
                    SyntaxKind::IMPORT_SYMBOL_RENAME => Some(SemanticToken::Type), //default to type

                    SyntaxKind::DIRECTIVE_LIST => None,
                    SyntaxKind::DIRECTIVE_SIMPLE => None,
                    SyntaxKind::DIRECTIVE_WITH_TYPE => None,
                    SyntaxKind::DIRECTIVE_WITH_PARAMS => None,
                    SyntaxKind::DIRECTIVE_PARAM_LIST => None,
                    SyntaxKind::DIRECTIVE_PARAM => Some(SemanticToken::Variable),

                    SyntaxKind::TYPE_BASIC => None,
                    SyntaxKind::TYPE_CUSTOM => Some(SemanticToken::Type),
                    SyntaxKind::TYPE_REFERENCE => None,
                    SyntaxKind::TYPE_MULTI_REFERENCE => None,
                    SyntaxKind::TYPE_PROCEDURE => None,
                    SyntaxKind::PROC_TYPE_PARAM_LIST => None,
                    SyntaxKind::TYPE_ARRAY_SLICE => None,
                    SyntaxKind::TYPE_ARRAY_STATIC => None,

                    SyntaxKind::BLOCK => None,
                    SyntaxKind::STMT_BREAK => None,
                    SyntaxKind::STMT_CONTINUE => None,
                    SyntaxKind::STMT_RETURN => None,
                    SyntaxKind::STMT_DEFER => None,
                    SyntaxKind::STMT_FOR => None,
                    SyntaxKind::FOR_BIND => Some(SemanticToken::Variable),
                    SyntaxKind::FOR_HEADER_COND => None,
                    SyntaxKind::FOR_HEADER_ELEM => None,
                    SyntaxKind::FOR_HEADER_PAT => None,
                    SyntaxKind::STMT_LOCAL => None,
                    SyntaxKind::STMT_ASSIGN => None,
                    SyntaxKind::STMT_EXPR_SEMI => None,
                    SyntaxKind::STMT_EXPR_TAIL => None,
                    SyntaxKind::STMT_WITH_DIRECTIVE => None,

                    SyntaxKind::EXPR_PAREN => None,
                    SyntaxKind::EXPR_IF => None,
                    SyntaxKind::IF_BRANCH => None,
                    SyntaxKind::EXPR_MATCH => None,
                    SyntaxKind::MATCH_ARM_LIST => None,
                    SyntaxKind::MATCH_ARM => None,
                    SyntaxKind::EXPR_FIELD => Some(SemanticToken::Property),
                    SyntaxKind::EXPR_INDEX => None,
                    SyntaxKind::EXPR_SLICE => None,
                    SyntaxKind::EXPR_CALL => None, //defer to path
                    SyntaxKind::EXPR_CAST => None,
                    SyntaxKind::EXPR_SIZEOF => None,
                    SyntaxKind::EXPR_ITEM => None,
                    SyntaxKind::EXPR_VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::EXPR_STRUCT_INIT => Some(SemanticToken::Type),
                    SyntaxKind::FIELD_INIT_LIST => None,
                    SyntaxKind::FIELD_INIT => Some(SemanticToken::Property),
                    SyntaxKind::EXPR_ARRAY_INIT => None,
                    SyntaxKind::EXPR_ARRAY_REPEAT => None,
                    SyntaxKind::EXPR_DEREF => None,
                    SyntaxKind::EXPR_ADDRESS => None,
                    SyntaxKind::EXPR_UNARY => None,
                    SyntaxKind::EXPR_BINARY => None,

                    SyntaxKind::PAT_WILD => None,
                    SyntaxKind::PAT_LIT => None,
                    SyntaxKind::PAT_ITEM => None,
                    SyntaxKind::PAT_VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::PAT_OR => None,

                    SyntaxKind::LIT_VOID => None,
                    SyntaxKind::LIT_NULL => None,
                    SyntaxKind::LIT_BOOL => None,
                    SyntaxKind::LIT_INT => None,
                    SyntaxKind::LIT_FLOAT => None,
                    SyntaxKind::LIT_CHAR => None,
                    SyntaxKind::LIT_STRING => None,

                    SyntaxKind::RANGE_FULL => None,
                    SyntaxKind::RANGE_TO_EXCLUSIVE => None,
                    SyntaxKind::RANGE_TO_INCLUSIVE => None,
                    SyntaxKind::RANGE_FROM => None,
                    SyntaxKind::RANGE_EXCLUSIVE => None,
                    SyntaxKind::RANGE_INCLUSIVE => None,

                    SyntaxKind::NAME => ident_style, // use pushed style
                    SyntaxKind::BIND => Some(SemanticToken::Variable),
                    SyntaxKind::BIND_LIST => None,
                    SyntaxKind::ARGS_LIST => None,
                    SyntaxKind::PATH => ident_style.or(Some(SemanticToken::Property)),
                    SyntaxKind::PATH_SEGMENT => ident_style.or(Some(SemanticToken::Property)),
                    SyntaxKind::POLYMORPH_ARGS => None,
                    SyntaxKind::POLYMORPH_PARAMS => Some(SemanticToken::Type),
                };

                semantic_visit_node(builder, intern_name, tree, node, ident_style);
            }
            //@color void differently (type vs void literal, number color?)
            NodeOrToken::Token(token_id) => {
                let (token, range) = tree.tokens().token_and_range(token_id);
                if let Some(semantic) = semantic_token_style(token, ident_style) {
                    semantic_token_add(builder, semantic, range);
                }
            }
            NodeOrToken::Trivia(id) => {
                let (trivia, range) = tree.tokens().trivia_and_range(id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment | Trivia::DocComment | Trivia::ModComment => {
                        semantic_token_add(builder, SemanticToken::Comment, range)
                    }
                };
            }
        }
    }
}

fn str_char_len_utf16(string: &str) -> u32 {
    string.chars().map(|c| c.len_utf16() as u32).sum()
}

fn semantic_token_add(
    builder: &mut SemanticTokenBuilder,
    semantic: SemanticToken,
    range: TextRange,
) {
    let mut delta_line: u32 = 0;
    let line_ranges = builder.line_ranges;

    while range.start() >= line_ranges[builder.curr_line as usize].end() {
        builder.curr_line += 1;
        delta_line += 1;
    }

    let (start, offset) = if delta_line == 0 {
        if let Some(prev_range) = builder.prev_range {
            let start = prev_range.start();
            (start, range.start() - start)
        } else {
            let start = line_ranges[builder.curr_line as usize].start();
            (start, range.start() - start)
        }
    } else {
        let start = line_ranges[builder.curr_line as usize].start();
        (start, range.start() - start)
    };

    let mut delta_range = TextRange::empty_at(start);
    delta_range.extend_by(offset);
    let token_str = &builder.source[delta_range.as_usize()];
    let delta_start = str_char_len_utf16(token_str);

    let token_str = &builder.source[range.as_usize()];
    let length = str_char_len_utf16(token_str);

    builder.prev_range = Some(range);
    builder.semantic_tokens.push(lsp::SemanticToken {
        delta_line,
        delta_start,
        length,
        token_type: semantic as u32,
        token_modifiers_bitset: 0,
    });
}

struct ModuleScope {
    symbols: HashMap<NameID, SyntaxKind>,
}

fn semantic_token_style(token: Token, ident_style: Option<SemanticToken>) -> Option<SemanticToken> {
    use rock_core::T;
    #[rustfmt::skip]
    let semantic = match token {
        T![eof] => return None,
        T![ident] => return ident_style,
        T![int_lit] | T![float_lit] => SemanticToken::Number,
        T![char_lit] | T![string_lit] => SemanticToken::String,

        T![proc] | T![enum] | T![struct] | T![import] |
        T![break] | T![continue] | T![return] | T![defer] | T![for] | T![in] |
        T![let] | T![mut] | T![zeroed] | T![undefined] => SemanticToken::Keyword,

        T![null] | T![true] | T![false] => SemanticToken::Number,
        T![if] | T![else] | T![match] | T![as] | T![sizeof] => SemanticToken::Keyword,
        T![_] => SemanticToken::Parameter,

        T![s8] | T![s16] | T![s32] | T![s64] | T![ssize] |
        T![u8] | T![u16] | T![u32] | T![u64] | T![usize] | 
        T![f32] | T![f64] | T![bool] | T![char] |
        T![rawptr] | T![void] | T![never] | T![string] | T![cstring] => SemanticToken::Type,

        T![.] | T![,] | T![:] | T![;] | T![#] |
        T!['('] | T![')'] | T!['['] | T![']'] | T!['{'] | T!['}'] => return None,

        T![..] | T![->] | T!["..<"] | T!["..="] | T![~] | T![!] |
        T![+] | T![-] | T![*] | T![/] | T![%] | T![&] | T![|] | T![^] | T![<<] | T![>>] |
        T![==] | T![!=] | T![<] | T![<=] | T![>] | T![>=] | T![&&] | T![||] |
        T![=] | T![+=] | T![-=] | T![*=] | T![/=] | T![%=] | 
        T![&=] | T![|=] | T![^=] | T![<<=] | T![>>=] => SemanticToken::Operator,
    };
    Some(semantic)
}
