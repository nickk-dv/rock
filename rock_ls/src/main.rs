#![forbid(unsafe_code)]
mod message;
mod text_ops;

use lsp_server::{Connection, RequestId};
use lsp_types as lsp;
use message::{Action, Message, MessageBuffer, Notification, Request};
use rock_core::intern::{NameID, StringPool};
use rock_core::syntax::ast_layer::{self as cst, AstNode};
use rock_core::syntax::format::FormatterCache;
use rock_core::syntax::token::{SemanticToken, Token, Trivia};
use rock_core::syntax::tree::{Node, NodeID, NodeOrToken, SyntaxKind, SyntaxTree};
use rustc_hash::FxHashMap;

#[macro_export]
macro_rules! server_error {
    ($($arg:tt)*) => {{
        eprintln!("[server error] {}", format!($($arg)*));
        std::process::exit(1);
    }};
}

#[macro_export]
macro_rules! debug_eprintln {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        eprintln!("{}", format!($($arg)*));
    }};
}

fn main() {
    let (conn, threads) = Connection::stdio();
    initialize_handshake(&conn);
    if let Ok(mut server) = initialize_server(&conn) {
        compile_project(&mut server);
        for module_id in server.session.module.ids() {
            update_module_symbols(&mut server, module_id, SymbolUpdate::Defined);
        }
        for module_id in server.session.module.ids() {
            update_module_symbols(&mut server, module_id, SymbolUpdate::Imports);
        }
        server_loop(&mut server);
    }
    let _ = threads.join();
}

fn initialize_handshake(conn: &Connection) {
    let document_sync = lsp::TextDocumentSyncOptions {
        open_close: Some(true),
        change: Some(lsp::TextDocumentSyncKind::INCREMENTAL),
        will_save: None,
        will_save_wait_until: None,
        save: Some(lsp::TextDocumentSyncSaveOptions::SaveOptions(lsp::SaveOptions {
            include_text: None,
        })),
    };

    let filter = lsp::FileOperationRegistrationOptions {
        filters: vec![
            lsp::FileOperationFilter {
                scheme: Some("file".to_string()),
                pattern: lsp::FileOperationPattern {
                    glob: "**/*.rock".to_string(),
                    matches: Some(lsp::FileOperationPatternKind::File),
                    options: None,
                },
            },
            lsp::FileOperationFilter {
                scheme: Some("file".to_string()),
                pattern: lsp::FileOperationPattern {
                    glob: "**/*".to_string(),
                    matches: Some(lsp::FileOperationPatternKind::Folder),
                    options: None,
                },
            },
        ],
    };
    let file_ops = lsp::WorkspaceFileOperationsServerCapabilities {
        did_create: Some(filter.clone()),
        did_rename: Some(filter.clone()),
        did_delete: Some(filter),
        will_create: None,
        will_rename: None,
        will_delete: None,
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

    let server_caps = lsp::ServerCapabilities {
        text_document_sync: Some(lsp::TextDocumentSyncCapability::Options(document_sync)),
        definition_provider: Some(lsp::OneOf::Left(true)),
        document_formatting_provider: Some(lsp::OneOf::Left(true)),
        workspace: Some(lsp::WorkspaceServerCapabilities {
            workspace_folders: None,
            file_operations: Some(file_ops),
        }),
        semantic_tokens_provider: Some(
            lsp::SemanticTokensServerCapabilities::SemanticTokensOptions(semantic_tokens),
        ),
        inlay_hint_provider: Some(lsp::OneOf::Left(true)),
        ..Default::default()
    };

    let (id, _) = match conn.initialize_start() {
        Ok((id, value)) => (id, from_json::<lsp::InitializeParams>(value)),
        Err(error) => server_error!("conn.initialize_start() failed\n{error}"),
    };

    let init_result = lsp::InitializeResult {
        capabilities: server_caps,
        server_info: Some(lsp::ServerInfo { name: "rock_ls".to_string(), version: None }),
    };
    if let Err(error) = conn.initialize_finish(id, into_json(init_result)) {
        server_error!("conn.initialize_finish() failed\n{error}")
    }
}

fn uri_to_path(uri: &lsp::Url) -> PathBuf {
    match uri.to_file_path() {
        Ok(path) => path,
        Err(()) => server_error!("failed to convert url `{}` to path", uri.to_string()),
    }
}

fn url_from_path(path: &PathBuf) -> lsp::Url {
    match lsp::Url::from_file_path(path) {
        Ok(url) => url,
        Err(()) => server_error!("failed to convert path `{}` to url", path.to_string_lossy()),
    }
}

#[track_caller]
fn into_json<T: serde::Serialize>(value: T) -> serde_json::Value {
    match serde_json::to_value(value) {
        Ok(value) => value,
        Err(error) => {
            let loc = core::panic::Location::caller();
            server_error!("into_json() failed at: {loc}\n{error}");
        }
    }
}

#[track_caller]
fn from_json<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    match serde_json::from_value(value) {
        Ok(value) => value,
        Err(error) => {
            let loc = core::panic::Location::caller();
            server_error!("from_json() failed at: {loc}\n{error}");
        }
    }
}

#[track_caller]
fn send_response(conn: &Connection, id: RequestId, result: impl serde::Serialize) {
    let result = Some(serde_json::to_value(result).unwrap());
    let response = lsp_server::Response { id, result, error: None };
    if let Err(error) = conn.sender.send(response.into()) {
        let loc = core::panic::Location::caller();
        server_error!("send_response() failed at: {loc}\n{error}");
    }
}

#[track_caller]
fn send_notification<N: lsp::notification::Notification>(
    conn: &Connection,
    params: impl serde::Serialize,
) {
    let params = serde_json::to_value(params).unwrap();
    let notification = lsp_server::Notification { method: N::METHOD.to_string(), params };
    if let Err(error) = conn.sender.send(notification.into()) {
        let loc = core::panic::Location::caller();
        server_error!("send_notification() failed at: {loc}\n{error}");
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
    symbols: FxHashMap<NameID, Symbol>,
}

fn initialize_server(conn: &Connection) -> Result<ServerContext, ()> {
    use rock_core::session::config::{Build, Config, TargetTriple};
    let config = Config::new(TargetTriple::host(), Build::Debug);

    let session = match session::create_session(config) {
        Ok(value) => value,
        Err(error) => {
            let message = error.diagnostic().msg.to_string();
            let params = lsp::ShowMessageParams { typ: lsp::MessageType::ERROR, message };
            send_notification::<lsp::notification::ShowMessage>(conn, params);
            return Err(());
        }
    };

    let mut modules = Vec::with_capacity(session.module.count());
    for _ in session.module.ids() {
        let data = ModuleData { symbols_version: 0, symbols: FxHashMap::default() };
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
    debug_eprintln!("");
    for message in messages {
        match message {
            Message::Request(id, req) => match req {
                Request::Format(path) => handle_format(server, id, path),
                Request::SemanticTokens(path) => handle_semantic_tokens(server, id, path),
                Request::InlayHints(path, range) => handle_inlay_hints(server, id, path, range),
                Request::GotoDefinition(path, pos) => handle_goto_definition(server, id, path, pos),
                Request::ShowSyntaxTree(path) => handle_show_syntax_tree(server, id, path),
            },
            Message::Notification(not) => match not {
                Notification::FileSaved => compile_project(server),
                Notification::FileCreate(p) => handle_file_create(server, p),
                Notification::FileDelete(p) => handle_file_delete(server, p),
                Notification::FileRename(p) => handle_file_rename(server, p),
                Notification::FileChanged(p) => handle_file_changed(server, p),
            },
        }
    }
}

fn update_syntax_tree(session: &mut Session, module_id: ModuleID) {
    let module = session.module.get_mut(module_id);
    if module.tree_version == module.file_version {
        return;
    }
    let (tree, errors) =
        syntax::parse_tree(&module.file.source, module_id, &mut session.intern_lit);
    module.tree = Some(tree);
    module.parse_errors = errors;
    module.tree_version = module.file_version;
}

#[derive(PartialEq)]
enum SymbolUpdate {
    All,
    Defined,
    Imports,
}

fn update_module_symbols(server: &mut ServerContext, module_id: ModuleID, update: SymbolUpdate) {
    let module = server.session.module.get(module_id);
    let data = &mut server.modules[module_id.index()];
    if update == SymbolUpdate::All && data.symbols_version == module.tree_version {
        return;
    }

    let tree = module.tree.as_ref().unwrap();
    let file = &module.file;

    if update == SymbolUpdate::All {
        data.symbols.clear();
        data.symbols.reserve(tree.root().content_len as usize); //content_len includes non-nodes aswell
        data.symbols_version = module.tree_version;
    }

    let root = cst::SourceFile::cast(tree.root()).unwrap();
    for item in root.items(tree) {
        match item {
            cst::Item::Proc(item) if update != SymbolUpdate::Imports => {
                if let Some(name) = item.name(tree) {
                    let id = name_id(name, tree, file, &mut server.session.intern_name);
                    server.modules[module_id.index()]
                        .symbols
                        .insert(id, Symbol::Defined(SymbolKind::Proc));
                }
            }
            cst::Item::Enum(item) if update != SymbolUpdate::Imports => {
                if let Some(name) = item.name(tree) {
                    let id = name_id(name, tree, file, &mut server.session.intern_name);
                    server.modules[module_id.index()]
                        .symbols
                        .insert(id, Symbol::Defined(SymbolKind::Enum));
                }
            }
            cst::Item::Struct(item) if update != SymbolUpdate::Imports => {
                if let Some(name) = item.name(tree) {
                    let id = name_id(name, tree, file, &mut server.session.intern_name);
                    server.modules[module_id.index()]
                        .symbols
                        .insert(id, Symbol::Defined(SymbolKind::Struct));
                }
            }
            cst::Item::Const(item) if update != SymbolUpdate::Imports => {
                if let Some(name) = item.name(tree) {
                    let id = name_id(name, tree, file, &mut server.session.intern_name);
                    server.modules[module_id.index()]
                        .symbols
                        .insert(id, Symbol::Defined(SymbolKind::Const));
                }
            }
            cst::Item::Global(item) if update != SymbolUpdate::Imports => {
                if let Some(name) = item.name(tree) {
                    let id = name_id(name, tree, file, &mut server.session.intern_name);
                    server.modules[module_id.index()]
                        .symbols
                        .insert(id, Symbol::Defined(SymbolKind::Global));
                }
            }
            cst::Item::Import(item) if update != SymbolUpdate::Defined => {
                let module_name = if let Some(rename) = item.rename(tree) {
                    rename
                        .alias(tree)
                        .map(|n| name_id(n, tree, file, &mut server.session.intern_name))
                } else if let Some(path) = item.import_path(tree) {
                    path.names(tree)
                        .last()
                        .map(|n| name_id(n, tree, file, &mut server.session.intern_name))
                } else {
                    None
                };

                let Some(source_id) = resolve_import_module(&server.session, module_id, item)
                else {
                    continue;
                };
                if let Some(module_name) = module_name {
                    server.modules[module_id.index()]
                        .symbols
                        .insert(module_name, Symbol::Module(source_id));
                }

                if let Some(symbol_list) = item.import_symbol_list(tree) {
                    for symbol in symbol_list.import_symbols(tree) {
                        let mut import_name;

                        let import_symbol = if let Some(name) = symbol.name(tree) {
                            import_name = Some(name);
                            let id = name_id(name, tree, file, &mut server.session.intern_name);
                            server.modules[source_id.index()].symbols.get(&id).copied()
                        } else {
                            continue;
                        };
                        if let Some(rename) = symbol.rename(tree) {
                            if rename.t_discard(tree).is_some() {
                                continue;
                            }
                            if let Some(name) = rename.alias(tree) {
                                import_name = Some(name);
                            }
                        }
                        if let Some(symbol) = import_symbol {
                            if let Some(name) = import_name {
                                let id = name_id(name, tree, file, &mut server.session.intern_name);
                                if let Symbol::Defined(kind) = symbol {
                                    server.modules[module_id.index()]
                                        .symbols
                                        .insert(id, Symbol::Imported(source_id, kind));
                                }
                            }
                        }
                    }
                }
            }
            cst::Item::Directive(_) => {}
            _ => {}
        }
    }
}

fn resolve_import_module(
    session: &Session,
    origin_id: ModuleID,
    import: cst::ImportItem,
) -> Option<ModuleID> {
    let module = session.module.get(origin_id);
    let tree = module.tree.as_ref().unwrap();
    let file = &module.file;
    let path = import.import_path(tree)?;

    let mut package_id = module.origin;
    if let Some(name) = import.package(tree) {
        let id = name_id_opt(name, tree, file, &session.intern_name)?;
        package_id = session.graph.find_package_dep(module.origin, id)?;
    }

    let mut module_name = None;
    let mut target_dir = &session.graph.package(package_id).src;
    let mut path_names = path.names(tree).peekable();

    while let Some(name) = path_names.next() {
        if path_names.peek().is_none() {
            module_name = Some(name);
            break;
        }
        let id = name_id_opt(name, tree, file, &session.intern_name)?;
        target_dir = match target_dir.find(session, id) {
            session::ModuleOrDirectory::Directory(dir) => dir,
            _ => return None,
        };
    }

    let module_name = module_name?;
    let id = name_id_opt(module_name, tree, file, &session.intern_name)?;
    match target_dir.find(session, id) {
        session::ModuleOrDirectory::Module(module_id) => Some(module_id),
        _ => None,
    }
}

fn name_id(
    name: cst::Name,
    tree: &SyntaxTree,
    file: &session::FileData,
    intern: &mut StringPool<NameID>,
) -> NameID {
    let text = &file.source[name.ident(tree).as_usize()];
    intern.intern(text)
}

fn name_id_opt(
    name: cst::Name,
    tree: &SyntaxTree,
    file: &session::FileData,
    intern: &StringPool<NameID>,
) -> Option<NameID> {
    let text = &file.source[name.ident(tree).as_usize()];
    intern.get_id(text)
}

fn handle_format(server: &mut ServerContext, id: RequestId, path: PathBuf) {
    debug_eprintln!("[format] path: {}", path.to_string_lossy());
    let session = &mut server.session;
    let module_id = match module_id_from_path(session, &path) {
        Some(module_id) => module_id,
        None => return send_response(server.conn, id, serde_json::Value::Null),
    };

    update_syntax_tree(session, module_id);

    let module = session.module.get(module_id);
    let tree = module.tree.as_ref().unwrap();
    if !tree.complete {
        return send_response(server.conn, id, serde_json::Value::Null);
    }

    let file = &module.file;
    let formatted = rock_core::syntax::format::format(
        tree,
        &file.source,
        &file.line_ranges,
        &mut server.fmt_cache,
    );

    let start = lsp::Position::new(0, 0);
    let end = lsp::Position::new(file.line_ranges.len() as u32 + 1, 0);
    let text_edit = lsp::TextEdit::new(lsp::Range::new(start, end), formatted);
    send_response(server.conn, id, vec![text_edit]);
}

fn handle_semantic_tokens(server: &mut ServerContext, id: RequestId, path: PathBuf) {
    debug_eprintln!("[semantic_tokens] path: {}", path.to_string_lossy());
    let session = &mut server.session;
    let module_id = match module_id_from_path(session, &path) {
        Some(module_id) => module_id,
        None => return send_response(server.conn, id, serde_json::Value::Null),
    };

    update_syntax_tree(session, module_id);
    update_module_symbols(server, module_id, SymbolUpdate::All);

    let data = semantic_tokens(server, module_id);
    send_response(server.conn, id, lsp::SemanticTokens { result_id: None, data });
}

fn handle_inlay_hints(server: &mut ServerContext, id: RequestId, path: PathBuf, range: lsp::Range) {
    debug_eprintln!("[inlay_hints] path: {}, {:?}", path.to_string_lossy(), range);
    let session = &mut server.session;
    let module_id = match module_id_from_path(session, &path) {
        Some(module_id) => module_id,
        None => return send_response(server.conn, id, serde_json::Value::Null),
    };

    update_syntax_tree(session, module_id);

    //@rework search & scoping, use previous Hir type info.
    /*
    let mut hints = Vec::with_capacity(64);
    let module = session.module.get(module_id);
    let tree = module.tree.as_ref().unwrap();

    for node in tree.nodes() {
        let Some(local) = cst::StmtLocal::cast(node) else {
            continue;
        };
        if local.ty(tree).is_some() {
            continue;
        }
        let Some(bind) = local.bind(tree) else {
            continue;
        };

        hints.push(lsp::InlayHint {
            position: text_ops::offset_utf8_to_position_utf16(&module.file, bind.0.range.end()),
            label: lsp::InlayHintLabel::String(": <unknown>".to_string()),
            kind: Some(lsp::InlayHintKind::TYPE),
            text_edits: None,
            tooltip: None,
            padding_left: None,
            padding_right: None,
            data: None,
        });
    }
    send_response(server.conn, id, hints)
    */

    send_response(server.conn, id, serde_json::Value::Null)
}

fn chain_node_nth<'syn, T: AstNode<'syn>>(
    tree: &'syn SyntaxTree,
    chain: &[NodeID],
    idx: usize,
) -> Option<T> {
    T::cast(tree.node(chain.iter().copied().nth(idx)?))
}

fn chain_node_nth_back<'syn, T: AstNode<'syn>>(
    tree: &'syn SyntaxTree,
    chain: &[NodeID],
    idx: usize,
) -> Option<T> {
    T::cast(tree.node(chain.iter().copied().nth_back(idx)?))
}

fn handle_goto_definition(
    server: &mut ServerContext,
    id: RequestId,
    path: PathBuf,
    pos: lsp::Position,
) {
    debug_eprintln!("[goto_definition] path: {}, {:?}", path.to_string_lossy(), pos);
    let module_id = match module_id_from_path(&server.session, &path) {
        Some(module_id) => module_id,
        None => return send_response(server.conn, id, serde_json::Value::Null),
    };

    update_syntax_tree(&mut server.session, module_id);
    update_module_symbols(server, module_id, SymbolUpdate::All);

    match goto_definition_location(server, module_id, pos) {
        Some(location) => send_response(server.conn, id, location),
        None => send_response(server.conn, id, serde_json::Value::Null),
    }
}

fn goto_definition_location(
    server: &mut ServerContext,
    module_id: ModuleID,
    pos: lsp::Position,
) -> Option<lsp::Location> {
    let module = server.session.module.get(module_id);
    let tree = module.tree.as_ref().unwrap();
    let goto_offset = text_ops::position_utf16_to_offset_utf8(&module.file, pos);
    let chain = node_chain(tree, goto_offset);

    let path = chain_node_nth_back::<cst::Path>(tree, &chain, 2)?;
    let mut segments = path.segments(tree);
    let mut name = segments.next()?.name(tree)?;
    let mut origin_id = module_id;

    if name.0.range.contains_inclusive(goto_offset) {
        if let Some(Some(param_list)) =
            chain_node_nth::<cst::ProcItem>(tree, &chain, 0).map(|p| p.param_list(tree))
        {
            for param in param_list.params(tree) {
                let Some(param_name) = param.name(tree) else {
                    continue;
                };
                let name_str = &module.file.source[name.ident(tree).as_usize()];
                let param_name_str = &module.file.source[param_name.ident(tree).as_usize()];
                if name_str == param_name_str {
                    let param_pos = text_ops::offset_utf8_to_position_utf16(
                        &module.file,
                        param_name.0.range.start(),
                    );
                    return Some(lsp::Location::new(
                        lsp::Url::from_file_path(&module.file.path).unwrap(),
                        lsp::Range::new(param_pos, param_pos),
                    ));
                }
            }
        }
    }

    let mut symbol = scope_symbol(server, module_id, module_id, name)?;

    if let Symbol::Module(target_id) = symbol {
        if name.0.range.contains_inclusive(goto_offset) {
            let target = server.session.module.get(target_id);
            return Some(lsp::Location::new(
                lsp::Url::from_file_path(&target.file.path).unwrap(),
                lsp::Range::default(),
            ));
        }
        name = segments.next()?.name(tree)?;
        symbol = scope_symbol(server, target_id, module_id, name)?;
        origin_id = target_id;
    }

    match symbol {
        Symbol::Module(_) => return None,
        Symbol::Defined(_) => {}
        Symbol::Imported(import_id, _) => {
            if origin_id != module_id {
                return None; //imported from another module
            }
            origin_id = import_id;
        }
    };

    let (offset, item) = find_item_name_offset(server, origin_id, module_id, name)?;
    if name.0.range.contains_inclusive(goto_offset) {
        let target = server.session.module.get(origin_id);
        let item_pos = text_ops::offset_utf8_to_position_utf16(&target.file, offset);
        return Some(lsp::Location::new(
            lsp::Url::from_file_path(&target.file.path).unwrap(),
            lsp::Range::new(item_pos, item_pos),
        ));
    }

    let cst::Item::Enum(item) = item else {
        return None;
    };
    name = segments.next()?.name(tree)?;
    if !name.0.range.contains_inclusive(goto_offset) {
        return None;
    }
    let variant_name = &module.file.source[name.0.range.as_usize()];
    let target = server.session.module.get(origin_id);
    let tree = target.tree.as_ref().unwrap();

    for variant in item.variant_list(tree)?.variants(tree) {
        if let Some(vname) = variant.name(tree) {
            let vname_name = &target.file.source[vname.0.range.as_usize()];
            if variant_name == vname_name {
                let item_pos =
                    text_ops::offset_utf8_to_position_utf16(&target.file, vname.0.range.start());
                return Some(lsp::Location::new(
                    lsp::Url::from_file_path(&target.file.path).unwrap(),
                    lsp::Range::new(item_pos, item_pos),
                ));
            }
        }
    }
    None
}

fn scope_symbol(
    server: &ServerContext,
    target_id: ModuleID,
    module_id: ModuleID,
    name: cst::Name,
) -> Option<Symbol> {
    let module = server.session.module.get(module_id);
    let tree = module.tree.as_ref().unwrap();
    let name_str = &module.file.source[name.ident(tree).as_usize()];
    let name_id = server.session.intern_name.get_id(name_str)?; //@intern is better, borrow check blocked
    server.modules[target_id.index()].symbols.get(&name_id).copied()
}

fn find_item_name_offset<'s>(
    server: &'s ServerContext,
    target_id: ModuleID,
    module_id: ModuleID,
    name: cst::Name,
) -> Option<(TextOffset, cst::Item<'s>)> {
    let module = server.session.module.get(module_id);
    let tree = module.tree.as_ref().unwrap();
    let name_str = &module.file.source[name.ident(tree).as_usize()];
    let name_id = server.session.intern_name.get_id(name_str)?; //@intern is better, borrow check blocked

    let module = server.session.module.get(target_id);
    let tree = module.tree.as_ref().unwrap();
    let root = cst::SourceFile(tree.root());

    for item in root.items(tree) {
        let name_opt = match item {
            cst::Item::Proc(item) => item.name(tree),
            cst::Item::Enum(item) => item.name(tree),
            cst::Item::Struct(item) => item.name(tree),
            cst::Item::Const(item) => item.name(tree),
            cst::Item::Global(item) => item.name(tree),
            cst::Item::Import(_) | cst::Item::Directive(_) => continue,
        };
        let Some(name) = name_opt else {
            continue;
        };
        let name_str = &module.file.source[name.0.range.as_usize()];
        //@intern is better, borrow check blocked
        if server.session.intern_name.get_id(name_str) == Some(name_id) {
            return Some((name.range().start(), item));
        }
    }
    None
}

fn node_chain(tree: &SyntaxTree, offset: TextOffset) -> Vec<NodeID> {
    let mut chain = Vec::with_capacity(16);
    let mut target_id = Some(NodeID::new(0));

    while let Some(node_id) = target_id {
        let target = tree.node(node_id);
        target_id = None;

        for not in tree.content(target) {
            if let NodeOrToken::Node(node_id) = not {
                if tree.node(node_id).range.contains_inclusive(offset) {
                    chain.push(node_id);
                    target_id = Some(node_id);
                    break;
                }
            }
        }
    }
    chain
}

fn handle_show_syntax_tree(server: &mut ServerContext, id: RequestId, path: PathBuf) {
    debug_eprintln!("[show_syntax_tree] path: {}", path.to_string_lossy());
    let session = &mut server.session;
    let module_id = match module_id_from_path(session, &path) {
        Some(module_id) => module_id,
        None => return send_response(server.conn, id, "error: unknown file".to_string()),
    };

    update_syntax_tree(session, module_id);

    let module = session.module.get(module_id);
    let tree = module.tree.as_ref().unwrap();
    let tree_display = syntax::tree::tree_display(tree, &module.file.source);
    send_response(server.conn, id, tree_display);
}

fn compile_project(server: &mut ServerContext) {
    use std::time::Instant;

    let session = &mut server.session;
    session.errors.clear();
    for module_id in session.module.ids() {
        session.module.get_mut(module_id).check_errors.clear();
    }

    let mut timer = Instant::now();
    fn check_impl(session: &mut Session) -> Result<(), ()> {
        syntax::parse_all_lsp(session)?;
        hir_lower::check(session)?;
        Ok(())
    }
    let _ = check_impl(session);
    let check_ms = timer.elapsed().as_secs_f64() * 1000.0;
    timer = Instant::now();

    let mut publish_diagnostics = Vec::with_capacity(session.module.count());

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = &module.file;

        let capacity = module.parse_errors.errors.len()
            + module.check_errors.errors.len()
            + module.check_errors.warnings.len();
        let mut diagnostics = Vec::with_capacity(capacity);

        for error in &module.parse_errors.errors {
            if let Some(d) = create_diagnostic(session, error.diagnostic(), Severity::Error) {
                diagnostics.push(d);
            }
        }
        for error in &module.check_errors.errors {
            if let Some(d) = create_diagnostic(session, error.diagnostic(), Severity::Error) {
                diagnostics.push(d);
            }
        }
        for warning in &module.check_errors.warnings {
            if let Some(d) = create_diagnostic(session, warning.diagnostic(), Severity::Warning) {
                diagnostics.push(d);
            }
        }

        let publish =
            lsp::PublishDiagnosticsParams::new(url_from_path(&file.path), diagnostics, None);
        publish_diagnostics.push(publish);
    }

    let di_ms = timer.elapsed().as_secs_f64() * 1000.0;
    debug_eprintln!("[compile project] check impl: {check_ms} ms, make diagnostics: {di_ms} ms");

    for publish in publish_diagnostics.iter() {
        send_notification::<lsp::notification::PublishDiagnostics>(server.conn, publish);
    }
    for error in &server.session.errors.errors {
        let message = error.diagnostic().msg.to_string();
        let params = lsp::ShowMessageParams { typ: lsp::MessageType::ERROR, message };
        send_notification::<lsp::notification::ShowMessage>(server.conn, params);
    }
}

fn handle_file_create(_: &mut ServerContext, p: lsp::CreateFilesParams) {
    debug_eprintln!("[file create] {:?}", p);
}

fn handle_file_delete(_: &mut ServerContext, p: lsp::DeleteFilesParams) {
    debug_eprintln!("[file delete] {:?}", p);
}

fn handle_file_rename(_: &mut ServerContext, p: lsp::RenameFilesParams) {
    debug_eprintln!("[file rename] {:?}", p);
}

fn handle_file_changed(server: &mut ServerContext, p: lsp::DidChangeTextDocumentParams) {
    let path = uri_to_path(&p.text_document.uri);
    debug_eprintln!(
        "[file changed] path: {}, changes: {}",
        path.to_string_lossy(),
        p.content_changes.len()
    );
    let session = &mut server.session;
    let module = match module_id_from_path(session, &path) {
        Some(module_id) => session.module.get_mut(module_id),
        None => return,
    };
    let file = &mut module.file;
    module.file_version += 1;

    for change in p.content_changes {
        if let Some(range) = change.range {
            let range = text_ops::range_utf16_to_range_utf8(file, range);
            file.source.replace_range(range.as_usize(), &change.text);
            text::find_line_ranges(&mut file.line_ranges, &file.source);
        } else {
            file.source = change.text;
            text::find_line_ranges(&mut file.line_ranges, &file.source);
        }
    }
}

use rock_core::error::{Diagnostic, DiagnosticData, Severity, SourceRange};
use rock_core::hir_lower;
use rock_core::session::{self, ModuleID, Session};
use rock_core::syntax;
use rock_core::text::{self, TextOffset, TextRange};
use std::path::{Path, PathBuf};

fn module_id_from_path(session: &Session, path: &Path) -> Option<ModuleID> {
    session.module.path_to_id(path)
}

fn create_diagnostic(
    session: &Session,
    diagnostic: &Diagnostic,
    severity: Severity,
) -> Option<lsp::Diagnostic> {
    let (main, related_info) = match &diagnostic.data {
        DiagnosticData::Message => return None,
        DiagnosticData::Context { main, info } => {
            if let Some(info) = info {
                let info_range = source_to_range(session, info.context().src);
                let info_path = source_to_path(session, info.context().src);
                let related_info = lsp::DiagnosticRelatedInformation {
                    location: lsp::Location::new(url_from_path(info_path), info_range),
                    message: info.context().msg.to_string(),
                };
                (main, Some(vec![related_info]))
            } else {
                (main, None)
            }
        }
        DiagnosticData::ContextVec { main, info_vec } => {
            let mut related_infos = Vec::with_capacity(info_vec.len());
            for info in info_vec {
                let info_range = source_to_range(session, info.context().src);
                let info_path = source_to_path(session, info.context().src);
                let related_info = lsp::DiagnosticRelatedInformation {
                    location: lsp::Location::new(url_from_path(info_path), info_range),
                    message: info.context().msg.to_string(),
                };
                related_infos.push(related_info);
            }
            (main, Some(related_infos))
        }
    };

    let mut message = diagnostic.msg.to_string();
    if !main.msg.is_empty() {
        message.push('\n');
        message += &main.msg;
    }

    let diagnostic = lsp::Diagnostic::new(
        source_to_range(session, main.src),
        severity_convert(severity),
        None,
        None,
        message,
        related_info,
        None,
    );

    Some(diagnostic)
}

fn source_to_path<'s, 'sref: 's>(session: &'sref Session<'s>, source: SourceRange) -> &'s PathBuf {
    let module = session.module.get(source.module_id);
    &module.file.path
}

fn source_to_range(session: &Session, source: SourceRange) -> lsp::Range {
    let module = session.module.get(source.module_id);
    text_ops::range_utf8_to_range_utf16(&module.file, source.range)
}

fn severity_convert(severity: Severity) -> Option<lsp::DiagnosticSeverity> {
    match severity {
        Severity::Info => Some(lsp::DiagnosticSeverity::HINT),
        Severity::Error => Some(lsp::DiagnosticSeverity::ERROR),
        Severity::Warning => Some(lsp::DiagnosticSeverity::WARNING),
    }
}

struct SemanticTokenBuilder<'s_ref, 's> {
    module_id: ModuleID,
    curr_line: u32,
    prev_range: Option<TextRange>,
    params_in_scope: Vec<NameID>,
    semantic_tokens: Vec<lsp::SemanticToken>,
    tree: &'s_ref SyntaxTree,
    file: &'s_ref session::FileData,
    modules: &'s_ref [ModuleData],
    intern_name: &'s_ref mut StringPool<'s, NameID>,
}

impl SemanticTokenBuilder<'_, '_> {
    fn name_id(&mut self, name: cst::Name) -> NameID {
        let text = &self.file.source[name.ident(self.tree).as_usize()];
        self.intern_name.intern(text)
    }
}

#[derive(Copy, Clone)]
enum Symbol {
    Module(ModuleID),
    Defined(SymbolKind),
    Imported(ModuleID, SymbolKind),
}

#[derive(Copy, Clone)]
enum SymbolKind {
    Proc,
    Enum,
    Struct,
    Const,
    Global,
}

fn semantic_tokens(server: &mut ServerContext, module_id: ModuleID) -> Vec<lsp::SemanticToken> {
    let module = server.session.module.get(module_id);
    let tree = module.tree.as_ref().unwrap();
    let modules = server.modules.as_slice();
    let intern_name = &mut server.session.intern_name;

    let mut b = SemanticTokenBuilder {
        module_id,
        curr_line: 0,
        prev_range: None,
        params_in_scope: Vec::with_capacity(16),
        semantic_tokens: Vec::with_capacity(tree.tokens.token_count().next_power_of_two() / 2),
        tree,
        file: &module.file,
        modules,
        intern_name,
    };

    semantic_visit_node(&mut b, tree.root(), None);
    b.semantic_tokens
}

fn semantic_visit_node(
    b: &mut SemanticTokenBuilder,
    node: &Node,
    ident_style: Option<SemanticToken>,
) {
    let parent = node.kind;

    if cst::Item::cast(node).is_some() {
        b.params_in_scope.clear();
    }
    if let Some(params) = cst::ParamList::cast(node) {
        for param in params.params(b.tree) {
            if let Some(name) = param.name(b.tree) {
                let id = b.name_id(name);
                b.params_in_scope.push(id);
            }
        }
    }

    for not in b.tree.content(node) {
        match not {
            NodeOrToken::Node(node_id) => {
                let node = b.tree.node(node_id);

                if let Some(path) = cst::Path::cast(node) {
                    semantic_visit_path(b, path, parent);
                    continue;
                } else if let Some(symbols) = cst::ImportSymbolList::cast(node) {
                    semantic_visit_import_symbols(b, symbols);
                    continue;
                }

                let ident_style = match node.kind {
                    SyntaxKind::PROC_ITEM => Some(SemanticToken::Function),
                    SyntaxKind::PARAM => Some(SemanticToken::Parameter),
                    SyntaxKind::ENUM_ITEM => Some(SemanticToken::Type),
                    SyntaxKind::VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::VARIANT_FIELD => Some(SemanticToken::Property),
                    SyntaxKind::STRUCT_ITEM => Some(SemanticToken::Type),
                    SyntaxKind::FIELD => Some(SemanticToken::Property),
                    SyntaxKind::CONST_ITEM => Some(SemanticToken::Variable),
                    SyntaxKind::GLOBAL_ITEM => Some(SemanticToken::Variable),
                    SyntaxKind::IMPORT_ITEM => Some(SemanticToken::Namespace),
                    SyntaxKind::IMPORT_PATH => Some(SemanticToken::Namespace),
                    SyntaxKind::IMPORT_SYMBOL => ident_style,
                    SyntaxKind::IMPORT_SYMBOL_RENAME => ident_style,

                    SyntaxKind::DIRECTIVE_PARAM => Some(SemanticToken::Variable),

                    SyntaxKind::TYPE_CUSTOM => Some(SemanticToken::Type),
                    SyntaxKind::PROC_TYPE_PARAM => Some(SemanticToken::Parameter),

                    SyntaxKind::FOR_BIND => Some(SemanticToken::Variable),
                    SyntaxKind::STMT_LOCAL => Some(SemanticToken::Variable),

                    SyntaxKind::EXPR_FIELD => Some(SemanticToken::Property),
                    SyntaxKind::EXPR_VARIANT => Some(SemanticToken::EnumMember),
                    SyntaxKind::FIELD_INIT => Some(SemanticToken::Property),

                    SyntaxKind::PAT_VARIANT => Some(SemanticToken::EnumMember),

                    SyntaxKind::NAME => ident_style,
                    SyntaxKind::BIND => Some(SemanticToken::Variable),
                    SyntaxKind::POLYMORPH_PARAMS => Some(SemanticToken::Type),
                    _ => None,
                };

                semantic_visit_node(b, node, ident_style);
            }
            NodeOrToken::Token(token_id) => {
                let (token, range) = b.tree.tokens.token_and_range(token_id);
                if let Some(semantic) = semantic_token_style(token, node.kind, ident_style) {
                    semantic_token_add(b, semantic, range);
                }
            }
            NodeOrToken::Trivia(id) => {
                let (trivia, range) = b.tree.tokens.trivia_and_range(id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment | Trivia::DocComment | Trivia::ModComment => {
                        semantic_token_add(b, SemanticToken::Comment, range)
                    }
                };
            }
        }
    }
}

fn semantic_visit_path(b: &mut SemanticTokenBuilder, path: cst::Path, parent: SyntaxKind) {
    let mut origin_id = b.module_id;
    let mut segments = path.segments(b.tree).peekable();

    if let Some(first) = segments.peek() {
        if let Some(name) = first.name(b.tree) {
            let name_id = b.name_id(name);
            let data = &b.modules[origin_id.index()];

            if let Some(Symbol::Module(module_id)) = data.symbols.get(&name_id).copied() {
                semantic_visit_node(b, first.0, Some(SemanticToken::Namespace));
                segments.next();
                origin_id = module_id;
            }
        }
    }

    match parent {
        SyntaxKind::TYPE_CUSTOM | SyntaxKind::EXPR_STRUCT_INIT => {
            for segment in segments.by_ref() {
                semantic_visit_node(b, segment.0, Some(SemanticToken::Type));
            }
        }
        SyntaxKind::EXPR_ITEM | SyntaxKind::PAT_ITEM => {
            let mut is_enum = false;

            if let Some(segment) = segments.next() {
                if let Some(name) = segment.name(b.tree) {
                    let name_id = b.name_id(name);
                    let data = &b.modules[origin_id.index()];
                    let same_origin = origin_id == b.module_id;

                    let style = if let Some(symbol) = data.symbols.get(&name_id).copied() {
                        if same_origin {
                            let kind = match symbol {
                                Symbol::Module(_) => SymbolKind::Struct, //default to type
                                Symbol::Defined(kind) => kind,
                                Symbol::Imported(_, kind) => kind,
                            };
                            is_enum = matches!(kind, SymbolKind::Enum);
                            Some(match kind {
                                SymbolKind::Proc => SemanticToken::Function,
                                SymbolKind::Enum | SymbolKind::Struct => SemanticToken::Type,
                                SymbolKind::Const | SymbolKind::Global => SemanticToken::Variable,
                            })
                        } else {
                            match symbol {
                                Symbol::Module(_) => None,
                                Symbol::Defined(kind) => {
                                    is_enum = matches!(kind, SymbolKind::Enum);
                                    Some(match kind {
                                        SymbolKind::Proc => SemanticToken::Function,
                                        SymbolKind::Enum | SymbolKind::Struct => {
                                            SemanticToken::Type
                                        }
                                        SymbolKind::Const | SymbolKind::Global => {
                                            SemanticToken::Variable
                                        }
                                    })
                                }
                                Symbol::Imported(_, _) => None,
                            }
                        }
                    } else if same_origin && b.params_in_scope.iter().any(|&n| n == name_id) {
                        Some(SemanticToken::Parameter)
                    } else {
                        None
                    };
                    semantic_visit_node(b, segment.0, style);
                }
            }

            if is_enum {
                if let Some(segment) = segments.next() {
                    semantic_visit_node(b, segment.0, Some(SemanticToken::EnumMember));
                }
            }
            for segment in segments.by_ref() {
                semantic_visit_node(b, segment.0, Some(SemanticToken::Property));
            }
        }
        _ => semantic_visit_node(b, path.0, None),
    }
}

fn semantic_visit_import_symbols(b: &mut SemanticTokenBuilder, symbols: cst::ImportSymbolList) {
    let data = &b.modules[b.module_id.index()];

    for symbol in symbols.import_symbols(b.tree) {
        let mut name = match symbol.name(b.tree) {
            Some(name) => name,
            None => {
                semantic_visit_node(b, symbol.0, None);
                continue;
            }
        };
        if let Some(rename) = symbol.rename(b.tree) {
            if let Some(rename) = rename.alias(b.tree) {
                name = rename;
            }
        }

        let id = b.name_id(name);
        let style = if let Some(symbol) = data.symbols.get(&id).copied() {
            let kind = match symbol {
                Symbol::Module(_) => SymbolKind::Struct, //default to type
                Symbol::Defined(kind) => kind,
                Symbol::Imported(_, kind) => kind,
            };
            Some(match kind {
                SymbolKind::Proc => SemanticToken::Function,
                SymbolKind::Enum | SymbolKind::Struct => SemanticToken::Type,
                SymbolKind::Const | SymbolKind::Global => SemanticToken::Variable,
            })
        } else {
            None
        };
        semantic_visit_node(b, symbol.0, style);
    }
}

fn semantic_token_add(b: &mut SemanticTokenBuilder, semantic: SemanticToken, range: TextRange) {
    let source = &b.file.source;
    let line_ranges = b.file.line_ranges.as_slice();

    let mut delta_line: u32 = 0;
    while range.start() >= line_ranges[b.curr_line as usize].end() {
        b.curr_line += 1;
        delta_line += 1;
    }

    let (start, offset) = if delta_line == 0 {
        if let Some(prev_range) = b.prev_range {
            let start = prev_range.start();
            (start, range.start() - start)
        } else {
            let start = line_ranges[b.curr_line as usize].start();
            (start, range.start() - start)
        }
    } else {
        let start = line_ranges[b.curr_line as usize].start();
        (start, range.start() - start)
    };

    let mut delta_range = TextRange::empty_at(start);
    delta_range.extend_by(offset);
    let token_str = &source[delta_range.as_usize()];
    let delta_start = text_ops::string_len_utf16(token_str);

    let token_str = &source[range.as_usize()];
    let length = text_ops::string_len_utf16(token_str);

    b.prev_range = Some(range);
    b.semantic_tokens.push(lsp::SemanticToken {
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

        T![s8] | T![s16] | T![s32] | T![s64] |
        T![u8] | T![u16] | T![u32] | T![u64] |
        T![f32] | T![f64] | T![bool] | T![bool16] | T![bool32] | T![bool64] |
        T![string] | T![cstring] | T![char] | T![never] | T![rawptr] => SemanticToken::Type,
        T![void] if parent == SyntaxKind::TYPE_BASIC => SemanticToken::Type,
        T![void] => SemanticToken::Number,

        T![.] | T![,] | T![:] | T![;] | T![#] => return None,
        T!['('] | T![')'] | T!['['] | T![']'] | T!['{'] | T!['}'] => return None,

        T![?] | T![..] | T![->] | T!["..<"] | T!["..="] | T![~] | T![!] |
        T![+] | T![-] | T![*] | T![/] | T![%] | T![&] | T![|] | T![^] | T![<<] | T![>>] |
        T![==] | T![!=] | T![<] | T![<=] | T![>] | T![>=] | T![&&] | T![||] |
        T![=] | T![+=] | T![-=] | T![*=] | T![/=] | T![%=] | 
        T![&=] | T![|=] | T![^=] | T![<<=] | T![>>=] => SemanticToken::Operator,
    };
    Some(semantic)
}
