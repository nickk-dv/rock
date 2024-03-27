use super::hir_builder as hb;
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::session::Session;
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

pub fn run(hb: &mut hb::HirBuilder, session: &Session) {
    let mut p = Pass::default();
    make_module_path_map(&mut p, hb, session);
    add_root_scope_task(&mut p, hb, session);
    while let Some(task) = p.task_queue.pop() {
        process_scope_task(&mut p, hb, session, task);
    }
}

fn make_module_path_map<'ast>(
    p: &mut Pass<'ast>,
    hb: &hb::HirBuilder<'_, 'ast>,
    session: &Session,
) {
    for module in hb.ast_modules() {
        p.module_map.insert(
            session.file(module.file_id).path.clone(),
            ModuleStatus::Available(*module),
        );
    }
}

fn add_root_scope_task(p: &mut Pass, hb: &mut hb::HirBuilder, session: &Session) {
    let root_path = if session.package().is_binary {
        session.cwd().join("src").join("main.rock")
    } else {
        session.cwd().join("src").join("lib.rock")
    };

    match p.module_map.remove(&root_path) {
        Some(status) => match status {
            ModuleStatus::Available(module) => {
                p.task_queue.push(ScopeTreeTask {
                    module,
                    parent: None,
                });
            }
            ModuleStatus::Taken(..) => panic!("root module cannot be taken"),
        },
        None => {
            hb.error(ErrorComp::error(format!(
                "root module file `{}` is missing",
                root_path.to_string_lossy()
            )));
        }
    }
}

fn process_scope_task<'ast>(
    p: &mut Pass,
    hb: &mut hb::HirBuilder<'_, 'ast>,
    session: &Session,
    task: ScopeTreeTask<'ast>,
) {
    let parent = task.parent.map(|mod_id| hb.get_mod(mod_id).origin_id);
    let origin_id = hb.add_scope(parent, task.module);

    if let Some(mod_id) = task.parent {
        hb.get_mod_mut(mod_id).target = Some(origin_id);
    }

    for item in hb.scope_ast_items(origin_id) {
        match item {
            ast::Item::Mod(item) => match hb.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => name_already_defined_error(hb, origin_id, item.name, existing),
                None => {
                    let data = hb::ModData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        target: None,
                    };
                    let id = hb.add_mod(origin_id, data);
                    add_scope_task_from_mod_item(p, hb, session, origin_id, item, id);
                }
            },
            ast::Item::Use(..) => {
                continue;
            }
            ast::Item::Proc(item) => match hb.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => name_already_defined_error(hb, origin_id, item.name, existing),
                None => {
                    let data = hir::ProcData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        params: &[],
                        is_variadic: item.is_variadic,
                        return_ty: hir::Type::Error,
                        block: None,
                        body: hir::ProcBody { locals: &[] },
                    };
                    hb.add_proc(origin_id, item, data);
                }
            },
            ast::Item::Enum(item) => match hb.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => name_already_defined_error(hb, origin_id, item.name, existing),
                None => {
                    let data = hir::EnumData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        variants: &[],
                    };
                    hb.add_enum(origin_id, item, data);
                }
            },
            ast::Item::Union(item) => match hb.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => name_already_defined_error(hb, origin_id, item.name, existing),
                None => {
                    let data = hir::UnionData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        members: &[],
                    };
                    hb.add_union(origin_id, item, data);
                }
            },
            ast::Item::Struct(item) => match hb.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => name_already_defined_error(hb, origin_id, item.name, existing),
                None => {
                    let data = hir::StructData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        fields: &[],
                    };
                    hb.add_struct(origin_id, item, data);
                }
            },
            ast::Item::Const(item) => match hb.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => name_already_defined_error(hb, origin_id, item.name, existing),
                None => {
                    let data = hir::ConstData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        ty: hir::Type::Error,
                        value: hb::DUMMY_CONST_EXPR_ID,
                    };
                    hb.add_const(origin_id, item, data);
                }
            },
            ast::Item::Global(item) => match hb.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => name_already_defined_error(hb, origin_id, item.name, existing),
                None => {
                    let data = hir::GlobalData {
                        origin_id,
                        vis: item.vis,
                        name: item.name,
                        ty: hir::Type::Error,
                        value: hb::DUMMY_CONST_EXPR_ID,
                    };
                    hb.add_global(origin_id, item, data);
                }
            },
        }
    }
}

pub fn name_already_defined_error(
    hb: &mut hb::HirBuilder,
    origin_id: hir::ScopeID,
    name: ast::Name,
    existing: SourceRange,
) {
    hb.error(
        ErrorComp::error(format!(
            "name `{}` is defined multiple times",
            hb.name_str(name.id)
        ))
        .context(hb.src(origin_id, name.range))
        .context_info("existing definition", existing),
    );
}

fn add_scope_task_from_mod_item(
    p: &mut Pass,
    hb: &mut hb::HirBuilder,
    session: &Session,
    scope_id: hir::ScopeID,
    item: &ast::ModItem,
    id: hb::ModID,
) {
    let file_id = hb.scope_file_id(scope_id);
    let mut scope_dir = session.file(file_id).path.clone();
    scope_dir.pop();

    let mod_name = hb.name_str(item.name.id);
    let mod_filename = mod_name.to_string() + ".rock";
    let mod_path_1 = scope_dir.join(mod_filename);
    let mod_path_2 = scope_dir.join(mod_name).join("mod.rock");

    let (status, chosen_path) = match (
        p.module_map.get(&mod_path_1).cloned(),
        p.module_map.get(&mod_path_2).cloned(),
    ) {
        (Some(..), Some(..)) => {
            hb.error(
                ErrorComp::error(format!(
                    "only one possible module path can exist:\n`{}` or `{}`",
                    mod_path_1.to_string_lossy(),
                    mod_path_2.to_string_lossy()
                ))
                .context(hb.src(scope_id, item.name.range)),
            );
            return;
        }
        (None, None) => {
            hb.error(
                ErrorComp::error(format!(
                    "both possible module paths are missing:\n`{}` or `{}`",
                    mod_path_1.to_string_lossy(),
                    mod_path_2.to_string_lossy()
                ))
                .context(hb.src(scope_id, item.name.range)),
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
                ModuleStatus::Taken(hb.src(scope_id, item.name.range)),
            );
            assert!(replaced.is_some());
            p.task_queue.push(ScopeTreeTask {
                module,
                parent: Some(id),
            });
        }
        //@also provide information on which file path was taken?
        // current error only gives the sources of module items
        ModuleStatus::Taken(src) => {
            hb.error(
                ErrorComp::error(format!(
                    "module `{}` is already taken by other mod item",
                    hb.name_str(item.name.id)
                ))
                .context(hb.src(scope_id, item.name.range))
                .context_info("taken by this mod item", src),
            );
        }
    }
}
