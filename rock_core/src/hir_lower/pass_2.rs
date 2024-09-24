use super::context::{HirCtx, Symbol, SymbolKind};
use crate::ast;
use crate::error::SourceRange;
use crate::errors as err;
use crate::session::{ModuleID, ModuleOrDirectory};

pub fn resolve_imports(ctx: &mut HirCtx) {
    for import_id in ctx.registry.import_ids() {
        let origin_id = ctx.registry.import_data(import_id).origin_id;
        let import = ctx.registry.import_item(import_id);
        resolve_import(ctx, origin_id, import);
    }
}

fn resolve_import(ctx: &mut HirCtx, origin_id: ModuleID, import: &ast::ImportItem) {
    let mut source_package = ctx
        .session
        .pkg_storage
        .package(ctx.session.pkg_storage.module(origin_id).package_id);

    if let Some(package_name) = import.package {
        if let Some(dependency_id) = source_package.dependency(package_name.id) {
            source_package = ctx.session.pkg_storage.package(dependency_id);
        } else {
            let src = SourceRange::new(origin_id, package_name.range);
            let dep_name = ctx.name_str(package_name.id);
            let src_name = ctx.name_str(source_package.name_id);
            err::import_package_dependency_not_found(&mut ctx.emit, src, dep_name, src_name);
            return;
        }
    }

    assert!(!import.import_path.is_empty());
    let directory_count = import.import_path.len() - 1;
    let directory_names = &import.import_path[0..directory_count];
    let module_name = *import.import_path.last().unwrap();
    let mut target_dir = &source_package.src;

    for name in directory_names {
        match target_dir.find(&ctx.session.pkg_storage, name.id) {
            ModuleOrDirectory::None => {
                let src = SourceRange::new(origin_id, name.range);
                let dir_name = ctx.name_str(name.id);
                let pkg_name = ctx.name_str(source_package.name_id);
                err::import_expected_dir_not_found(&mut ctx.emit, src, dir_name, pkg_name);
                return;
            }
            ModuleOrDirectory::Module(_) => {
                let src = SourceRange::new(origin_id, name.range);
                let module_name = ctx.name_str(name.id);
                err::import_expected_dir_found_mod(&mut ctx.emit, src, module_name);
                return;
            }
            ModuleOrDirectory::Directory(directory) => {
                target_dir = directory;
            }
        }
    }

    let target_id = match target_dir.find(&ctx.session.pkg_storage, module_name.id) {
        ModuleOrDirectory::None => {
            let src = SourceRange::new(origin_id, module_name.range);
            let module_name = ctx.name_str(module_name.id);
            let pkg_name = ctx.name_str(source_package.name_id);
            err::import_expected_mod_not_found(&mut ctx.emit, src, module_name, pkg_name);
            return;
        }
        ModuleOrDirectory::Module(module_id) => module_id,
        ModuleOrDirectory::Directory(_) => {
            let src = SourceRange::new(origin_id, module_name.range);
            let dir_name = ctx.name_str(module_name.id);
            err::import_expected_mod_found_dir(&mut ctx.emit, src, dir_name);
            return;
        }
    };

    if target_id == origin_id {
        let src = SourceRange::new(origin_id, module_name.range);
        let module_name = ctx.name_str(module_name.id);
        err::import_module_into_itself(&mut ctx.emit, src, module_name);
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

    if let Err(error) = ctx
        .scope
        .already_defined_check(&ctx.registry, origin_id, module_alias)
    {
        error.emit(ctx);
        return;
    }

    let symbol = Symbol::Imported {
        kind: SymbolKind::Module(target_id),
        import_range: module_alias.range,
    };
    ctx.scope.add_symbol(origin_id, module_alias.id, symbol);
}

fn import_symbol(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    target_id: ModuleID,
    symbol: &ast::ImportSymbol,
) {
    let kind = match ctx
        .scope
        .symbol_from_scope(&ctx.registry, origin_id, target_id, symbol.name)
    {
        Ok(kind) => kind,
        Err(error) => {
            error.emit(ctx);
            return;
        }
    };

    let symbol_alias = check_symbol_rename(ctx, origin_id, symbol.name, symbol.rename, true);
    let symbol_alias = match symbol_alias {
        Some(symbol_alias) => symbol_alias,
        None => return,
    };

    if let Err(error) = ctx
        .scope
        .already_defined_check(&ctx.registry, origin_id, symbol_alias)
    {
        error.emit(ctx);
        return;
    }

    let symbol = Symbol::Imported {
        kind,
        import_range: symbol_alias.range,
    };
    ctx.scope.add_symbol(origin_id, symbol_alias.id, symbol);
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
                let src = SourceRange::new(origin_id, alias.range);
                let alias = ctx.name_str(alias.id);
                err::import_name_alias_redundant(&mut ctx.emit, src, alias);
            }
            Some(alias)
        }
        ast::SymbolRename::Discard(range) => {
            if for_symbol {
                let src = SourceRange::new(origin_id, range);
                err::import_name_discard_redundant(&mut ctx.emit, src);
            }
            None
        }
    }
}
