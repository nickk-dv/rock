#![forbid(unsafe_code)]

mod message;

use lsp_server::{Connection, RequestId};
use lsp_types as lsp;
use lsp_types::notification::{self, Notification as NotificationTrait};
use message::{Action, Message, MessageBuffer, Notification, Request};
use rock_core::syntax::syntax_kind::SyntaxKind;
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
        position_encoding: None, //@vscode client crashes on init Some(lsp::PositionEncodingKind::UTF8),
        text_document_sync: Some(lsp::TextDocumentSyncCapability::Options(
            lsp::TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(lsp::TextDocumentSyncKind::FULL), //@switch to incremental
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

struct ServerContext<'s> {
    cache: FileCache,
    session: Option<Session<'s>>,
}

impl<'s> ServerContext<'s> {
    fn new() -> ServerContext<'s> {
        ServerContext {
            cache: FileCache::new(),
            session: None,
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
    eprintln!("\n====================");
    eprintln!("[HANDLE MESSAGES] {}", messages.len());
    for message in &messages {
        match message {
            Message::Request(_, request) => match request {
                Request::Completion(_) => eprintln!(" - Request::Completion"),
                Request::GotoDefinition(_) => eprintln!(" - Request::GotoDefinition"),
                Request::Format(_) => eprintln!(" - Request::Format"),
                Request::Hover(_) => eprintln!(" - Request::Hover"),
                Request::SemanticTokens(_) => eprintln!(" - Request::SemanticTokens"),
            },
            Message::Notification(not) => match not {
                Notification::SourceFileChanged { .. } => {
                    eprintln!(" - Notification::SourceFileChanged")
                }
                Notification::SourceFileClosed { .. } => {
                    eprintln!(" - Notification::SourceFileClosed")
                }
            },
            Message::CompileProject => eprintln!(" - CompileProject"),
        }
    }
    eprintln!("====================\n");

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
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::Format\n - document: {:?}", &path);

            let session = match &context.session {
                Some(session) => session,
                None => {
                    eprintln!(" - session is None");
                    return;
                }
            };
            let module_id = match session.pkg_storage.find_module_by_path(&path) {
                Some(module_id) => module_id,
                None => {
                    eprintln!(" - module not found by path");
                    return;
                }
            };

            if let Some(tree) = session.module_trees[module_id.raw_index()].as_ref() {
                let module = session.pkg_storage.module(module_id);
                let formatted = rock_core::format::format(tree, &module.source);
                context.cache.change(path, Some(formatted.clone())); //@hack

                //@hack overshoot by 1 line to ignore last line chars
                let end_line = module.line_ranges.len() as u32 + 1;
                let edit_start = lsp::Position::new(0, 0);
                let edit_end = lsp::Position::new(end_line, 0);
                let edit_range = lsp::Range::new(edit_start, edit_end);
                let text_edit = lsp::TextEdit::new(edit_range, formatted);

                let json = serde_json::to_value(vec![text_edit]).expect("json value");
                send_response(conn, id, json);
            } else {
                eprintln!(" - syntax tree is None");
            }
        }
        Request::Hover(params) => {
            let path = uri_to_path(&params.text_document_position_params.text_document.uri);
            eprintln!("[Handle] Request::Hover\n - document: {:?}", &path);
        }
        Request::SemanticTokens(params) => {
            let path = uri_to_path(&params.text_document.uri);
            eprintln!("[Handle] Request::SemanticTokens\n - document: {:?}", &path);

            let session = match &context.session {
                Some(session) => session,
                None => {
                    eprintln!(" - session is None");
                    return;
                }
            };
            let module_id = match session.pkg_storage.find_module_by_path(&path) {
                Some(module_id) => module_id,
                None => {
                    eprintln!(" - module not found by path");
                    return;
                }
            };

            //@should produce semantic tokens even for incomplete syntax trees
            if let Some(tree) = session.module_trees[module_id.raw_index()].as_ref() {
                let semantic_tokens = semantic_tokens(session, module_id, tree);
                eprintln!(
                    "[SEND: Response] SemanticTokens ({})",
                    semantic_tokens.len()
                );

                let result = lsp::SemanticTokens {
                    result_id: None,
                    data: semantic_tokens,
                };
                send(conn, lsp_server::Response::new_ok(id, result));
            } else {
                eprintln!(" - syntax tree is None");
            }
        }
    }
}

fn handle_notification(context: &mut ServerContext, not: Notification) {
    match not {
        Notification::SourceFileChanged { path, text } => {
            eprintln!("[HANDLE] Notification::SourceFileChanged: {:?}", &path);
            if let Some(session) = &mut context.session {
                if let Some(module_id) = session.pkg_storage.find_module_by_path(&path) {
                    context.cache.change(path, Some(text.clone())); //@hack
                    let module = session.pkg_storage.module_mut(module_id);
                    module.line_ranges = text::find_line_ranges(&text);
                    module.source = text;
                    eprintln!(" - updated source & line_ranges");
                } else {
                    eprintln!(" - module not found");
                }
            } else {
                eprintln!(" - session is None");
            }
        }
        Notification::SourceFileClosed { path } => {}
    }
}

fn handle_compile_project(conn: &Connection, context: &mut ServerContext) {
    eprintln!("[Handle] CompileProject");

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
    Diagnostic, DiagnosticData, ErrorWarningBuffer, Severity, SourceRange, WarningBuffer,
};
use rock_core::hir_lower;
use rock_core::session::{FileCache, ModuleID, Session};
use rock_core::syntax::ast_build;
use rock_core::text::{self, TextRange};

use lsp::{DiagnosticRelatedInformation, Location, Position, PublishDiagnosticsParams, Range};
use std::path::PathBuf;

fn check_impl(session: &mut Session) -> Result<WarningBuffer, ErrorWarningBuffer> {
    ast_build::parse(session)?;
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

fn run_diagnostics(
    conn: &Connection,
    context: &mut ServerContext,
) -> Vec<PublishDiagnosticsParams> {
    //@not used now, only sending one session error
    let mut messages = Vec::<lsp::Diagnostic>::new();
    let mut diagnostics_map = HashMap::new();

    let mut session = match Session::new(&context.cache, false) {
        Ok(value) => value,
        Err(error) => {
            let params = lsp::ShowMessageParams {
                typ: lsp::MessageType::ERROR,
                message: error.diagnostic().msg().as_str().to_string(),
            };
            let notification = lsp_server::Notification {
                method: notification::ShowMessage::METHOD.into(),
                params: serde_json::to_value(params).unwrap(),
            };
            send(conn, notification);
            return vec![];
        }
    };

    let (errors, warnings) = match check_impl(&mut session) {
        Ok(warnings) => (vec![], warnings.collect()),
        Err(errw) => errw.collect(),
    };

    for module_id in session.pkg_storage.module_ids() {
        let path = session.pkg_storage.module(module_id).path.clone();
        diagnostics_map.insert(path, Vec::new());
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

    context.session = Some(session);

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
}

fn semantic_tokens(
    session: &Session,
    module_id: ModuleID,
    //@should in theory always exist if module exists
    // store even broken SyntaxTree in the module (without creating the Ast)
    tree: &SyntaxTree, //@dont pass if can get from module_id
) -> Vec<lsp::SemanticToken> {
    let module = session.pkg_storage.module(module_id);
    let source = module.source.as_str();
    let line_ranges = module.line_ranges.as_slice();

    let mut builder = SemanticTokenBuilder {
        curr_line: 0,
        prev_range: None,
        source,
        line_ranges,
        //@temp semantic token count estimate
        semantic_tokens: Vec::with_capacity(tree.tokens().token_count() / 2),
    };

    semantic_visit_node(&mut builder, tree, tree.root(), None);
    builder.semantic_tokens
}

fn semantic_visit_node(
    builder: &mut SemanticTokenBuilder,
    tree: &SyntaxTree,
    node: &Node,
    ident_style: Option<SemanticToken>,
) {
    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                let node = tree.node(node_id);

                let ident_style = match node.kind {
                    SyntaxKind::ERROR => None,
                    SyntaxKind::TOMBSTONE => None,
                    SyntaxKind::SOURCE_FILE => None,

                    SyntaxKind::ATTR_LIST => None,
                    SyntaxKind::ATTR => Some(SemanticToken::Variable),
                    SyntaxKind::ATTR_PARAM_LIST => None,
                    SyntaxKind::ATTR_PARAM => Some(SemanticToken::Variable),
                    SyntaxKind::VISIBILITY => None,

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
                    SyntaxKind::IMPORT_SYMBOL => Some(SemanticToken::Property), //depends
                    SyntaxKind::IMPORT_SYMBOL_RENAME => Some(SemanticToken::Property),

                    SyntaxKind::TYPE_BASIC => None,
                    SyntaxKind::TYPE_CUSTOM => Some(SemanticToken::Type),
                    SyntaxKind::TYPE_REFERENCE => None,
                    SyntaxKind::TYPE_PROCEDURE => None,
                    SyntaxKind::PARAM_TYPE_LIST => None,
                    SyntaxKind::TYPE_ARRAY_SLICE => None,
                    SyntaxKind::TYPE_ARRAY_STATIC => None,

                    SyntaxKind::BLOCK => None,
                    SyntaxKind::STMT_BREAK => None,
                    SyntaxKind::STMT_CONTINUE => None,
                    SyntaxKind::STMT_RETURN => None,
                    SyntaxKind::STMT_DEFER => None,
                    SyntaxKind::STMT_LOOP => None,
                    SyntaxKind::LOOP_WHILE_HEADER => None,
                    SyntaxKind::LOOP_CLIKE_HEADER => None,
                    SyntaxKind::STMT_LOCAL => None,
                    SyntaxKind::STMT_ASSIGN => None,
                    SyntaxKind::STMT_EXPR_SEMI => None,
                    SyntaxKind::STMT_EXPR_TAIL => None,

                    SyntaxKind::EXPR_PAREN => None,
                    SyntaxKind::EXPR_IF => None,
                    SyntaxKind::BRANCH_ENTRY => None,
                    SyntaxKind::BRANCH_ELSE_IF => None,
                    SyntaxKind::EXPR_MATCH => None,
                    SyntaxKind::MATCH_ARM_LIST => None,
                    SyntaxKind::MATCH_ARM => None,
                    SyntaxKind::EXPR_FIELD => Some(SemanticToken::Property),
                    SyntaxKind::EXPR_INDEX => None,
                    SyntaxKind::EXPR_CALL => None, //defer to path
                    SyntaxKind::EXPR_CAST => None,
                    SyntaxKind::EXPR_SIZEOF => None,
                    SyntaxKind::EXPR_ITEM => None,
                    SyntaxKind::EXPR_VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::EXPR_STRUCT_INIT => None, //defer to path
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
                    SyntaxKind::PATH => Some(SemanticToken::Property),
                    SyntaxKind::BIND => Some(SemanticToken::Variable),
                    SyntaxKind::BIND_LIST => None,
                    SyntaxKind::ARGS_LIST => None,
                };

                semantic_visit_node(builder, tree, node, ident_style);
            }
            NodeOrToken::Token(token_id) => {
                let (token, range) = tree.tokens().token_and_range(token_id);

                let semantic = match token {
                    Token::Eof => continue,
                    Token::Ident => match ident_style {
                        Some(semantic) => semantic,
                        None => continue,
                    },
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

                semantic_add_token(builder, semantic, range);
            }
            NodeOrToken::Trivia(id) => {
                let (trivia, range) = tree.tokens().trivia_and_range(id);

                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment => {
                        semantic_add_token(builder, SemanticToken::Comment, range)
                    }
                };
            }
        }
    }
}

fn str_char_len_utf16(string: &str) -> u32 {
    string.chars().map(|c| c.len_utf16() as u32).sum()
}

fn semantic_add_token(
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
