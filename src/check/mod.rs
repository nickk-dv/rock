use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::SourceLoc;
use crate::err::error_new::{CompError, ErrorContext, Message};
use crate::hir::hir;
use crate::hir::hir_temp;

// @some structures are temporarily used to construct final Hir
// 1. scope creation from ast decl scopes (map of declarations)
// 2. imports from ast decl scopes        (proccess use declarations)
// 3. ? constants in decls
// 4. create "real" Hir scopes & add resolved variants of declarations

pub fn check<'ast, 'hir>(
    ctx: &CompCtx,
    ast: ast::Ast<'ast>,
) -> Result<hir::Hir<'hir>, Vec<CompError>> {
    let hir = hir::Hir::new();
    let mut hir_temp = hir_temp::HirTemp::new(ast);
    hir_pass_create_scope_tree(&mut hir_temp, ctx);
    Ok(hir)
}

fn hir_pass_create_scope_tree<'ast>(hir_temp: &mut hir_temp::HirTemp<'ast>, ctx: &CompCtx) {
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct ScopeTreeTask<'ast> {
        module: ast::Module<'ast>,
        parent: Option<hir_temp::ModID>,
    }

    // create path -> ast::Module map
    let mut module_map = HashMap::<&PathBuf, ast::Module<'ast>>::new();
    let mut taken_module_map = HashMap::<&PathBuf, SourceLoc>::new();
    let mut task_queue = Vec::<ScopeTreeTask>::new();

    for module in hir_temp.ast_modules().cloned() {
        module_map.insert(&ctx.file(module.file_id).path, module);
    }

    // add root module task
    let root_path: PathBuf = ["test", "main.lang"].iter().collect();
    match module_map.remove(&root_path) {
        Some(module) => task_queue.push(ScopeTreeTask {
            module,
            parent: None,
        }),
        None => {
            eprintln!("no root module found, add src/main.lang"); //@report
            return;
        }
    }

    // process scope tree tasks
    while let Some(task) = task_queue.pop() {
        let parent = match task.parent {
            Some(mod_id) => Some(hir_temp.get_mod(mod_id).from_id),
            None => None,
        };

        let scope_temp = hir_temp::ScopeTemp::new(parent, task.module);
        let scope_id = hir_temp.add_scope_temp(scope_temp);

        if let Some(mod_id) = task.parent {
            hir_temp.get_mod_mut(mod_id).target = Some(scope_id);
        }

        // @use macros when this is in a finished state
        for decl in hir_temp.get_scope_temp(scope_id).module_decls() {
            match decl {
                ast::Decl::Use(_) => {}
                ast::Decl::Mod(decl) => {
                    let mod_id = match hir_temp.get_scope_temp(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `mod`");
                            continue;
                        }
                        None => {
                            let mod_id = hir_temp.add_mod(hir_temp::ModData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                target: None,
                            });
                            let scope_temp = hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Mod(mod_id),
                                },
                            );
                            mod_id
                        }
                    };

                    let scope_temp = hir_temp.get_scope_temp(scope_id);
                    let mut scope_dir = ctx.file(scope_temp.module_file_id()).path.clone();
                    scope_dir.pop();

                    let mod_name = ctx.intern().get_str(decl.name.id);
                    let mod_filename = mod_name.to_string() + ".lang";

                    let mod_path_1 = scope_dir.join(mod_filename);
                    let mod_path_2 = scope_dir.join(mod_name).join("mod.lang");

                    let (module, path) =
                        match (module_map.get(&mod_path_1), module_map.get(&mod_path_2)) {
                            (Some(..), Some(..)) => {
                                let msg = format!(
                                    "module path is ambiguous:\n{:?}\n{:?}",
                                    mod_path_1, mod_path_2
                                );
                                eprintln!("{}", msg);
                                continue;
                            }
                            (None, None) => {
                                let msg = format!(
                                    "both module paths are missing:\n{:?}\n{:?}",
                                    mod_path_1, mod_path_2
                                );
                                eprintln!("{}", msg);
                                continue;
                            }
                            (Some(module), None) => (module, mod_path_1),
                            (None, Some(module)) => (module, mod_path_2),
                        };
                }
                ast::Decl::Proc(decl) => {
                    match hir_temp.get_scope_temp(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `proc`");
                        }
                        None => {
                            let scope_temp = hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Proc(decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Enum(decl) => {
                    match hir_temp.get_scope_temp(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `enum`");
                        }
                        None => {
                            let scope_temp = hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Enum(decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Union(decl) => {
                    match hir_temp.get_scope_temp(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `union`");
                        }
                        None => {
                            let scope_temp = hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Union(decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Struct(decl) => {
                    match hir_temp.get_scope_temp(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `struct`");
                        }
                        None => {
                            let scope_temp = hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Struct(decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Const(decl) => {
                    match hir_temp.get_scope_temp(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `const`");
                        }
                        None => {
                            let scope_temp = hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Const(decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Global(decl) => {
                    match hir_temp.get_scope_temp(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `global`");
                        }
                        None => {
                            let scope_temp = hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Global(decl),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}
