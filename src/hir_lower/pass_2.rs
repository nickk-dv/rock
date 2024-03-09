use super::pass_1;
use crate::ast::ast;
use crate::err::error_new::{ErrorComp, ErrorSeverity};
use crate::hir::hir_builder as hb;
use crate::text_range::TextRange;

#[derive(Default)]
struct Pass {
    errors: Vec<ErrorComp>,
}

struct UseTask<'ast> {
    resolved: bool,
    decl: &'ast ast::UseDecl<'ast>,
}

pub fn run(hb: &mut hb::HirBuilder) -> Vec<ErrorComp> {
    let mut p = Pass::default();
    for scope_id in hb.scope_ids() {
        let mut use_tasks = Vec::new();

        for decl in hb.get_scope(scope_id).module_decls() {
            if let ast::Decl::Use(use_decl) = decl {
                use_tasks.push(UseTask {
                    resolved: false,
                    decl: use_decl,
                });
            }
        }

        loop {
            let mut new_progress = false;
            for task in use_tasks.iter_mut() {
                if task.resolved {
                    continue;
                }
                task.resolved = try_process_use_decl(&mut p, hb, scope_id, task.decl);
                if task.resolved {
                    new_progress = true;
                }
            }
            if !new_progress {
                break;
            }
        }

        let scope = hb.get_scope(scope_id);
        for task in use_tasks.iter() {
            if task.resolved {
                continue;
            }
            for name in task.decl.path.names.iter() {
                p.errors.push(ErrorComp::new(
                    format!("module `{}` is not found", hb.ctx.intern().get_str(name.id)).into(),
                    ErrorSeverity::Error,
                    scope.source(name.range),
                ));
                break;
            }
        }
    }
    p.errors
}

// @try iteration of first module name is not done yet
// store progress in pass and compare it to know when to stop iteration
// + know which use declarations ware already fully evaluated (using an option might be fine)
fn try_process_use_decl<'ctx, 'ast, 'hir>(
    p: &mut Pass,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::UseDecl<'ast>,
) -> bool {
    let from_id = match try_resolve_use_path(p, hb, scope_id, decl.path) {
        Ok(Some(from_id)) => from_id,
        Ok(None) => return true,
        Err(()) => return false,
    };

    for use_name in decl.symbols.iter() {
        let from_scope = hb.get_scope(from_id);

        let alias_name = match use_name.alias {
            Some(alias) => alias,
            None => use_name.name,
        };

        match from_scope.get_symbol(use_name.name.id) {
            Some(hb::Symbol::Defined { kind }) => {
                if !pass_1::name_already_defined_error(&mut p.errors, hb, scope_id, alias_name) {
                    let origin_scope = hb.get_scope_mut(scope_id);
                    origin_scope.add_symbol(
                        alias_name.id,
                        hb::Symbol::Imported {
                            kind,
                            import: alias_name.range,
                        },
                    )
                }
            }
            _ => {
                // @duplicate error with path resolve code
                // maybe standartize error formats? and create simpler api to feed related positional / name data
                // format strings can be stored else-where, along the Severity

                // @in general need to rework error system to allow main markers to have messages
                // rework cli output and support warnings
                // sort source range locs if in same file + dont display file link twice
                // display main error todo link, with hints being in any order based on lexical order
                let origin_scope = hb.get_scope(scope_id);
                p.errors.push(ErrorComp::new(
                    format!(
                        "name `{}` is not found in module", //@support showing module paths in all errors
                        hb.ctx.intern().get_str(use_name.name.id)
                    )
                    .into(),
                    ErrorSeverity::Error,
                    origin_scope.source(use_name.name.range),
                ));
            }
        }
    }
    true
}

// @visibility rules are ignored
fn try_resolve_use_path<'ctx, 'ast, 'hir>(
    p: &mut Pass,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    path: &'ast ast::Path,
) -> Result<Option<hb::ScopeID>, ()> {
    let origin_scope = hb.get_scope(scope_id);

    let (mut from_id, mut allow_retry) = match path.kind {
        ast::PathKind::None => (scope_id, true),
        ast::PathKind::Super => match origin_scope.parent() {
            Some(parent_id) => (parent_id, false),
            None => {
                let mut range = TextRange::empty_at(path.range_start);
                range.extend_by(5.into());
                p.errors.push(ErrorComp::new(
                    "parent module `super` doesnt exist for the root module".into(),
                    ErrorSeverity::Error,
                    origin_scope.source(range),
                ));
                return Ok(None);
            }
        },
        ast::PathKind::Package => (hb::ROOT_SCOPE_ID, false),
    };

    for name in path.names {
        let from_scope = hb.get_scope(from_id);

        match from_scope.get_symbol(name.id) {
            // copy pasted Defined code, not checking if from_scope and origin_scope match
            // hack to get out of order imports working
            Some(hb::Symbol::Imported { kind, import }) => match kind {
                hb::SymbolKind::Mod(id) => {
                    let mod_data = hb.get_mod(id);
                    if let Some(target) = mod_data.target {
                        from_id = target;
                    } else {
                        p.errors.push(ErrorComp::new(
                            format!(
                                "module `{}` is missing its associated file",
                                hb.ctx.intern().get_str(name.id)
                            )
                            .into(),
                            ErrorSeverity::Error,
                            origin_scope.source(name.range),
                        ));
                        return Ok(None);
                    }
                }
                _ => {
                    // add info hint to its declaration or apperance if its imported
                    p.errors.push(ErrorComp::new(
                        format!("`{}` is not a module", hb.ctx.intern().get_str(name.id)).into(),
                        ErrorSeverity::Error,
                        origin_scope.source(name.range),
                    ));
                    return Ok(None);
                }
            },
            //@ignoring imported which might be valid if we are querying from origin_scope
            // need a better HirBuilder api to get symbol From or Within Scope
            // to simplify handling of Defined / Imported, also might handle Visibility rules in that api
            Some(hb::Symbol::Defined { kind }) => match kind {
                hb::SymbolKind::Mod(id) => {
                    let mod_data = hb.get_mod(id);
                    if let Some(target) = mod_data.target {
                        from_id = target;
                    } else {
                        p.errors.push(ErrorComp::new(
                            format!(
                                "module `{}` is missing its associated file",
                                hb.ctx.intern().get_str(name.id)
                            )
                            .into(),
                            ErrorSeverity::Error,
                            origin_scope.source(name.range),
                        ));
                        return Ok(None);
                    }
                }
                _ => {
                    // add info hint to its declaration or apperance if its imported
                    p.errors.push(ErrorComp::new(
                        format!("`{}` is not a module", hb.ctx.intern().get_str(name.id)).into(),
                        ErrorSeverity::Error,
                        origin_scope.source(name.range),
                    ));
                    return Ok(None);
                }
            },
            _ => {
                if allow_retry {
                    return Err(());
                }
                p.errors.push(ErrorComp::new(
                    format!("module `{}` is not found", hb.ctx.intern().get_str(name.id)).into(),
                    ErrorSeverity::Error,
                    origin_scope.source(name.range),
                ));
                return Ok(None);
            }
        };
        allow_retry = false;
    }

    Ok(Some(from_id))
}
