use crate::ast::ast;
use crate::err::error_new::{ErrorComp, ErrorSeverity, SourceRange};
use crate::hir;
use crate::hir::hir_builder as hb;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
struct Pass<'ast> {
    errors: Vec<ErrorComp>,
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

pub fn run(hb: &mut hb::HirBuilder) -> Vec<ErrorComp> {
    let mut p = Pass::default();
    make_module_path_map(&mut p, hb);
    push_root_scope_task(&mut p);
    while let Some(task) = p.task_queue.pop() {
        process_scope_task(&mut p, hb, task);
    }
    p.errors
}

fn make_module_path_map<'ctx, 'ast, 'hir>(
    p: &mut Pass<'ast>,
    hb: &hb::HirBuilder<'ctx, 'ast, 'hir>,
) {
    for module in hb.ast_modules().cloned() {
        p.module_map.insert(
            hb.ctx.file(module.file_id).path.clone(),
            ModuleStatus::Available(module),
        );
    }
}

fn push_root_scope_task<'ctx, 'ast, 'hir>(p: &mut Pass<'ast>) {
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
            eprintln!("no root module found, add src/main.lang");
        }
    }
}

fn process_scope_task<'ctx, 'ast, 'hir>(
    p: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    task: ScopeTreeTask<'ast>,
) {
    let parent = match task.parent {
        Some(mod_id) => Some(hb.get_mod(mod_id).from_id),
        None => None,
    };

    let scope_temp = hb::Scope::new(parent, task.module);
    let scope_id = hb.add_scope(scope_temp);

    if let Some(mod_id) = task.parent {
        hb.get_mod_mut(mod_id).target = Some(scope_id);
    }

    for decl in hb.get_scope(scope_id).module_decls() {
        match decl {
            ast::Decl::Use(..) => {}
            ast::Decl::Mod(decl) => {
                if !name_already_defined_error(&mut p.errors, hb, scope_id, decl.name) {
                    add_defined_mod_decl(p, hb, scope_id, decl);
                }
            }
            ast::Decl::Proc(decl) => {
                if !name_already_defined_error(&mut p.errors, hb, scope_id, decl.name) {
                    add_defined_proc_decl(p, hb, scope_id, decl);
                }
            }
            ast::Decl::Enum(decl) => {
                if !name_already_defined_error(&mut p.errors, hb, scope_id, decl.name) {
                    add_defined_enum_decl(p, hb, scope_id, decl);
                }
            }
            ast::Decl::Union(decl) => {
                if !name_already_defined_error(&mut p.errors, hb, scope_id, decl.name) {
                    add_defined_union_decl(p, hb, scope_id, decl);
                }
            }
            ast::Decl::Struct(decl) => {
                if !name_already_defined_error(&mut p.errors, hb, scope_id, decl.name) {
                    add_defined_struct_decl(p, hb, scope_id, decl);
                }
            }
            ast::Decl::Const(decl) => {
                if !name_already_defined_error(&mut p.errors, hb, scope_id, decl.name) {
                    add_defined_const_decl(p, hb, scope_id, decl);
                }
            }
            ast::Decl::Global(decl) => {
                if !name_already_defined_error(&mut p.errors, hb, scope_id, decl.name) {
                    add_defined_global_decl(p, hb, scope_id, decl);
                }
            }
        }
    }
}

pub fn name_already_defined_error<'ctx, 'ast, 'hir>(
    errors: &mut Vec<ErrorComp>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
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
    errors.push(
        ErrorComp::new(
            format!(
                "name `{}` is defined multiple times",
                hb.ctx.intern().get_str(name.id)
            )
            .into(),
            ErrorSeverity::Error,
            scope.source(name.range),
        )
        .context(
            "existing definition".into(),
            ErrorSeverity::InfoHint,
            Some(scope.get_defined_symbol_source(hb, existing)),
        ),
    );
    true
}

fn add_defined_mod_decl<'ctx, 'ast, 'hir>(
    p: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::ModDecl,
) {
    let id = hb.add_mod(hb::ModData {
        from_id: scope_id,
        vis: decl.vis,
        name: decl.name,
        target: None,
    });

    let scope = hb.get_scope_mut(scope_id);
    scope.add_symbol(
        decl.name.id,
        hb::Symbol::Defined {
            kind: hb::SymbolKind::Mod(id),
        },
    );

    let scope = hb.get_scope(scope_id);
    let mut scope_dir = hb.ctx.file(scope.module_file_id()).path.clone();
    scope_dir.pop();

    let mod_name = hb.ctx.intern().get_str(decl.name.id);
    let mod_filename = mod_name.to_string() + ".lang";
    let mod_path_1 = scope_dir.join(mod_filename);
    let mod_path_2 = scope_dir.join(mod_name).join("mod.lang");

    let (status, chosen_path) = match (
        p.module_map.get(&mod_path_1).cloned(),
        p.module_map.get(&mod_path_2).cloned(),
    ) {
        (Some(..), Some(..)) => {
            p.errors.push(ErrorComp::new(
                format!(
                    "only one possible module path can exist:\n{:?} or {:?}",
                    mod_path_1, mod_path_2
                )
                .into(),
                ErrorSeverity::Error,
                scope.source(decl.name.range),
            ));
            return;
        }
        (None, None) => {
            p.errors.push(ErrorComp::new(
                format!(
                    "both possible module paths are missing:\n{:?} or {:?}",
                    mod_path_1, mod_path_2
                )
                .into(),
                ErrorSeverity::Error,
                scope.source(decl.name.range),
            ));
            return;
        }
        (Some(status), None) => (status, mod_path_1.clone()),
        (None, Some(status)) => (status, mod_path_2.clone()),
    };

    match status {
        ModuleStatus::Taken(src) => {
            p.errors.push(
                ErrorComp::new(
                    format!(
                        "module `{}` is already taken by other mod declaration",
                        hb.ctx.intern().get_str(decl.name.id)
                    )
                    .into(),
                    ErrorSeverity::Error,
                    scope.source(decl.name.range),
                )
                .context(
                    "taken by this module declaration".into(),
                    ErrorSeverity::InfoHint,
                    Some(src),
                ),
            );
        }
        ModuleStatus::Available(module) => {
            p.module_map.insert(
                chosen_path,
                ModuleStatus::Taken(scope.source(decl.name.range)),
            );
            p.task_queue.push(ScopeTreeTask {
                module,
                parent: Some(id),
            });
        }
    }
}

fn add_defined_proc_decl<'ctx, 'ast, 'hir>(
    p: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::ProcDecl<'ast>,
) {
    let scope = hb.get_scope(scope_id);
    let mut unique_params = Vec::<hir::ProcParam>::new();

    for param in decl.params.iter() {
        if let Some(existing) = unique_params.iter().find_map(|it| {
            if it.name.id == param.name.id {
                Some(it)
            } else {
                None
            }
        }) {
            p.errors.push(
                ErrorComp::new(
                    format!(
                        "parameter `{}` is defined multiple times",
                        hb.ctx.intern().get_str(param.name.id)
                    )
                    .into(),
                    ErrorSeverity::Error,
                    scope.source(param.name.range),
                )
                .context(
                    "existing parameter".into(),
                    ErrorSeverity::InfoHint,
                    Some(scope.source(existing.name.range)),
                ),
            );
        } else {
            unique_params.push(hir::ProcParam {
                mutt: param.mutt,
                name: param.name,
                ty: hir::Type::Error,
            });
        }
    }

    let params = hb.arena().alloc_slice(&unique_params);
    let id = hb.add_proc(hir::ProcData {
        from_id: scope_id,
        vis: decl.vis,
        name: decl.name,
        params,
        is_variadic: decl.is_variadic,
        return_ty: hir::Type::Error, //@array constants not processed
        block: None,
    });

    let scope = hb.get_scope_mut(scope_id);
    scope.add_symbol(
        decl.name.id,
        hb::Symbol::Defined {
            kind: hb::SymbolKind::Proc(id, decl),
        },
    );
}

fn add_defined_enum_decl<'ctx, 'ast, 'hir>(
    p: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::EnumDecl<'ast>,
) {
    let scope = hb.get_scope(scope_id);
    let mut unique_variants = Vec::<hir::EnumVariant>::new();

    for variant in decl.variants.iter() {
        if let Some(existing) = unique_variants.iter().find_map(|it| {
            if it.name.id == variant.name.id {
                Some(it)
            } else {
                None
            }
        }) {
            p.errors.push(
                ErrorComp::new(
                    format!(
                        "variant `{}` is defined multiple times",
                        hb.ctx.intern().get_str(variant.name.id)
                    )
                    .into(),
                    ErrorSeverity::Error,
                    scope.source(variant.name.range),
                )
                .context(
                    "existing variant".into(),
                    ErrorSeverity::InfoHint,
                    Some(scope.source(existing.name.range)),
                ),
            );
        } else {
            unique_variants.push(hir::EnumVariant {
                name: variant.name,
                value: None, //@optional constant expr value not handled
            });
        }
    }

    let variants = hb.arena().alloc_slice(&unique_variants);
    let id = hb.add_enum(hir::EnumData {
        from_id: scope_id,
        vis: decl.vis,
        name: decl.name,
        variants,
    });

    let scope = hb.get_scope_mut(scope_id);
    scope.add_symbol(
        decl.name.id,
        hb::Symbol::Defined {
            kind: hb::SymbolKind::Enum(id, decl),
        },
    );
}

fn add_defined_union_decl<'ctx, 'ast, 'hir>(
    p: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::UnionDecl<'ast>,
) {
    let scope = hb.get_scope(scope_id);
    let mut unique_members = Vec::<hir::UnionMember>::new();

    for member in decl.members.iter() {
        if let Some(existing) = unique_members.iter().find_map(|it| {
            if it.name.id == member.name.id {
                Some(it)
            } else {
                None
            }
        }) {
            p.errors.push(
                ErrorComp::new(
                    format!(
                        "member `{}` is defined multiple times",
                        hb.ctx.intern().get_str(member.name.id)
                    )
                    .into(),
                    ErrorSeverity::Error,
                    scope.source(member.name.range),
                )
                .context(
                    "existing member".into(),
                    ErrorSeverity::InfoHint,
                    Some(scope.source(existing.name.range)),
                ),
            );
        } else {
            unique_members.push(hir::UnionMember {
                name: member.name,
                ty: hir::Type::Error, // @not handled
            });
        }
    }

    let members = hb.arena().alloc_slice(&unique_members);
    let id = hb.add_union(hir::UnionData {
        from_id: scope_id,
        vis: decl.vis,
        name: decl.name,
        members,
    });

    let scope = hb.get_scope_mut(scope_id);
    scope.add_symbol(
        decl.name.id,
        hb::Symbol::Defined {
            kind: hb::SymbolKind::Union(id, decl),
        },
    );
}

fn add_defined_struct_decl<'ctx, 'ast, 'hir>(
    p: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::StructDecl<'ast>,
) {
    let scope = hb.get_scope(scope_id);
    let mut unique_fields = Vec::<hir::StructField>::new();

    for field in decl.fields.iter() {
        if let Some(existing) = unique_fields.iter().find_map(|it| {
            if it.name.id == field.name.id {
                Some(it)
            } else {
                None
            }
        }) {
            p.errors.push(
                ErrorComp::new(
                    format!(
                        "field `{}` is defined multiple times",
                        hb.ctx.intern().get_str(field.name.id)
                    )
                    .into(),
                    ErrorSeverity::Error,
                    scope.source(field.name.range),
                )
                .context(
                    "existing field".into(),
                    ErrorSeverity::InfoHint,
                    Some(scope.source(existing.name.range)),
                ),
            );
        } else {
            unique_fields.push(hir::StructField {
                vis: field.vis,
                name: field.name,
                ty: hir::Type::Error, // @not handled
            });
        }
    }

    let fields = hb.arena().alloc_slice(&unique_fields);
    let id = hb.add_struct(hir::StructData {
        from_id: scope_id,
        vis: decl.vis,
        name: decl.name,
        fields,
    });

    let scope = hb.get_scope_mut(scope_id);
    scope.add_symbol(
        decl.name.id,
        hb::Symbol::Defined {
            kind: hb::SymbolKind::Struct(id, decl),
        },
    );
}

fn add_defined_const_decl<'ctx, 'ast, 'hir>(
    _: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::ConstDecl<'ast>,
) {
    let id = hb.add_const(hir::ConstData {
        from_id: scope_id,
        vis: decl.vis,
        name: decl.name,
        ty: hir::Type::Error,       // @not handled
        value: hir::ConstExprID(0), // @not handled
    });

    let scope = hb.get_scope_mut(scope_id);
    scope.add_symbol(
        decl.name.id,
        hb::Symbol::Defined {
            kind: hb::SymbolKind::Const(id, decl),
        },
    );
}

fn add_defined_global_decl<'ctx, 'ast, 'hir>(
    _: &mut Pass<'ast>,
    hb: &mut hb::HirBuilder<'ctx, 'ast, 'hir>,
    scope_id: hb::ScopeID,
    decl: &'ast ast::GlobalDecl<'ast>,
) {
    let id = hb.add_global(hir::GlobalData {
        from_id: scope_id,
        vis: decl.vis,
        name: decl.name,
        ty: hir::Type::Error,       // @not handled
        value: hir::ConstExprID(0), // @not handled
    });

    let scope = hb.get_scope_mut(scope_id);
    scope.add_symbol(
        decl.name.id,
        hb::Symbol::Defined {
            kind: hb::SymbolKind::Global(id, decl),
        },
    );
}
