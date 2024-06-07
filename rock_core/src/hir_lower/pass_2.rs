use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::error::{ErrorComp, WarningComp};
use crate::hir;
use crate::session::PackageID;

pub fn resolve_imports<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for origin_id in hir.registry().module_ids() {
        let package_id = hir.module_package_id(origin_id);

        for item in hir.registry().module_ast(origin_id).items.iter().cloned() {
            if let ast::Item::Import(import) = item {
                resolve_import(hir, emit, package_id, origin_id, import);
            }
        }
    }
}

fn resolve_import<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    package_id: PackageID,
    origin_id: hir::ModuleID,
    import: &'ast ast::ImportItem<'ast>,
) {
    let target_package_id = if let Some(name) = import.package {
        if let Some(dep_id) = hir.get_package_dep_id(package_id, name) {
            dep_id
        } else {
            emit.error(ErrorComp::new(
                format!(
                    "package `{}` is not found in dependencies",
                    hir.name_str(name.id)
                ),
                hir.src(origin_id, name.range),
                None,
            ));
            return;
        }
    } else {
        package_id
    };

    let target_id =
        if let Some(target_id) = hir.get_package_module_id(target_package_id, import.module.id) {
            target_id
        } else {
            emit.error(ErrorComp::new(
                //@mention in which package we were looking into?
                // currently not fully descriptive @20.04.24
                format!(
                    "module `{}` is not found in package",
                    hir.name_str(import.module.id)
                ),
                hir.src(origin_id, import.module.range),
                None,
            ));
            return;
        };

    if target_id == origin_id {
        emit.error(ErrorComp::new(
            format!(
                "importing module `{}` into itself is redundant, remove this import",
                hir.name_str(import.module.id)
            ),
            hir.src(origin_id, import.module.range),
            None,
        ));
        return;
    }

    let alias_name = match import.alias {
        Some(alias) => {
            if import.module.id == alias.id {
                emit.warning(WarningComp::new(
                    format!("name alias `{}` is redundant", hir.name_str(alias.id)),
                    hir.src(origin_id, alias.range),
                    None,
                ));
            }
            alias
        }
        None => import.module,
    };

    match hir.scope_name_defined(origin_id, alias_name.id) {
        Some(existing) => {
            super::pass_1::name_already_defined_error(hir, emit, origin_id, alias_name, existing);
            return;
        }
        None => hir.add_symbol(
            origin_id,
            alias_name.id,
            Symbol::Imported {
                kind: SymbolKind::Module(target_id),
                import_range: alias_name.range,
            },
        ),
    }

    for symbol in import.symbols {
        let item_name = symbol.name;
        let alias_name = match symbol.alias {
            Some(alias) => {
                if symbol.name.id == alias.id {
                    emit.warning(WarningComp::new(
                        format!("name alias `{}` is redundant", hir.name_str(alias.id),),
                        hir.src(origin_id, alias.range),
                        None,
                    ));
                }
                alias
            }
            None => symbol.name,
        };

        if let Some((kind, source)) = hir.symbol_from_scope(emit, origin_id, target_id, item_name) {
            match hir.scope_name_defined(origin_id, alias_name.id) {
                Some(existing) => {
                    super::pass_1::name_already_defined_error(
                        hir, emit, origin_id, alias_name, existing,
                    );
                }
                None => hir.add_symbol(
                    origin_id,
                    alias_name.id,
                    Symbol::Imported {
                        kind,
                        import_range: alias_name.range,
                    },
                ),
            }
        }
    }
}
