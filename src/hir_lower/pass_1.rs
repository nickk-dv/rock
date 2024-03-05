use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::SourceRange;
use crate::hir::hir_temp;

use std::collections::HashMap;
use std::path::PathBuf;

pub fn run_scope_tree_gen<'a, 'ast>(ctx: &'a CompCtx, hir_temp: &'a mut hir_temp::HirTemp<'ast>) {
    PassContext::new(ctx, hir_temp).run();
}

struct PassContext<'a, 'ast> {
    ctx: &'a CompCtx,
    hir_temp: &'a mut hir_temp::HirTemp<'ast>,
    task_queue: Vec<ScopeTreeTask<'ast>>,
    module_map: HashMap<PathBuf, ModuleStatus<'ast>>,
}

#[derive(Copy, Clone)]
struct ScopeTreeTask<'ast> {
    module: ast::Module<'ast>,
    parent: Option<hir_temp::ModID>,
}

#[derive(Copy, Clone)]
enum ModuleStatus<'ast> {
    Taken(SourceRange),
    Available(ast::Module<'ast>),
}

impl<'a, 'ast> PassContext<'a, 'ast> {
    fn new(ctx: &'a CompCtx, hir_temp: &'a mut hir_temp::HirTemp<'ast>) -> Self {
        Self {
            ctx,
            hir_temp,
            task_queue: Vec::new(),
            module_map: HashMap::new(),
        }
    }

    fn run(&mut self) {
        self.add_modules();
        self.add_root_task();
        self.process_tasks();
    }

    fn add_modules(&mut self) {
        for module in self.hir_temp.ast_modules().cloned() {
            self.module_map.insert(
                self.ctx.file(module.file_id).path.clone(),
                ModuleStatus::Available(module),
            );
        }
    }

    fn add_root_task(&mut self) {
        let root_path: PathBuf = ["test", "main.lang"].iter().collect();
        match self.module_map.remove(&root_path) {
            Some(status) => match status {
                ModuleStatus::Available(module) => {
                    self.task_queue.push(ScopeTreeTask {
                        module,
                        parent: None,
                    });
                }
                ModuleStatus::Taken(src) => {
                    eprintln!("module was taken by:");
                }
            },
            None => {
                eprintln!("no root module found, add src/main.lang"); //@report
            }
        }
    }

    fn process_tasks(&mut self) {
        while let Some(task) = self.task_queue.pop() {
            self.process_task(task);
        }
    }

    fn process_task(&mut self, task: ScopeTreeTask<'ast>) {
        let parent = match task.parent {
            Some(mod_id) => Some(self.hir_temp.get_mod(mod_id).from_id),
            None => None,
        };

        let scope_temp = hir_temp::ScopeTemp::new(parent, task.module);
        let scope_id = self.hir_temp.add_scope_temp(scope_temp);

        if let Some(mod_id) = task.parent {
            self.hir_temp.get_mod_mut(mod_id).target = Some(scope_id);
        }

        // @use macros when this is in a finished state
        for decl in self.hir_temp.get_scope_temp(scope_id).module_decls() {
            match decl {
                ast::Decl::Use(_) => {}
                ast::Decl::Mod(decl) => {
                    let mod_id = match self
                        .hir_temp
                        .get_scope_temp(scope_id)
                        .get_symbol(decl.name.id)
                    {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `mod`");
                            continue;
                        }
                        None => {
                            let mod_id = self.hir_temp.add_mod(hir_temp::ModData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                target: None,
                            });
                            let scope_temp = self.hir_temp.get_scope_temp_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hir_temp::SymbolTemp::Defined {
                                    kind: hir_temp::SymbolTempKind::Mod(mod_id),
                                },
                            );
                            mod_id
                        }
                    };

                    let scope_temp = self.hir_temp.get_scope_temp(scope_id);
                    let mut scope_dir = self.ctx.file(scope_temp.module_file_id()).path.clone();
                    scope_dir.pop();

                    let mod_name = self.ctx.intern().get_str(decl.name.id);
                    let mod_filename = mod_name.to_string() + ".lang";
                    let mod_path_1 = scope_dir.join(mod_filename);
                    let mod_path_2 = scope_dir.join(mod_name).join("mod.lang");

                    let (status, chosen_path) = match (
                        self.module_map.get(&mod_path_1).cloned(),
                        self.module_map.get(&mod_path_2).cloned(),
                    ) {
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
                        (Some(status), None) => (status, mod_path_1.clone()),
                        (None, Some(status)) => (status, mod_path_2.clone()),
                    };

                    match status {
                        ModuleStatus::Taken(src) => {
                            eprintln!("module was taken by:");
                        }
                        ModuleStatus::Available(module) => {
                            let src =
                                SourceRange::new(decl.name.range, scope_temp.module_file_id());
                            self.module_map
                                .insert(chosen_path, ModuleStatus::Taken(src));
                            self.task_queue.push(ScopeTreeTask {
                                module,
                                parent: Some(mod_id),
                            });
                        }
                    }
                }
                ast::Decl::Proc(decl) => {
                    match self
                        .hir_temp
                        .get_scope_temp(scope_id)
                        .get_symbol(decl.name.id)
                    {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `proc`");
                        }
                        None => {
                            let scope_temp = self.hir_temp.get_scope_temp_mut(scope_id);
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
                    match self
                        .hir_temp
                        .get_scope_temp(scope_id)
                        .get_symbol(decl.name.id)
                    {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `enum`");
                        }
                        None => {
                            let scope_temp = self.hir_temp.get_scope_temp_mut(scope_id);
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
                    match self
                        .hir_temp
                        .get_scope_temp(scope_id)
                        .get_symbol(decl.name.id)
                    {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `union`");
                        }
                        None => {
                            let scope_temp = self.hir_temp.get_scope_temp_mut(scope_id);
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
                    match self
                        .hir_temp
                        .get_scope_temp(scope_id)
                        .get_symbol(decl.name.id)
                    {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `struct`");
                        }
                        None => {
                            let scope_temp = self.hir_temp.get_scope_temp_mut(scope_id);
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
                    match self
                        .hir_temp
                        .get_scope_temp(scope_id)
                        .get_symbol(decl.name.id)
                    {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `const`");
                        }
                        None => {
                            let scope_temp = self.hir_temp.get_scope_temp_mut(scope_id);
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
                    match self
                        .hir_temp
                        .get_scope_temp(scope_id)
                        .get_symbol(decl.name.id)
                    {
                        Some(existing) => {
                            // ?
                            eprintln!("name already in scope when adding `global`");
                        }
                        None => {
                            let scope_temp = self.hir_temp.get_scope_temp_mut(scope_id);
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
