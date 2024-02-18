use super::*;
use crate::ast::parser::CompCtx;
use crate::err::ansi;
use crate::err::span_fmt;

fn report_no_src(message: &'static str) {
    let ansi_red = ansi::Color::as_ansi_str(ansi::Color::BoldRed);
    let ansi_clear = "\x1B[0m";
    eprintln!("{}error:{} {}", ansi_red, ansi_clear, message);
}

fn report(message: &'static str, ctx: &CompCtx, src: SourceLoc) {
    let ansi_red = ansi::Color::as_ansi_str(ansi::Color::BoldRed);
    let ansi_clear = "\x1B[0m";
    eprintln!("{}error:{} {}", ansi_red, ansi_clear, message);
    span_fmt::print_simple(ctx.file(src.file_id), src.span, None, false);
}

fn report_info(marker: &'static str, ctx: &CompCtx, src: SourceLoc) {
    span_fmt::print_simple(ctx.file(src.file_id), src.span, Some(marker), true);
}

pub fn check(ctx: &CompCtx, ast: &Ast) {
    let mut context = Context::new();
    pass_0_populate_scopes(&mut context, &ast, ctx);
    pass_1_check_namesets(&context, ctx);
    pass_2_import_symbols(&mut context, ctx);
}

fn pass_0_populate_scopes(context: &mut Context, ast: &Ast, ctx: &CompCtx) {
    use std::path::PathBuf;
    struct ScopeTreeTask {
        module: P<Module>,
        parent: Option<(ScopeID, ModuleID)>,
    }

    let mut module_map = HashMap::<PathBuf, Result<P<Module>, SourceLoc>>::new();
    let mut tasks = Vec::<ScopeTreeTask>::new();

    for module in ast.modules.iter() {
        module_map.insert(ctx.file(module.file_id).path.clone(), Ok(*module));
    }

    let root_path = PathBuf::new().join("test").join("main.lang");
    match module_map.get(&root_path).cloned() {
        Some(p) => match p {
            Ok(module) => {
                tasks.push(ScopeTreeTask {
                    module,
                    parent: None,
                });
            }
            Err(..) => {}
        },
        None => {
            report_no_src("missing root `main` or `lib` module");
            return;
        }
    };

    while let Some(task) = tasks.pop() {
        let parent_id = match task.parent {
            Some((scope_id, ..)) => Some(scope_id),
            None => None,
        };
        let scope_id = context.add_scope(Scope::new(task.module, parent_id));
        if let Some((.., module_id)) = task.parent {
            let parent_module = context.get_module_mut(module_id);
            parent_module.target_id = Some(scope_id);
        }

        macro_rules! add_declared {
            ($decl_name:ident, $add_fn_name:ident, $symbol_name:ident, $decl_data:expr) => {
                let scope = context.get_scope(scope_id);
                if let Some(existing) = scope.get_symbol($decl_name.name.id) {
                    report("symbol redefinition", ctx, scope.src($decl_name.name.span));
                    report_info(
                        "already declared here",
                        ctx,
                        context.get_symbol_id_src(existing),
                    );
                    continue;
                }
                let id = context.$add_fn_name($decl_data);
                let scope = context.get_scope_mut(scope_id);
                let _ = scope.add_declared_symbol($decl_name.name.id, SymbolID::$symbol_name(id));
            };
        }

        for decl in task.module.decls {
            match decl {
                Decl::Module(module_decl) => {
                    #[rustfmt::skip]
                    add_declared!(
                        module_decl, add_module, Module,
                        ModuleData { from_id: scope_id, decl: module_decl, target_id: None }
                    );

                    let scope = context.get_scope(scope_id);
                    let module_id = ModuleID((context.modules.len() - 1) as u32);

                    let name = ctx.intern().get_str(module_decl.name.id);
                    let name_ext = name.to_string().clone() + ".lang";
                    let mut origin = ctx.file(scope.module.file_id).path.clone();
                    origin.pop();
                    let path1 = origin.clone().join(name_ext);
                    let path2 = origin.join(name).join("mod.lang");

                    let src = scope.src(module_decl.name.span);
                    let target = match (
                        module_map.get(&path1).cloned(),
                        module_map.get(&path2).cloned(),
                    ) {
                        (None, None) => {
                            report("both module paths are missing", ctx, src);
                            eprintln!("path: {:?}", path1);
                            eprintln!("path2: {:?}", path2);
                            continue;
                        }
                        (Some(..), Some(..)) => {
                            report("both module paths are prevent", ctx, src);
                            eprintln!("path: {:?}", path1);
                            eprintln!("path2: {:?}", path2);
                            continue;
                        }
                        (Some(p), None) => match p {
                            Ok(module) => {
                                if let Some(node) = module_map.get_mut(&path1) {
                                    *node = Err(src);
                                }
                                module
                            }
                            Err(taken) => {
                                report("module has been taken", ctx, src);
                                report_info("by this module declaration", ctx, taken);
                                eprintln!("path: {:?}", path1);
                                continue;
                            }
                        },
                        (None, Some(p)) => match p {
                            Ok(module) => {
                                if let Some(node) = module_map.get_mut(&path2) {
                                    *node = Err(src);
                                }
                                module
                            }
                            Err(taken) => {
                                report("module has been taken", ctx, src);
                                report_info("by this module declaration", ctx, taken);
                                eprintln!("path: {:?}", path2);
                                continue;
                            }
                        },
                    };

                    tasks.push(ScopeTreeTask {
                        module: target,
                        parent: Some((scope_id, module_id)),
                    });
                }
                Decl::Global(global_decl) => {
                    #[rustfmt::skip]
                    add_declared!(
                        global_decl, add_global, Global,
                        GlobalData { from_id: scope_id, decl: global_decl }
                    );
                }
                Decl::Proc(proc_decl) => {
                    #[rustfmt::skip]
                    add_declared!(
                        proc_decl, add_proc, Proc,
                        ProcData { from_id: scope_id, decl: proc_decl }
                    );
                }
                Decl::Enum(enum_decl) => {
                    #[rustfmt::skip]
                    add_declared!(
                        enum_decl, add_enum, Enum,
                        EnumData { from_id: scope_id, decl: enum_decl }
                    );
                }
                Decl::Union(union_decl) => {
                    #[rustfmt::skip]
                    add_declared!(
                        union_decl, add_union, Union,
                        UnionData { from_id: scope_id, decl: union_decl, size: 0, align: 0 }
                    );
                }
                Decl::Struct(struct_decl) => {
                    #[rustfmt::skip]
                    add_declared!(
                        struct_decl, add_struct, Struct,
                        StructData { from_id: scope_id, decl: struct_decl, size: 0, align: 0 }
                    );
                }
                Decl::Import(..) => {}
            }
        }
    }
}

fn pass_1_check_namesets(context: &Context, ctx: &CompCtx) {
    for scope_id in context.scope_iter() {
        let mut name_set = HashMap::<InternID, Span>::new();
        let scope = context.get_scope(scope_id);

        macro_rules! check_nameset {
            ($decl_name:ident, $list_name:ident, $message:expr) => {{
                if $decl_name.$list_name.is_empty() {
                    continue;
                }
                name_set.clear();
                for item in $decl_name.$list_name.iter() {
                    if let Some(existing) = name_set.get(&item.name.id).cloned() {
                        report($message, ctx, scope.src(item.name.span));
                        report_info("already defined here", ctx, scope.src(existing));
                    } else {
                        name_set.insert(item.name.id, item.name.span);
                    }
                }
            }};
        }

        for decl in scope.module.decls {
            match decl {
                Decl::Proc(proc_decl) => {
                    check_nameset!(proc_decl, params, "proc param redefinition")
                }
                Decl::Enum(enum_decl) => {
                    check_nameset!(enum_decl, variants, "enum variant redefinition")
                }
                Decl::Union(union_decl) => {
                    check_nameset!(union_decl, members, "union member redefinition")
                }
                Decl::Struct(struct_decl) => {
                    check_nameset!(struct_decl, fields, "struct field redefinition")
                }
                _ => {}
            }
        }
    }
}

// @locations are wrong when working with imported / declared
// its unclear on which span to use, try to improve the api
fn pass_2_import_symbols(context: &mut Context, ctx: &CompCtx) {
    struct ImportTask {
        import_decl: P<ImportDecl>,
        resolved: bool,
    }
    struct ImportSymbolTask {
        symbol_id: SymbolID,
        name: Ident,
    }

    let mut import_tasks = Vec::<ImportTask>::new();
    let mut import_symbol_tasks = Vec::<ImportSymbolTask>::new();

    for scope_id in context.scope_iter() {
        import_tasks.clear();

        for decl in context.get_scope(scope_id).module.decls {
            match decl {
                Decl::Import(import_decl) => import_tasks.push(ImportTask {
                    import_decl,
                    resolved: false,
                }),
                _ => {}
            }
        }

        let mut progress = 0;
        loop {
            let mut curr_progress = 0;

            'task: for task in import_tasks.iter_mut() {
                if task.resolved {
                    curr_progress += 1;
                    continue 'task;
                }
                // resolve path kind
                let scope = context.get_scope(scope_id);
                let mut from_id = match task.import_decl.path.kind {
                    PathKind::None => scope_id,
                    PathKind::Super => match scope.parent_id {
                        Some(parent_id) => parent_id,
                        None => {
                            let span = Span::new(
                                task.import_decl.path.span_start,
                                task.import_decl.path.span_start + 5,
                            );
                            report(
                                "cannot use `super` from the root module",
                                ctx,
                                scope.src(span),
                            );
                            task.resolved = true;
                            curr_progress += 1;
                            continue 'task;
                        }
                    },
                    PathKind::Package => ScopeID(0),
                };
                // resolve module path
                let mut span_end = task.import_decl.path.span_start;
                for name in task.import_decl.path.names {
                    let from_scope = context.get_scope(from_id);
                    let module_id = match from_scope.get_module(name.id) {
                        Ok(module_id) => module_id,
                        Err(symbol) => {
                            if let Some(symbol) = symbol {
                                report("module not found", ctx, scope.src(name.span));
                                report_info(
                                    "name refers to this declaration",
                                    ctx,
                                    context.get_symbol_id_src(symbol),
                                );
                                task.resolved = true;
                                curr_progress += 1;
                            }
                            continue 'task;
                        }
                    };
                    let module_data = context.get_module(module_id);
                    from_id = match module_data.target_id {
                        Some(target_id) => target_id,
                        None => {
                            report(
                                "module file is missing as reported earlier",
                                ctx,
                                scope.src(name.span),
                            );
                            task.resolved = true;
                            curr_progress += 1;
                            continue 'task;
                        }
                    };
                    span_end = name.span.end;
                }

                // collect visible symbol imports
                task.resolved = true;
                curr_progress += 1;
                if from_id.0 == scope_id.0 {
                    report(
                        "importing from self is redundant",
                        ctx,
                        scope.src(Span::new(task.import_decl.path.span_start, span_end)),
                    );
                    continue 'task;
                }
                let from_scope = context.get_scope(from_id);
                import_symbol_tasks.clear();

                for import_symbol in task.import_decl.symbols.iter() {
                    match from_scope.get_symbol(import_symbol.name.id) {
                        Some(symbol_id) => {
                            let vis = context.get_symbol_vis(symbol_id);
                            //@allowing use of private package lvl items
                            if vis == Vis::Private && from_id.0 != 0 {
                                report(
                                    "symbol is private",
                                    ctx,
                                    scope.src(import_symbol.name.span),
                                );
                                report_info(
                                    "private declaration",
                                    ctx,
                                    context.get_symbol_id_src(symbol_id),
                                );
                                continue;
                            }
                            let name = match import_symbol.alias {
                                Some(alias) => alias,
                                None => import_symbol.name,
                            };
                            import_symbol_tasks.push(ImportSymbolTask { symbol_id, name });
                        }
                        None => {
                            report(
                                "symbol not found in path",
                                ctx,
                                scope.src(import_symbol.name.span),
                            );
                            continue;
                        }
                    }
                }
                // import into scope
                for task in import_symbol_tasks.iter() {
                    let scope = context.get_scope_mut(scope_id);
                    match scope.add_imported_symbol(task.name.id, task.symbol_id, task.name.span) {
                        Ok(()) => {}
                        Err(existing) => {
                            report("symbol redifinition", ctx, scope.src(task.name.span));
                            report_info(
                                "already declared here",
                                ctx,
                                context.get_symbol_src(scope_id, existing),
                            );
                        }
                    }
                }
            }

            if progress >= curr_progress {
                break;
            }
            progress = curr_progress;
        }

        // report unresolved paths
        let scope = context.get_scope(scope_id);
        for task in import_tasks.iter() {
            if !task.resolved {
                for name in task.import_decl.path.names {
                    report("module not found", ctx, scope.src(name.span));
                    break;
                }
            }
        }
    }
}
