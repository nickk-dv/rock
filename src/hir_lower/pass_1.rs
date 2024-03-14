use super::hir_builder as hb;
use crate::ast::ast;
use crate::err::error_new::{ErrorComp, SourceRange};
use crate::hir;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
struct Pass<'ast> {
    task_queue: Vec<ScopeTreeTask<'ast>>,
    module_map: HashMap<PathBuf, ModuleStatus<'ast>>,
}

struct ScopeTreeTask<'ast> {
    module: ast::Module<'ast>,
    parent: Option<hb::ModID>,
}

#[derive(Copy, Clone)]
enum ModuleStatus<'ast> {
    Taken(SourceRange),
    Available(ast::Module<'ast>),
}

pub fn run(hb: &mut hb::HirBuilder) {
    let mut p = Pass::default();
    make_module_path_map(&mut p, hb);
    add_root_scope_task(&mut p);
    while let Some(task) = p.task_queue.pop() {
        process_scope_task(&mut p, hb, task);
    }
}

fn make_module_path_map<'ast>(p: &mut Pass<'ast>, hb: &hb::HirBuilder<'_, 'ast, '_>) {
    for module in hb.ast_modules().cloned() {
        p.module_map.insert(
            hb.ctx().file(module.file_id).path.clone(),
            ModuleStatus::Available(module),
        );
    }
}

fn add_root_scope_task(p: &mut Pass) {
    let root_path: PathBuf = ["test", "main.lang"].iter().collect();
    match p.module_map.remove(&root_path) {
        Some(status) => match status {
            ModuleStatus::Available(module) => {
                p.task_queue.push(ScopeTreeTask {
                    module,
                    parent: None,
                });
            }
            ModuleStatus::Taken(..) => unreachable!("root module cannot be taken"),
        },
        None => {
            //@ reporting without main source range is not supported yet @03/05/24
            //@allow errors without any context @03/11/24
            eprintln!("no root module found, add src/main.lang");
        }
    }
}

fn process_scope_task<'ast>(
    p: &mut Pass,
    hb: &mut hb::HirBuilder<'_, 'ast, '_>,
    task: ScopeTreeTask<'ast>,
) {
    let parent = match task.parent {
        Some(mod_id) => Some(hb.get_mod(mod_id).from_id),
        None => None,
    };

    let scope = hb::Scope::new(parent, task.module);
    let scope_id = hb.add_scope(scope);

    if let Some(mod_id) = task.parent {
        hb.get_mod_mut(mod_id).target = Some(scope_id);
    }

    for decl in hb.get_scope(scope_id).ast_decls() {
        match decl {
            ast::Decl::Use(..) => {
                continue;
            }
            ast::Decl::Mod(decl) => {
                if !name_already_defined_error(hb, scope_id, decl.name) {
                    let data = hb::ModData {
                        from_id: scope_id,
                        vis: decl.vis,
                        name: decl.name,
                        target: None,
                    };
                    let (symbol, id) = hb.add_mod(data);
                    hb.get_scope_mut(scope_id).add_symbol(decl.name.id, symbol);
                    add_scope_task_from_mod_decl(p, hb, scope_id, decl, id);
                }
            }
            ast::Decl::Proc(decl) => {
                if !name_already_defined_error(hb, scope_id, decl.name) {
                    let data = hir::ProcData {
                        from_id: scope_id,
                        vis: decl.vis,
                        name: decl.name,
                        params: &[],
                        is_variadic: decl.is_variadic,
                        return_ty: hir::Type::Error,
                        block: None,
                        body: hir::ProcBody { locals: &[] },
                    };
                    let symbol = hb.add_proc(decl, data);
                    hb.get_scope_mut(scope_id).add_symbol(decl.name.id, symbol);
                }
            }
            ast::Decl::Enum(decl) => {
                if !name_already_defined_error(hb, scope_id, decl.name) {
                    let data = hir::EnumData {
                        from_id: scope_id,
                        vis: decl.vis,
                        name: decl.name,
                        variants: &[],
                    };
                    let symbol = hb.add_enum(decl, data);
                    hb.get_scope_mut(scope_id).add_symbol(decl.name.id, symbol);
                }
            }
            ast::Decl::Union(decl) => {
                if !name_already_defined_error(hb, scope_id, decl.name) {
                    let data = hir::UnionData {
                        from_id: scope_id,
                        vis: decl.vis,
                        name: decl.name,
                        members: &[],
                    };
                    let symbol = hb.add_union(decl, data);
                    hb.get_scope_mut(scope_id).add_symbol(decl.name.id, symbol);
                }
            }
            ast::Decl::Struct(decl) => {
                if !name_already_defined_error(hb, scope_id, decl.name) {
                    let data = hir::StructData {
                        from_id: scope_id,
                        vis: decl.vis,
                        name: decl.name,
                        fields: &[],
                    };
                    let symbol = hb.add_struct(decl, data);
                    hb.get_scope_mut(scope_id).add_symbol(decl.name.id, symbol);
                }
            }
            ast::Decl::Const(decl) => {
                if !name_already_defined_error(hb, scope_id, decl.name) {
                    let data = hir::ConstData {
                        from_id: scope_id,
                        vis: decl.vis,
                        name: decl.name,
                        ty: hir::Type::Error,
                        value: hb::DUMMY_CONST_EXPR_ID,
                    };
                    let symbol = hb.add_const(decl, data);
                    hb.get_scope_mut(scope_id).add_symbol(decl.name.id, symbol);
                }
            }
            ast::Decl::Global(decl) => {
                if !name_already_defined_error(hb, scope_id, decl.name) {
                    let data = hir::GlobalData {
                        from_id: scope_id,
                        vis: decl.vis,
                        name: decl.name,
                        ty: hir::Type::Error,
                        value: hb::DUMMY_CONST_EXPR_ID,
                    };
                    let symbol = hb.add_global(decl, data);
                    hb.get_scope_mut(scope_id).add_symbol(decl.name.id, symbol);
                }
            }
        }
    }
}

pub fn name_already_defined_error(
    hb: &mut hb::HirBuilder,
    scope_id: hir::ScopeID,
    name: ast::Ident,
) -> bool {
    let scope = hb.get_scope(scope_id);
    let existing = match scope.get_symbol(name.id) {
        Some(existing) => existing,
        None => return false,
    };
    // @add marker for error span with "name redefinition"
    // to fully explain this error
    // currently marker are not possible on main error message source loc
    hb.error(
        ErrorComp::error(format!(
            "name `{}` is defined multiple times",
            hb.name_str(name.id)
        ))
        .context(scope.source(name.range))
        .context_info(
            "existing definition",
            scope.source(hb.symbol_range(existing)),
        ),
    );
    true
}

fn add_scope_task_from_mod_decl(
    p: &mut Pass,
    hb: &mut hb::HirBuilder,
    scope_id: hir::ScopeID,
    decl: &ast::ModDecl,
    id: hb::ModID,
) {
    let scope = hb.get_scope(scope_id);
    let mut scope_dir = hb.ctx().file(scope.file_id()).path.clone();
    scope_dir.pop();

    let mod_name = hb.name_str(decl.name.id);
    let mod_filename = mod_name.to_string() + ".lang";
    let mod_path_1 = scope_dir.join(mod_filename);
    let mod_path_2 = scope_dir.join(mod_name).join("mod.lang");

    let (status, chosen_path) = match (
        p.module_map.get(&mod_path_1).cloned(),
        p.module_map.get(&mod_path_2).cloned(),
    ) {
        (Some(..), Some(..)) => {
            hb.error(
                ErrorComp::error(format!(
                    "only one possible module path can exist:\n{:?} or {:?}",
                    mod_path_1, mod_path_2
                ))
                .context(scope.source(decl.name.range)),
            );
            return;
        }
        (None, None) => {
            hb.error(
                ErrorComp::error(format!(
                    "both possible module paths are missing:\n{:?} or {:?}",
                    mod_path_1, mod_path_2
                ))
                .context(scope.source(decl.name.range)),
            );
            return;
        }
        (Some(status), None) => (status, mod_path_1),
        (None, Some(status)) => (status, mod_path_2),
    };

    match status {
        ModuleStatus::Available(module) => {
            let replaced = p.module_map.insert(
                chosen_path,
                ModuleStatus::Taken(scope.source(decl.name.range)),
            );
            assert!(replaced.is_some());
            p.task_queue.push(ScopeTreeTask {
                module,
                parent: Some(id),
            });
        }
        ModuleStatus::Taken(src) => {
            hb.error(
                ErrorComp::error(format!(
                    "module `{}` is already taken by other mod declaration",
                    hb.name_str(decl.name.id)
                ))
                .context(scope.source(decl.name.range))
                .context_info("taken by this module declaration", src),
            );
        }
    }
}
