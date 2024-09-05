use super::context::{HirCtx, Symbol, SymbolKind};
use super::errors as err;
use crate::ast;
use crate::error::{ErrorComp, ErrorSink, SourceRange, WarningComp};
use crate::session::{ModuleID, ModuleOrDirectory, Session};

pub fn resolve_imports(ctx: &mut HirCtx, session: &Session) {
    for import_id in ctx.registry.import_ids() {
        let origin_id = ctx.registry.import_data(import_id).origin_id;
        let import = ctx.registry.import_item(import_id);
        resolve_import(ctx, session, origin_id, import);
    }
}

fn resolve_import(
    ctx: &mut HirCtx,
    session: &Session,
    origin_id: ModuleID,
    import: &ast::ImportItem,
) {
    let mut source_package = session.package(session.module(origin_id).package_id);

    if let Some(package_name) = import.package {
        if let Some(dependency_id) = source_package.dependency(package_name.id) {
            source_package = session.package(dependency_id);
        } else {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "package `{}` is not found in dependencies of `{}`",
                    ctx.name_str(package_name.id),
                    ctx.name_str(source_package.name_id),
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
                ctx.emit.error(ErrorComp::new(
                    format!(
                        "expected directory `{}` is not found in `{}`",
                        ctx.name_str(name.id),
                        ctx.name_str(source_package.name_id),
                    ),
                    SourceRange::new(origin_id, name.range),
                    None,
                ));
                return;
            }
            ModuleOrDirectory::Module(_) => {
                ctx.emit.error(ErrorComp::new(
                    format!(
                        "expected directory, found module `{}`",
                        ctx.name_str(name.id),
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
            ctx.emit.error(ErrorComp::new(
                format!(
                    "expected module `{}` is not found in `{}`",
                    ctx.name_str(module_name.id),
                    ctx.name_str(source_package.name_id),
                ),
                SourceRange::new(origin_id, module_name.range),
                None,
            ));
            return;
        }
        ModuleOrDirectory::Module(module_id) => module_id,
        ModuleOrDirectory::Directory(_) => {
            ctx.emit.error(ErrorComp::new(
                format!(
                    "expected module, found directory `{}`",
                    ctx.name_str(module_name.id),
                ),
                SourceRange::new(origin_id, module_name.range),
                None,
            ));
            return;
        }
    };

    if target_id == origin_id {
        ctx.emit.error(ErrorComp::new(
            format!(
                "importing module `{}` into itself is not allowed",
                ctx.name_str(module_name.id)
            ),
            SourceRange::new(origin_id, module_name.range),
            None,
        ));
        return;
    }

    import_module(ctx, origin_id, target_id, module_name, import.rename);
    for symbol in import.symbols {
        import_symbol(ctx, origin_id, target_id, symbol);
    }
}

fn import_module(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    target_id: ModuleID,
    module_name: ast::Name,
    rename: ast::SymbolRename,
) {
    let module_alias = check_symbol_rename(ctx, origin_id, module_name, rename, false);
    let module_alias = match module_alias {
        Some(module_alias) => module_alias,
        None => return,
    };

    if super::pass_1::name_already_defined_check(ctx, origin_id, module_alias).is_ok() {
        let symbol = Symbol::Imported {
            kind: SymbolKind::Module(target_id),
            import_range: module_alias.range,
        };
        ctx.scope.add_symbol(origin_id, module_alias.id, symbol);
    }
}

fn import_symbol(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    target_id: ModuleID,
    symbol: &ast::ImportSymbol,
) {
    let symbol_alias = check_symbol_rename(ctx, origin_id, symbol.name, symbol.rename, true);
    let (symbol_alias, discarded) = match symbol_alias {
        Some(symbol_alias) => (symbol_alias, false),
        None => (symbol.name, true),
    };

    let kind =
        match ctx
            .scope
            .symbol_from_scope(ctx, &ctx.registry, origin_id, target_id, symbol.name)
        {
            Ok((kind, _)) => kind,
            Err(error) => {
                ctx.emit.error(error);
                return;
            }
        };

    if discarded {
        return;
    }

    if super::pass_1::name_already_defined_check(ctx, origin_id, symbol_alias).is_ok() {
        let symbol = Symbol::Imported {
            kind,
            import_range: symbol_alias.range,
        };
        ctx.scope.add_symbol(origin_id, symbol_alias.id, symbol);
    }
}

fn check_symbol_rename(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    name: ast::Name,
    rename: ast::SymbolRename,
    for_symbol: bool,
) -> Option<ast::Name> {
    match rename {
        ast::SymbolRename::None => Some(name),
        ast::SymbolRename::Alias(alias) => {
            if name.id == alias.id {
                ctx.emit.warning(WarningComp::new(
                    format!(
                        "name alias `{}` is redundant, remove it",
                        ctx.name_str(alias.id)
                    ),
                    SourceRange::new(origin_id, alias.range),
                    None,
                ));
            }
            Some(alias)
        }
        ast::SymbolRename::Discard(range) => {
            if for_symbol {
                ctx.emit.warning(WarningComp::new(
                    "name discard `_` is redundant, remove it",
                    SourceRange::new(origin_id, range),
                    None,
                ));
            }
            None
        }
    }
}
