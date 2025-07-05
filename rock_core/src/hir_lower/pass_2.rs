use super::check_path;
use super::context::HirCtx;
use super::scope::{Symbol, SymbolOrModule};
use crate::ast;
use crate::errors as err;
use crate::session::{ModuleID, ModuleOrDirectory};

pub fn resolve_imports(ctx: &mut HirCtx) {
    for import_id in ctx.registry.import_ids() {
        let data = ctx.registry.import_data(import_id);
        let import = ctx.registry.import_item(import_id);
        ctx.scope.origin = data.origin_id;
        resolve_import(ctx, import);
    }
}

fn resolve_import(ctx: &mut HirCtx, import: &ast::ImportItem) {
    let source_package_id = ctx.session.module.get(ctx.scope.origin).origin;
    let mut source_package = ctx.session.graph.package(source_package_id);

    if let Some(package_name) = import.package {
        if let Some(dep_id) = ctx.session.graph.find_package_dep(source_package_id, package_name.id)
        {
            source_package = ctx.session.graph.package(dep_id);
        } else {
            let src = ctx.src(package_name.range);
            let dep_name = ctx.name(package_name.id);
            let src_name = ctx.name(source_package.name_id);
            err::import_package_dependency_not_found(&mut ctx.emit, src, dep_name, src_name);
            return;
        }
    };

    assert!(!import.import_path.is_empty());
    let directory_count = import.import_path.len() - 1;
    let directory_names = &import.import_path[0..directory_count];
    let module_name = import.import_path.last().copied().unwrap();
    let mut target_dir = &source_package.src;

    for name in directory_names {
        match target_dir.find(ctx.session, name.id) {
            ModuleOrDirectory::None => {
                let src = ctx.src(name.range);
                let dir_name = ctx.name(name.id);
                let pkg_name = ctx.name(source_package.name_id);
                err::import_expected_dir_not_found(&mut ctx.emit, src, dir_name, pkg_name);
                return;
            }
            ModuleOrDirectory::Module(_) => {
                let src = ctx.src(name.range);
                let mod_name = ctx.name(name.id);
                err::import_expected_dir_found_mod(&mut ctx.emit, src, mod_name);
                return;
            }
            ModuleOrDirectory::Directory(directory) => {
                target_dir = directory;
            }
        }
    }

    let target_id = match target_dir.find(ctx.session, module_name.id) {
        ModuleOrDirectory::None => {
            let src = ctx.src(module_name.range);
            let mod_name = ctx.name(module_name.id);
            let pkg_name = ctx.name(source_package.name_id);
            err::import_expected_mod_not_found(&mut ctx.emit, src, mod_name, pkg_name);
            return;
        }
        ModuleOrDirectory::Module(module_id) => module_id,
        ModuleOrDirectory::Directory(_) => {
            let src = ctx.src(module_name.range);
            let dir_name = ctx.name(module_name.id);
            err::import_expected_mod_found_dir(&mut ctx.emit, src, dir_name);
            return;
        }
    };

    if target_id == ctx.scope.origin {
        let src = ctx.src(module_name.range);
        let mod_name = ctx.name(module_name.id);
        err::import_module_into_itself(&mut ctx.emit, src, mod_name);
        return;
    }

    let _ = import_module(ctx, import, target_id, module_name);
    for symbol in import.symbols {
        let _ = import_symbol(ctx, target_id, symbol);
    }
}

fn import_module(
    ctx: &mut HirCtx,
    import: &ast::ImportItem,
    target_id: ModuleID,
    module_name: ast::Name,
) -> Result<(), ()> {
    let alias = check_symbol_rename(ctx, module_name, import.rename)?;
    ctx.scope.check_already_defined_global(alias, ctx.session, &ctx.registry, &mut ctx.emit)?;

    let origin_id = ctx.scope.origin;
    let was_used = !import.symbols.is_empty(); //dont warn if module import has symbols
    let symbol = Symbol::ImportedModule(target_id, alias.range, was_used);
    ctx.scope.global.add_symbol(origin_id, alias.id, symbol);
    Ok(())
}

fn import_symbol(
    ctx: &mut HirCtx,
    target_id: ModuleID,
    symbol: &ast::ImportSymbol,
) -> Result<(), ()> {
    let origin_id = ctx.scope.origin;
    let kind = ctx.scope.global.find_symbol(
        origin_id,
        target_id,
        symbol.name,
        ctx.session,
        &ctx.registry,
        &mut ctx.emit,
    )?;

    let alias = check_symbol_rename(ctx, symbol.name, symbol.rename)?;
    ctx.scope.check_already_defined_global(alias, ctx.session, &ctx.registry, &mut ctx.emit)?;

    let symbol = match kind {
        SymbolOrModule::Symbol(symbol_id) => {
            check_path::set_symbol_used_flag(ctx, symbol_id);
            Symbol::Imported(symbol_id, alias.range, false)
        }
        SymbolOrModule::Module(module_id) => Symbol::ImportedModule(module_id, alias.range, false),
    };
    ctx.scope.global.add_symbol(origin_id, alias.id, symbol);
    Ok(())
}

fn check_symbol_rename(
    ctx: &mut HirCtx,
    name: ast::Name,
    rename: ast::SymbolRename,
) -> Result<ast::Name, ()> {
    match rename {
        ast::SymbolRename::None => Ok(name),
        ast::SymbolRename::Alias(alias) => {
            if name.id == alias.id {
                let src = ctx.src(alias.range);
                let alias = ctx.name(alias.id);
                err::import_name_alias_redundant(&mut ctx.emit, src, alias);
            }
            Ok(alias)
        }
        ast::SymbolRename::Discard(_) => Err(()),
    }
}
