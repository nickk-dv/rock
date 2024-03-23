use super::hir_builder as hb;
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

struct UseTask<'ast> {
    resolved: bool,
    decl: &'ast ast::UseDecl<'ast>,
}

pub fn run(hb: &mut hb::HirBuilder) {
    let mut use_tasks = Vec::new();

    for scope_id in hb.scope_ids() {
        use_tasks.clear();

        for decl in hb.scope_ast_decls(scope_id) {
            if let ast::Decl::Use(use_decl) = decl {
                use_tasks.push(UseTask {
                    resolved: false,
                    decl: use_decl,
                });
            }
        }

        loop {
            let mut made_progress = false;
            for task in use_tasks.iter_mut() {
                if task.resolved {
                    continue;
                }
                task.resolved = try_process_use_decl(hb, scope_id, task.decl);
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
            for name in task.decl.path.names {
                hb.error(
                    ErrorComp::error(format!("module `{}` is not found", hb.name_str(name.id)))
                        .context(hb.src(scope_id, name.range)),
                );
                break;
            }
        }
    }
}

fn try_process_use_decl<'ctx, 'ast, 'hir>(
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    origin_id: hir::ScopeID,
    decl: &'ast ast::UseDecl<'ast>,
) -> bool {
    let path = decl.path;

    if path.kind == ast::PathKind::None {
        let name = path.names.first().cloned().expect("");
        let symbol = hb.symbol_from_scope(origin_id, origin_id, path.kind, name.id);
        if symbol.is_none() {
            return false;
        }
    }
    let target_id = match super::pass_5::path_resolve_as_module_path(hb, origin_id, path) {
        Some(it) => it,
        None => return true,
    };

    for use_symbol in decl.symbols {
        let item_name = use_symbol.name;
        let alias_name = match use_symbol.alias {
            Some(alias) => alias,
            None => use_symbol.name,
        };

        match hb.symbol_from_scope(origin_id, target_id, path.kind, item_name.id) {
            Some((kind, ..)) => match hb.scope_name_defined(origin_id, alias_name.id) {
                Some(existing) => {
                    super::pass_1::name_already_defined_error(hb, origin_id, alias_name, existing);
                }
                None => hb.scope_add_imported(origin_id, alias_name, kind),
            },
            None => {
                hb.error(
                    ErrorComp::error(format!(
                        "name `{}` is not found in module",
                        hb.name_str(item_name.id)
                    ))
                    .context(hb.src(origin_id, item_name.range)),
                );
            }
        }
    }
    true
}
