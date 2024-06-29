use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::error::{ErrorComp, SourceRange, WarningComp};
use crate::session::{ModuleID, ModuleOrDirectory, Session};

pub fn resolve_imports<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
) {
    for origin_id in session.module_ids() {
        let module_ast = hir.ast_module(origin_id);
        for item in module_ast.items.iter().copied() {
            if let ast::Item::Import(import) = item {
                resolve_import(hir, emit, session, origin_id, import);
            }
        }
    }
}

fn resolve_import<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
    origin_id: ModuleID,
    import: &'ast ast::ImportItem<'ast>,
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
    let last_name = *import.import_path.last().unwrap();
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

    let target_id = match target_dir.find(session, last_name.id) {
        ModuleOrDirectory::None => {
            emit.error(ErrorComp::new(
                format!(
                    "expected module `{}` is not found in `{}`",
                    hir.name_str(last_name.id),
                    hir.name_str(source_package.name_id),
                ),
                SourceRange::new(origin_id, last_name.range),
                None,
            ));
            return;
        }
        ModuleOrDirectory::Module(module_id) => module_id,
        ModuleOrDirectory::Directory(_) => {
            emit.error(ErrorComp::new(
                format!(
                    "expected module, found directory `{}`",
                    hir.name_str(last_name.id),
                ),
                SourceRange::new(origin_id, last_name.range),
                None,
            ));
            return;
        }
    };

    if target_id == origin_id {
        emit.error(ErrorComp::new(
            format!(
                "importing module `{}` into itself is redundant, remove this import",
                hir.name_str(last_name.id)
            ),
            SourceRange::new(origin_id, last_name.range),
            None,
        ));
        return;
    }

    let module_alias = name_alias_check(hir, emit, origin_id, last_name, import.alias);

    match hir.symbol_in_scope_source(origin_id, module_alias.id) {
        Some(existing) => {
            super::pass_1::error_name_already_defined(hir, emit, origin_id, module_alias, existing);
        }
        None => hir.add_symbol(
            origin_id,
            module_alias.id,
            Symbol::Imported {
                kind: SymbolKind::Module(target_id),
                import_range: module_alias.range,
            },
        ),
    }

    for symbol in import.symbols {
        let symbol_alias = name_alias_check(hir, emit, origin_id, symbol.name, symbol.alias);
        let found_symbol = hir.symbol_from_scope(origin_id, target_id, symbol.name);

        match found_symbol {
            Err(error) => emit.error(error),
            Ok((kind, _)) => match hir.symbol_in_scope_source(origin_id, symbol_alias.id) {
                Some(existing) => {
                    super::pass_1::error_name_already_defined(
                        hir,
                        emit,
                        origin_id,
                        symbol_alias,
                        existing,
                    );
                }
                None => hir.add_symbol(
                    origin_id,
                    symbol_alias.id,
                    Symbol::Imported {
                        kind,
                        import_range: symbol_alias.range,
                    },
                ),
            },
        }
    }
}

fn name_alias_check(
    hir: &mut HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    name: ast::Name,
    name_alias: Option<ast::Name>,
) -> ast::Name {
    if let Some(alias) = name_alias {
        if alias.id == name.id {
            emit.warning(WarningComp::new(
                format!(
                    "name alias `{}` is redundant, remove it",
                    hir.name_str(alias.id)
                ),
                SourceRange::new(origin_id, alias.range),
                None,
            ));
        }
        alias
    } else {
        name
    }
}
