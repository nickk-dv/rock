use super::hir_build::{HirData, HirEmit};
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
    // import.name <- find that this module file exists
    // import.alias <- this name should be not already in scope

    for symbol in import.symbols {
        // symbol.name <- find that this symbol exists in target module
        // symbol.alias <- add it to current module if available under this name

        let item_name = symbol.name;
        let alias_name = match symbol.alias {
            Some(alias) => alias,
            None => symbol.name,
        };

        match hir.symbol_from_scope(origin_id, target_id, path.kind, item_name.id) {
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
                        "name `{}` is not found in module",
                        hir.name_str(item_name.id)
                    ))
                    .context(hir.src(origin_id, item_name.range)),
                );
            }
        }
    }
}
