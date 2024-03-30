use super::hir_build::{HirData, HirEmit, SymbolKind};
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

pub fn run<'hir>(hir: &mut HirData<'hir, '_>, emit: &mut HirEmit<'hir>) {
    for origin_id in hir.scope_ids() {
        for item in hir.scope_ast_items(origin_id) {
            if let ast::Item::Import(import) = item {
                resolve_import(hir, emit, origin_id, import);
            }
        }
    }
}

fn resolve_import<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    import: &'ast ast::ImportItem<'ast>,
) {
    let target_id = match hir.get_module_id(import.module.id) {
        Some(it) => it,
        None => {
            emit.error(
                ErrorComp::error(format!(
                    "module `{}` is not found in current package",
                    hir.name_str(import.module.id)
                ))
                .context(hir.src(origin_id, import.module.range)),
            );
            return;
        }
    };

    if target_id == origin_id {
        emit.error(
            ErrorComp::error(format!(
                "importing module `{}` into itself is redundant, remove this import",
                hir.name_str(import.module.id)
            ))
            .context(hir.src(origin_id, import.module.range)),
        );
        return;
    }

    let alias_name = match import.alias {
        Some(alias) => alias,
        None => import.module,
    };

    match hir.scope_name_defined(origin_id, alias_name.id) {
        Some(existing) => {
            super::pass_1::name_already_defined_error(hir, emit, origin_id, alias_name, existing);
            return;
        }
        None => hir.scope_add_imported(origin_id, alias_name, SymbolKind::Module(target_id)),
    }

    for symbol in import.symbols {
        let item_name = symbol.name;
        let alias_name = match symbol.alias {
            Some(alias) => alias,
            None => symbol.name,
        };

        match hir.symbol_from_scope(origin_id, target_id, item_name.id) {
            Some((kind, ..)) => match hir.scope_name_defined(origin_id, alias_name.id) {
                Some(existing) => {
                    super::pass_1::name_already_defined_error(
                        hir, emit, origin_id, alias_name, existing,
                    );
                }
                None => hir.scope_add_imported(origin_id, alias_name, kind),
            },
            None => {
                emit.error(
                    ErrorComp::error(format!(
                        "name `{}` is not found in module", //@display module paths?
                        hir.name_str(item_name.id)
                    ))
                    .context(hir.src(origin_id, item_name.range)),
                );
            }
        }
    }
}
