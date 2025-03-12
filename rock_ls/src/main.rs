#![forbid(unsafe_code)]

mod message;
mod text_ops;

use lsp_server::{Connection, RequestId};
use lsp_types as lsp;
use message::{Action, Message, MessageBuffer, Notification, Request};
use rock_core::intern::{InternPool, NameID};
use rock_core::session::FileData;
use rock_core::support::Timer;
use rock_core::syntax::ast_layer::{self as cst, AstNode};
use rock_core::syntax::format::FormatterCache;
use rock_core::syntax::syntax_kind::SyntaxKind;
use rock_core::syntax::syntax_tree::{Node, NodeOrToken, SyntaxTree};
use rock_core::syntax::token::{SemanticToken, Token, Trivia};
use std::collections::HashMap;

fn main() {
    let (conn, threads) = Connection::stdio();
    let _ = initialize_handshake(&conn);
    if let Ok(mut server) = initialize_server(&conn) {
        handle_compile_project(&mut server);
        server_loop(&mut server);
    }
    threads.join().expect("io join");
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
    conn: &'s Connection,
    session: Session<'s>,
    modules: Vec<ModuleData>,
    fmt_cache: FormatterCache,
}

struct ModuleData {
    symbols_version: u32,
    symbols: HashMap<NameID, SemanticToken>,
}

fn initialize_server(conn: &Connection) -> Result<ServerContext, ()> {
    use rock_core::config::{BuildKind, Config, TargetTriple};
    let config = Config::new(TargetTriple::host(), BuildKind::Debug);

    let session = match session::create_session(config) {
        Ok(value) => value,
        Err(error) => {
            let message = error.diagnostic().msg().as_str().to_string();
            let params = lsp::ShowMessageParams { typ: lsp::MessageType::ERROR, message };
            send_notification::<lsp::notification::ShowMessage>(conn, params);
            return Err(());
        }
    };

    let mut modules = Vec::with_capacity(session.module.count());
    for _ in session.module.ids() {
        let data = ModuleData { symbols_version: 0, symbols: HashMap::new() };
        modules.push(data);
    }

    let server = ServerContext { conn, session, modules, fmt_cache: FormatterCache::new() };
    Ok(server)
}

fn server_loop(server: &mut ServerContext) {
    let mut buffer = MessageBuffer::new();
    loop {
        match buffer.receive(server.conn) {
            Action::Collect => continue,
            Action::Shutdown => break,
            Action::Handle(messages) => handle_messages(server, messages),
        }
    }
}

fn handle_messages(server: &mut ServerContext, messages: Vec<Message>) {
    eprintln!("\n====================");
    eprintln!("[info] handling {} messages:", messages.len());
    eprintln!("====================\n");

    for message in messages {
        match message {
            Message::Request(id, req) => handle_request(server, id.clone(), req),
            Message::Notification(not) => handle_notification(server, not),
        }
    }
}

fn handle_request(server: &mut ServerContext, id: RequestId, req: Request) {
    match &req {
        Request::Format(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::Format\n - document: {:?}", &path);

            let session = &mut server.session;
            let module_id = match module_id_from_path(session, &path) {
                Some(module_id) => module_id,
                None => {
                    eprintln!(" - module not found by path");
                    send_response(server.conn, id, Vec::<lsp::TextEdit>::new());
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
                eprintln!(" - tree is incomplete"); //@remove?
                send_response(server.conn, id, Vec::<lsp::TextEdit>::new());
                return;
            }
            let file = session.vfs.file(module.file_id());
            let formatted = rock_core::syntax::format::format(
                tree,
                &file.source,
                &file.line_ranges,
                &mut server.fmt_cache,
            );

            //@hack overshoot by 1 line to ignore last line chars
            let end_line = file.line_ranges.len() as u32 + 1;
            let edit_start = lsp::Position::new(0, 0);
            let edit_end = lsp::Position::new(end_line, 0);
            let edit_range = lsp::Range::new(edit_start, edit_end);

            let text_edit = lsp::TextEdit::new(edit_range, formatted);
            send_response(server.conn, id, vec![text_edit]);
        }
        Request::SemanticTokens(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::SemanticTokens\n - document: {:?}", &path);

            let session = &mut server.session;
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

            //@always updating, so far sufficient
            let root = cst::SourceFile::cast(tree.root()).unwrap();
            let data = &mut server.modules[module_id.index()];
            data.symbols.clear();
            data.symbols.reserve(root.0.content.len());

            for item in root.items(&tree) {
                fn name_id(
                    pool: &mut InternPool<NameID>,
                    tree: &SyntaxTree,
                    file: &FileData,
                    name: cst::Name,
                ) -> NameID {
                    let name_range = name.ident(tree).unwrap();
                    let name_text = &file.source[name_range.as_usize()];
                    pool.intern(name_text)
                }

                match item {
                    cst::Item::Proc(item) => {
                        if let Some(name) = item.name(&tree) {
                            let id = name_id(&mut session.intern_name, &tree, file, name);
                            data.symbols.insert(id, SemanticToken::Function);
                        }
                    }
                    cst::Item::Enum(item) => {
                        if let Some(name) = item.name(&tree) {
                            let id = name_id(&mut session.intern_name, &tree, file, name);
                            data.symbols.insert(id, SemanticToken::Type);
                        }
                    }
                    cst::Item::Struct(item) => {
                        if let Some(name) = item.name(&tree) {
                            let id = name_id(&mut session.intern_name, &tree, file, name);
                            data.symbols.insert(id, SemanticToken::Type);
                        }
                    }
                    cst::Item::Const(item) => {
                        if let Some(name) = item.name(&tree) {
                            let id = name_id(&mut session.intern_name, &tree, file, name);
                            data.symbols.insert(id, SemanticToken::Variable);
                        }
                    }
                    cst::Item::Global(item) => {
                        if let Some(name) = item.name(&tree) {
                            let id = name_id(&mut session.intern_name, &tree, file, name);
                            data.symbols.insert(id, SemanticToken::Variable);
                        }
                    }
                    cst::Item::Import(item) => {
                        if let Some(rename) = item.rename(&tree) {
                            if let Some(name) = rename.alias(&tree) {
                                let id = name_id(&mut session.intern_name, &tree, file, name);
                                data.symbols.insert(id, SemanticToken::Namespace);
                            }
                        } else if let Some(path) = item.import_path(&tree) {
                            if let Some(name) = path.names(&tree).last() {
                                let id = name_id(&mut session.intern_name, &tree, file, name);
                                data.symbols.insert(id, SemanticToken::Namespace);
                            }
                        }
                        if let Some(symbol_list) = item.import_symbol_list(&tree) {
                            for symbol in symbol_list.import_symbols(&tree) {
                                if let Some(name) = symbol.name(&tree) {
                                    let id = name_id(&mut session.intern_name, &tree, file, name);
                                }
                                if let Some(rename) = symbol.rename(&tree) {
                                    if let Some(name) = rename.alias(&tree) {
                                        //
                                    }
                                }
                            }
                        }
                    }
                    cst::Item::Directive(_) => continue,
                };
            }

            let timer = Timer::start();
            let data = semantic_tokens(server, &tree, module_id);
            eprintln!("[semantic tokens] ms: {}, count: {}", timer.measure_ms(), data.len());
            send_response(server.conn, id, lsp::SemanticTokens { result_id: None, data });
            server.session.module.get_mut(module_id).set_tree(tree);
        }
        Request::ShowSyntaxTree(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::ShowSyntaxTree\n - document: {:?}", &path);

            let session = &mut server.session;
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
            send_response(server.conn, id, tree_display);
        }
    }
}

fn handle_notification(server: &mut ServerContext, not: Notification) {
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
        Notification::FileSaved(_) => {
            handle_compile_project(server);
        }
        Notification::FileChanged(path, changes) => {
            eprintln!(
                "[HANDLE] Notification::SourceFileChanged: {:?} changes: {}",
                &path,
                changes.len()
            );
            let session = &mut server.session;
            let module = match module_id_from_path(session, &path) {
                Some(module_id) => session.module.get(module_id),
                None => {
                    eprintln!(" - module not found");
                    return;
                }
            };

            let file = session.vfs.file_mut(module.file_id());
            file.version += 1;
            eprintln!(
                "[info] file changed, path: `{}`, version: `{}`",
                file.path.to_string_lossy(),
                file.version
            );
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

fn handle_compile_project(server: &mut ServerContext) {
    eprintln!("[Handle] CompileProject");

    use std::time::Instant;
    let start_time = Instant::now();
    let publish_diagnostics = run_diagnostics(server);
    let elapsed_time = start_time.elapsed();
    eprintln!("run diagnostics: {} ms", elapsed_time.as_secs_f64() * 1000.0);

    for publish in publish_diagnostics.iter() {
        send_notification::<lsp::notification::PublishDiagnostics>(server.conn, publish);
    }
}

fn send_response(conn: &Connection, id: RequestId, result: impl serde::Serialize) {
    let result = Some(serde_json::to_value(result).unwrap());
    let response = lsp_server::Response { id, result, error: None };
    send(conn, response);
}

fn send_notification<N: lsp::notification::Notification>(
    conn: &Connection,
    params: impl serde::Serialize,
) {
    let params = serde_json::to_value(params).unwrap();
    let notification = lsp_server::Notification { method: N::METHOD.to_string(), params };
    send(conn, notification);
}

fn send<Content: Into<lsp_server::Message>>(conn: &Connection, msg: Content) {
    conn.sender.send(msg.into()).unwrap();
}

use rock_core::error::{Diagnostic, DiagnosticData, Severity, SourceRange};
use rock_core::hir_lower;
use rock_core::session::{self, ModuleID, Session};
use rock_core::syntax;
use rock_core::text::{self, TextRange};

use lsp::{DiagnosticRelatedInformation, Location, Position, PublishDiagnosticsParams, Range};
use std::path::PathBuf;

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
    if let Some(file_id) = session.vfs.path_to_file_id(path) {
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

fn source_to_range_and_path<'s, 'sref: 's>(
    session: &'sref Session<'s>,
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

    (range, &file.path)
}

fn create_diagnostic(
    session: &Session,
    diagnostic: &Diagnostic,
    severity: Severity,
) -> Option<lsp::Diagnostic> {
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

    Some(diagnostic)
}

fn run_diagnostics(server: &mut ServerContext) -> Vec<PublishDiagnosticsParams> {
    let session = &mut server.session;
    for module_id in session.module.ids() {
        let module = session.module.get_mut(module_id);
        module.errors.clear();
    }

    let _ = check_impl(session);
    let mut publish_diagnostics = Vec::with_capacity(session.module.count());

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());

        let capacity = module.errors.errors.len() + module.errors.warnings.len();
        let mut diagnostics = Vec::with_capacity(capacity);

        for error in &module.errors.errors {
            if let Some(d) = create_diagnostic(session, error.diagnostic(), Severity::Error) {
                diagnostics.push(d);
            }
        }
        for warning in &module.errors.warnings {
            if let Some(d) = create_diagnostic(session, warning.diagnostic(), Severity::Warning) {
                diagnostics.push(d);
            }
        }

        let publish = PublishDiagnosticsParams::new(url_from_path(&file.path), diagnostics, None);
        publish_diagnostics.push(publish);
    }

    publish_diagnostics
}

fn check_impl(session: &mut Session) -> Result<(), ()> {
    syntax::parse_all_lsp(session, true)?;
    hir_lower::check(session)?;
    Ok(())
}

struct SemanticTokenBuilder {
    module_id: ModuleID,
    curr_line: u32,
    prev_range: Option<TextRange>,
    params_in_scope: Vec<NameID>,
    semantic_tokens: Vec<lsp::SemanticToken>,
}

fn semantic_tokens(
    server: &mut ServerContext,
    tree: &SyntaxTree,
    module_id: ModuleID,
) -> Vec<lsp::SemanticToken> {
    let mut builder = SemanticTokenBuilder {
        module_id,
        curr_line: 0,
        prev_range: None,
        params_in_scope: Vec::with_capacity(16),
        semantic_tokens: Vec::with_capacity(tree.tokens().token_count() / 2), //@estimate better count
    };

    semantic_visit_node(server, &mut builder, tree.root(), tree, None);
    builder.semantic_tokens
}

fn name_id(
    pool: &mut InternPool<NameID>,
    tree: &SyntaxTree,
    file: &FileData,
    name: cst::Name,
) -> NameID {
    let name_range = name.ident(tree).unwrap();
    let name_text = &file.source[name_range.as_usize()];
    pool.intern(name_text)
}

fn semantic_visit_path(
    server: &mut ServerContext,
    builder: &mut SemanticTokenBuilder,
    path: cst::Path,
    tree: &SyntaxTree,
    parent: SyntaxKind,
) {
    let module = server.session.module.get(builder.module_id);
    let file = server.session.vfs.file(module.file_id());
    let data = &server.modules[builder.module_id.index()];
    let mut segments = path.segments(tree);

    if let Some(first) = segments.next() {
        let style = if let Some(name) = first.name(tree) {
            let id = name_id(&mut server.session.intern_name, tree, file, name);
            let mut symbol = data.symbols.get(&id).copied();
            if symbol.is_none() {
                if builder.params_in_scope.iter().any(|&n| n == id) {
                    symbol = Some(SemanticToken::Parameter);
                }
            }
            symbol
        } else {
            None
        };
        semantic_visit_node(server, builder, first.0, tree, style);
    }

    for segment in segments.by_ref() {
        let style =
            if parent == SyntaxKind::TYPE_CUSTOM { Some(SemanticToken::Type) } else { None };
        semantic_visit_node(server, builder, segment.0, tree, style);
    }

    // TYPE_CUSTOM
    // EXPR_ITEM
    // EXPR_STRUCT_INIT
    // PAT_ITEM
}

fn semantic_visit_node(
    server: &mut ServerContext,
    builder: &mut SemanticTokenBuilder,
    node: &Node,
    tree: &SyntaxTree,
    ident_style: Option<SemanticToken>,
) {
    let parent = node.kind;

    if cst::Item::cast(node).is_some() {
        builder.params_in_scope.clear();
    }
    if let Some(params) = cst::ParamList::cast(node) {
        for param in params.params(tree) {
            if let Some(name) = param.name(tree) {
                //@repetative way to get name_id
                let module = server.session.module.get(builder.module_id);
                let file = server.session.vfs.file(module.file_id());
                let id = name_id(&mut server.session.intern_name, tree, file, name);
                builder.params_in_scope.push(id);
            }
        }
    }

    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                let node = tree.node(node_id);

                if let Some(path) = cst::Path::cast(node) {
                    semantic_visit_path(server, builder, path, tree, parent);
                    continue;
                }

                let ident_style = match node.kind {
                    SyntaxKind::PROC_ITEM => Some(SemanticToken::Function),
                    SyntaxKind::PARAM => Some(SemanticToken::Parameter),
                    SyntaxKind::ENUM_ITEM => Some(SemanticToken::Type),
                    SyntaxKind::VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::STRUCT_ITEM => Some(SemanticToken::Type),
                    SyntaxKind::FIELD => Some(SemanticToken::Property),
                    SyntaxKind::CONST_ITEM => Some(SemanticToken::Variable),
                    SyntaxKind::GLOBAL_ITEM => Some(SemanticToken::Variable),
                    SyntaxKind::IMPORT_ITEM => Some(SemanticToken::Namespace),
                    SyntaxKind::IMPORT_PATH => Some(SemanticToken::Namespace),
                    SyntaxKind::IMPORT_SYMBOL => None, //@todo based on symbol
                    SyntaxKind::IMPORT_SYMBOL_RENAME => None, //@todo based on symbol

                    SyntaxKind::DIRECTIVE_PARAM => Some(SemanticToken::Variable),
                    SyntaxKind::BUILTIN_ERROR => Some(SemanticToken::Function),
                    SyntaxKind::BUILTIN_WITH_TYPE => Some(SemanticToken::Function),
                    SyntaxKind::BUILTIN_TRANSMUTE => Some(SemanticToken::Function),

                    SyntaxKind::TYPE_CUSTOM => Some(SemanticToken::Type),

                    SyntaxKind::FOR_BIND => Some(SemanticToken::Variable),
                    SyntaxKind::STMT_LOCAL => Some(SemanticToken::Variable),

                    SyntaxKind::EXPR_FIELD => Some(SemanticToken::Property),
                    SyntaxKind::EXPR_CALL => None, //@todo path
                    SyntaxKind::EXPR_ITEM => None, //@todo path
                    SyntaxKind::EXPR_VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::EXPR_STRUCT_INIT => None, //@todo path
                    SyntaxKind::FIELD_INIT => Some(SemanticToken::Property),

                    SyntaxKind::PAT_ITEM => None, //@todo path
                    SyntaxKind::PAT_VARIANT => Some(SemanticToken::EnumMember),

                    SyntaxKind::NAME => ident_style,
                    SyntaxKind::BIND => Some(SemanticToken::Variable),
                    SyntaxKind::POLYMORPH_PARAMS => Some(SemanticToken::Type),
                    _ => None,
                };

                semantic_visit_node(server, builder, node, tree, ident_style);
            }
            NodeOrToken::Token(token_id) => {
                let (token, range) = tree.tokens().token_and_range(token_id);
                if let Some(semantic) = semantic_token_style(token, node.kind, ident_style) {
                    semantic_token_add(server, builder, semantic, range);
                }
            }
            NodeOrToken::Trivia(id) => {
                let (trivia, range) = tree.tokens().trivia_and_range(id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment | Trivia::DocComment | Trivia::ModComment => {
                        semantic_token_add(server, builder, SemanticToken::Comment, range)
                    }
                };
            }
        }
    }
}

fn semantic_token_add(
    server: &ServerContext,
    builder: &mut SemanticTokenBuilder,
    semantic: SemanticToken,
    range: TextRange,
) {
    let module = server.session.module.get(builder.module_id);
    let file = server.session.vfs.file(module.file_id());
    let source = &file.source;
    let line_ranges = file.line_ranges.as_slice();

    let mut delta_line: u32 = 0;
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
    let token_str = &source[delta_range.as_usize()];
    let delta_start = text_ops::str_char_len_utf16(token_str);

    let token_str = &source[range.as_usize()];
    let length = text_ops::str_char_len_utf16(token_str);

    builder.prev_range = Some(range);
    builder.semantic_tokens.push(lsp::SemanticToken {
        delta_line,
        delta_start,
        length,
        token_type: semantic as u32,
        token_modifiers_bitset: 0,
    });
}

fn semantic_token_style(
    token: Token,
    parent: SyntaxKind,
    ident_style: Option<SemanticToken>,
) -> Option<SemanticToken> {
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
        T![if] | T![else] | T![match] | T![as] => SemanticToken::Keyword,
        T![_] => SemanticToken::Parameter,

        T![s8] | T![s16] | T![s32] | T![s64] | T![ssize] |
        T![u8] | T![u16] | T![u32] | T![u64] | T![usize] |
        T![f32] | T![f64] | T![bool] | T![bool16] | T![bool32] | T![bool64] | T![char] |
        T![rawptr] | T![never] | T![string] | T![cstring] => SemanticToken::Type,
        T![void] if parent == SyntaxKind::TYPE_BASIC => SemanticToken::Type,
        T![void] => SemanticToken::Number,

        T![.] | T![,] | T![:] | T![;] | T![#] => return None,
        T![@] => SemanticToken::Function,
        T!['('] | T![')'] | T!['['] | T![']'] | T!['{'] | T!['}'] => return None,

        T![..] | T![->] | T!["..<"] | T!["..="] | T![~] | T![!] |
        T![+] | T![-] | T![*] | T![/] | T![%] | T![&] | T![|] | T![^] | T![<<] | T![>>] |
        T![==] | T![!=] | T![<] | T![<=] | T![>] | T![>=] | T![&&] | T![||] |
        T![=] | T![+=] | T![-=] | T![*=] | T![/=] | T![%=] | 
        T![&=] | T![|=] | T![^=] | T![<<=] | T![>>=] => SemanticToken::Operator,
    };
    Some(semantic)
}
