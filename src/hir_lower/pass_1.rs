use crate::ast::ast;
use crate::ast::CompCtx;
use crate::err::error_new::ErrorComp;
use crate::err::error_new::ErrorSeverity;
use crate::err::error_new::SourceRange;
use crate::hir;
use crate::hir::hir_builder as hb;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn run_scope_tree_gen<'a, 'ast, 'hir: 'ast>(
    ctx: &'a CompCtx,
    hir_temp: &'a mut hb::HirBuilder<'ast, 'hir>,
) -> Vec<ErrorComp> {
    let mut pass = PassContext::new(ctx, hir_temp);
    pass.run();
    pass.errors
}

struct PassContext<'a, 'ast, 'hir: 'ast> {
    ctx: &'a CompCtx,
    hb: &'a mut hb::HirBuilder<'ast, 'hir>,
    errors: Vec<ErrorComp>,
    task_queue: Vec<ScopeTreeTask<'ast>>,
    module_map: HashMap<PathBuf, ModuleStatus<'ast>>,
}

#[derive(Copy, Clone)]
struct ScopeTreeTask<'ast> {
    module: ast::Module<'ast>,
    parent: Option<hb::ModID>,
}

#[derive(Copy, Clone)]
enum ModuleStatus<'ast> {
    Taken(SourceRange),
    Available(ast::Module<'ast>),
}

impl<'a, 'ast, 'hir: 'ast> PassContext<'a, 'ast, 'hir> {
    fn new(ctx: &'a CompCtx, hb: &'a mut hb::HirBuilder<'ast, 'hir>) -> Self {
        Self {
            ctx,
            hb,
            errors: Vec::new(),
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
        for module in self.hb.ast_modules().cloned() {
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
                ModuleStatus::Taken(..) => return,
            },
            None => {
                //@ reporting without main context is not supported yet @03/05/24
                eprintln!("no root module found, add src/main.lang");
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
            Some(mod_id) => Some(self.hb.get_mod(mod_id).from_id),
            None => None,
        };

        let scope_temp = hb::Scope::new(parent, task.module);
        let scope_id = self.hb.add_scope(scope_temp);

        if let Some(mod_id) = task.parent {
            self.hb.get_mod_mut(mod_id).target = Some(scope_id);
        }

        // @use macros when this is in a finished state
        for decl in self.hb.get_scope(scope_id).module_decls() {
            match decl {
                ast::Decl::Use(_) => {}
                ast::Decl::Mod(decl) => {
                    let mod_id = match self.hb.get_scope(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            let scope_temp = self.hb.get_scope(scope_id);
                            self.errors.push(
                                ErrorComp::new(
                                    format!(
                                        "name `{}` is defined multiple times",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::Error,
                                    scope_temp.source(decl.name.range),
                                )
                                .context(
                                    format!(
                                        "name `{}` already used here",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::InfoHint,
                                    Some(scope_temp.get_local_symbol_source(&self.hb, existing)),
                                ),
                            );
                            continue;
                        }
                        None => {
                            let mod_id = self.hb.add_mod(hb::ModData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                target: None,
                            });
                            let scope_temp = self.hb.get_scope_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hb::Symbol::Defined {
                                    kind: hb::SymbolKind::Mod(mod_id),
                                },
                            );
                            mod_id
                        }
                    };

                    let scope_temp = self.hb.get_scope(scope_id);
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
                            self.errors.push(ErrorComp::new(
                                format!(
                                    "module path is ambiguous:\n{:?}\n{:?}",
                                    mod_path_1, mod_path_2
                                )
                                .into(),
                                ErrorSeverity::Error,
                                scope_temp.source(decl.name.range),
                            ));
                            continue;
                        }
                        (None, None) => {
                            self.errors.push(ErrorComp::new(
                                format!(
                                    "both module paths are missing:\n{:?}\n{:?}",
                                    mod_path_1, mod_path_2
                                )
                                .into(),
                                ErrorSeverity::Error,
                                scope_temp.source(decl.name.range),
                            ));
                            continue;
                        }
                        (Some(status), None) => (status, mod_path_1.clone()),
                        (None, Some(status)) => (status, mod_path_2.clone()),
                    };

                    match status {
                        ModuleStatus::Taken(src) => {
                            self.errors.push(ErrorComp::new(
                                "module is already taken by other mod declaration".into(),
                                ErrorSeverity::Error,
                                src,
                            ));
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
                    match self.hb.get_scope(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            let scope_temp = self.hb.get_scope(scope_id);
                            self.errors.push(
                                ErrorComp::new(
                                    format!(
                                        "name `{}` is defined multiple times",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::Error,
                                    scope_temp.source(decl.name.range),
                                )
                                .context(
                                    format!(
                                        "name `{}` already used here",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::InfoHint,
                                    Some(scope_temp.get_local_symbol_source(&self.hb, existing)),
                                ),
                            );
                        }
                        None => {
                            let params = self.hb.arena().alloc_slice(&[]); //@not processed
                            let id = self.hb.add_proc(hir::ProcData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                params,
                                is_variadic: decl.is_variadic,
                                return_ty: hir::Type::Error, //@not processed
                                block: None,
                            });
                            let scope_temp = self.hb.get_scope_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hb::Symbol::Defined {
                                    kind: hb::SymbolKind::Proc(id, decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Enum(decl) => {
                    match self.hb.get_scope(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            let scope_temp = self.hb.get_scope(scope_id);
                            self.errors.push(
                                ErrorComp::new(
                                    format!(
                                        "name `{}` is defined multiple times",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::Error,
                                    scope_temp.source(decl.name.range),
                                )
                                .context(
                                    format!(
                                        "name `{}` already used here",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::InfoHint,
                                    Some(scope_temp.get_local_symbol_source(&self.hb, existing)),
                                ),
                            );
                        }
                        None => {
                            let variants = self.hb.arena().alloc_slice(&[]); //@not processed
                            let id = self.hb.add_enum(hir::EnumData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                variants,
                            });
                            let scope_temp = self.hb.get_scope_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hb::Symbol::Defined {
                                    kind: hb::SymbolKind::Enum(id, decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Union(decl) => {
                    match self.hb.get_scope(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            let scope_temp = self.hb.get_scope(scope_id);
                            self.errors.push(
                                ErrorComp::new(
                                    format!(
                                        "name `{}` is defined multiple times",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::Error,
                                    scope_temp.source(decl.name.range),
                                )
                                .context(
                                    format!(
                                        "name `{}` already used here",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::InfoHint,
                                    Some(scope_temp.get_local_symbol_source(&self.hb, existing)),
                                ),
                            );
                        }
                        None => {
                            let members = self.hb.arena().alloc_slice(&[]);
                            let id = self.hb.add_union(hir::UnionData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                members,
                            });
                            let scope_temp = self.hb.get_scope_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hb::Symbol::Defined {
                                    kind: hb::SymbolKind::Union(id, decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Struct(decl) => {
                    match self.hb.get_scope(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            let scope_temp = self.hb.get_scope(scope_id);
                            self.errors.push(
                                ErrorComp::new(
                                    format!(
                                        "name `{}` is defined multiple times",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::Error,
                                    scope_temp.source(decl.name.range),
                                )
                                .context(
                                    format!(
                                        "name `{}` already used here",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::InfoHint,
                                    Some(scope_temp.get_local_symbol_source(&self.hb, existing)),
                                ),
                            );
                        }
                        None => {
                            let fields = self.hb.arena().alloc_slice(&[]); //@not processed
                            let id = self.hb.add_struct(hir::StructData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                fields,
                            });
                            let scope_temp = self.hb.get_scope_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hb::Symbol::Defined {
                                    kind: hb::SymbolKind::Struct(id, decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Const(decl) => {
                    match self.hb.get_scope(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            let scope_temp = self.hb.get_scope(scope_id);
                            self.errors.push(
                                ErrorComp::new(
                                    format!(
                                        "name `{}` is defined multiple times",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::Error,
                                    scope_temp.source(decl.name.range),
                                )
                                .context(
                                    format!(
                                        "name `{}` already used here",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::InfoHint,
                                    Some(scope_temp.get_local_symbol_source(&self.hb, existing)),
                                ),
                            );
                        }
                        None => {
                            let id = self.hb.add_const(hir::ConstData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                ty: None,                   //@not processed
                                value: hir::ConstExprID(0), //@not processed
                            });
                            let scope_temp = self.hb.get_scope_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hb::Symbol::Defined {
                                    kind: hb::SymbolKind::Const(id, decl),
                                },
                            );
                        }
                    }
                }
                ast::Decl::Global(decl) => {
                    match self.hb.get_scope(scope_id).get_symbol(decl.name.id) {
                        Some(existing) => {
                            let scope_temp = self.hb.get_scope(scope_id);
                            self.errors.push(
                                ErrorComp::new(
                                    format!(
                                        "name `{}` is defined multiple times",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::Error,
                                    scope_temp.source(decl.name.range),
                                )
                                .context(
                                    format!(
                                        "name `{}` already used here",
                                        self.ctx.intern().get_str(decl.name.id)
                                    )
                                    .into(),
                                    ErrorSeverity::InfoHint,
                                    Some(scope_temp.get_local_symbol_source(&self.hb, existing)),
                                ),
                            );
                        }
                        None => {
                            let id = self.hb.add_global(hir::GlobalData {
                                from_id: scope_id,
                                vis: decl.vis,
                                name: decl.name,
                                ty: None,                   //@not processed
                                value: hir::ConstExprID(0), //@not processed
                            });
                            let scope_temp = self.hb.get_scope_mut(scope_id);
                            let useless = scope_temp.add_symbol(
                                decl.name.id,
                                hb::Symbol::Defined {
                                    kind: hb::SymbolKind::Global(id, decl),
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}
