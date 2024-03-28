use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

struct UseTask<'ast> {
    resolved: bool,
    use_item: &'ast ast::UseItem<'ast>,
}

pub fn run<'hir>(hir: &mut HirData<'hir, '_>, emit: &mut HirEmit<'hir>) {
    let mut use_tasks = Vec::new();

    for origin_id in hir.scope_ids() {
        use_tasks.clear();

        for item in hir.scope_ast_items(origin_id) {
            if let ast::Item::Use(use_item) = item {
                use_tasks.push(UseTask {
                    resolved: false,
                    use_item,
                });
            }
        }

        loop {
            let mut made_progress = false;
            for task in use_tasks.iter_mut() {
                if task.resolved {
                    continue;
                }
                task.resolved = try_process_use_item(hir, emit, origin_id, task.use_item);
                if task.resolved {
                    made_progress = true;
                }
            }
            if !made_progress {
                break;
            }
        }

        for task in use_tasks.iter() {
            if task.resolved {
                continue;
            }
            if let Some(name) = task.use_item.path.names.iter().next() {
                emit.error(
                    ErrorComp::error(format!("module `{}` is not found", hir.name_str(name.id)))
                        .context(hir.src(origin_id, name.range)),
                );
            }
        }
    }
}

fn try_process_use_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    use_item: &'ast ast::UseItem<'ast>,
) -> bool {
    let path = use_item.path;

    if path.kind == ast::PathKind::None {
        let name = path.names[0];
        let symbol = hir.symbol_from_scope(origin_id, origin_id, path.kind, name.id);
        if symbol.is_none() {
            return false;
        }
    }
    let target_id = match super::pass_5::path_resolve_as_module_path(hir, emit, origin_id, path) {
        Some(it) => it,
        None => return true,
    };

    for use_symbol in use_item.symbols {
        let item_name = use_symbol.name;
        let alias_name = match use_symbol.alias {
            Some(alias) => alias,
            None => use_symbol.name,
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
    true
}
