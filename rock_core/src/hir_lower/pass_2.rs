use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::error::{ErrorComp, SourceRange, WarningComp};
use crate::session::{ModuleID, ModuleOrDirectory, Session};

pub fn resolve_imports<'hir: 'ast, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'ast>,
    session: &Session,
) {
    for import_id in hir.registry().import_ids() {
        let origin_id = hir.registry().import_data(import_id).origin_id;
        let import = hir.registry().import_item(import_id);
        resolve_import(hir, emit, session, origin_id, import);
    }
}

fn resolve_import<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    import: &ast::ImportItem<'ast>,
) {
    let mut source_package = session.package(session.module(origin_id).package_id);

    if let Some(package_name) = import.package {
        if let Some(dependency_id) = source_package.dependency(package_name.id) {
            source_package = session.package(dependency_id);
        } else {
            emit.error(ErrorComp::new(
                format!(
                    "package `{}` is not found in dependencies of `{}`",
                    hir.name_str(package_name.id),
                    hir.name_str(source_package.name_id),
                ),
                SourceRange::new(origin_id, package_name.range),
                None,
            ));
            return;
        }
    }

    assert!(!import.import_path.is_empty());
    let directory_count = import.import_path.len() - 1;
    let directory_names = &import.import_path[0..directory_count];
    let module_name = *import.import_path.last().unwrap();
    let mut target_dir = &source_package.src;

    for name in directory_names {
        match target_dir.find(session, name.id) {
            ModuleOrDirectory::None => {
                emit.error(ErrorComp::new(
                    format!(
                        "expected directory `{}` is not found in `{}`",
                        hir.name_str(name.id),
                        hir.name_str(source_package.name_id),
                    ),
                    SourceRange::new(origin_id, name.range),
                    None,
                ));
                return;
            }
            ModuleOrDirectory::Module(_) => {
                emit.error(ErrorComp::new(
                    format!(
                        "expected directory, found module `{}`",
                        hir.name_str(name.id),
                    ),
                    SourceRange::new(origin_id, name.range),
                    None,
                ));
                return;
            }
            ModuleOrDirectory::Directory(directory) => {
                target_dir = directory;
            }
        }
    }

    let target_id = match target_dir.find(session, module_name.id) {
        ModuleOrDirectory::None => {
            emit.error(ErrorComp::new(
                format!(
                    "expected module `{}` is not found in `{}`",
                    hir.name_str(module_name.id),
                    hir.name_str(source_package.name_id),
                ),
                SourceRange::new(origin_id, module_name.range),
                None,
            ));
            return;
        }
        ModuleOrDirectory::Module(module_id) => module_id,
        ModuleOrDirectory::Directory(_) => {
            emit.error(ErrorComp::new(
                format!(
                    "expected module, found directory `{}`",
                    hir.name_str(module_name.id),
                ),
                SourceRange::new(origin_id, module_name.range),
                None,
            ));
            return;
        }
    };

    if target_id == origin_id {
        emit.error(ErrorComp::new(
            format!(
                "importing module `{}` into itself is not allowed",
                hir.name_str(module_name.id)
            ),
            SourceRange::new(origin_id, module_name.range),
            None,
        ));
        return;
    }

    import_module(hir, emit, origin_id, target_id, module_name, import.rename);
    for symbol in import.symbols {
        import_symbol(hir, emit, origin_id, target_id, symbol);
    }
}

fn import_module<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    target_id: ModuleID,
    module_name: ast::Name<'ast>,
    rename: ast::SymbolRename<'ast>,
) {
    let module_alias = check_symbol_rename(hir, emit, origin_id, module_name, rename, false);
    let module_alias = match module_alias {
        Some(module_alias) => module_alias,
        None => return,
    };

    if let Some(existing) = hir.symbol_in_scope_source(origin_id, module_alias.id) {
        super::pass_1::error_name_already_defined(hir, emit, origin_id, module_alias, existing);
    } else {
        let symbol = Symbol::Imported {
            kind: SymbolKind::Module(target_id),
            import_range: module_alias.range,
        };
        hir.add_symbol(origin_id, module_alias.id, symbol);
    }
}

fn import_symbol<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    target_id: ModuleID,
    symbol: &ast::ImportSymbol<'ast>,
) {
    let symbol_alias = check_symbol_rename(hir, emit, origin_id, symbol.name, symbol.rename, true);
    let (symbol_alias, discarded) = match symbol_alias {
        Some(symbol_alias) => (symbol_alias, false),
        None => (symbol.name, true),
    };

    let kind = match hir.symbol_from_scope(origin_id, target_id, symbol.name) {
        Ok((kind, _)) => kind,
        Err(error) => {
            emit.error(error);
            return;
        }
    };

    if discarded {
        return;
    }

    if let Some(existing) = hir.symbol_in_scope_source(origin_id, symbol_alias.id) {
        super::pass_1::error_name_already_defined(hir, emit, origin_id, symbol_alias, existing);
    } else {
        let symbol = Symbol::Imported {
            kind,
            import_range: symbol_alias.range,
        };
        hir.add_symbol(origin_id, symbol_alias.id, symbol);
    }
}

fn check_symbol_rename<'hir, 'ast>(
    hir: &HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    name: ast::Name<'ast>,
    rename: ast::SymbolRename<'ast>,
    for_symbol: bool,
) -> Option<ast::Name<'ast>> {
    match rename {
        ast::SymbolRename::None => Some(name),
        ast::SymbolRename::Alias(alias) => {
            if name.id == alias.id {
                emit.warning(WarningComp::new(
                    format!(
                        "name alias `{}` is redundant, remove it",
                        hir.name_str(alias.id)
                    ),
                    SourceRange::new(origin_id, alias.range),
                    None,
                ));
            }
            Some(alias)
        }
        ast::SymbolRename::Discard(range) => {
            if for_symbol {
                emit.warning(WarningComp::new(
                    "name discard `_` is redundant, remove it",
                    SourceRange::new(origin_id, range),
                    None,
                ));
            }
            None
        }
    }
}
