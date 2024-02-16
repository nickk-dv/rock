use super::*;
use crate::ast::parser::CompCtx;
use crate::err::ansi;
use crate::err::span_fmt;

pub fn check(ctx: &CompCtx, ast: &Ast) {
    let mut context = Context::new();
    pass_0_populate_scopes(&mut context, &ast, ctx);
}

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

fn pass_0_populate_scopes(context: &mut Context, ast: &Ast, ctx: &CompCtx) {
    use std::path::PathBuf;

    struct ScopeTreeTask {
        module: P<Module>,
        parent: Option<(ScopeID, ModuleID)>,
    }

    let mut module_map = HashMap::<PathBuf, P<Module>>::new();
    for module in ast.modules.iter() {
        module_map.insert(ctx.file(module.file_id).path.clone(), *module);
    }

    let mut tasks = Vec::<ScopeTreeTask>::new();

    match ast.modules.get(0).cloned() {
        Some(module) => tasks.push(ScopeTreeTask {
            module,
            parent: None,
        }),
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

        for decl in task.module.decls {
            match decl {
                Decl::Module(sym_decl) => {
                    let scope = context.get_scope(scope_id);
                    if let Some(existing) = scope.get_symbol(sym_decl.name.id) {
                        let src = SourceLoc::new(sym_decl.name.span, scope.module.file_id);
                        let info_src = context.get_symbol_src(existing);
                        report("symbol redefinition", ctx, src);
                        report_info("already declared here", ctx, info_src)
                    } else {
                        let id = context.add_module(ModuleData {
                            from_id: scope_id,
                            decl: sym_decl,
                            target_id: None,
                        });
                        let scope = context.get_scope_mut(scope_id);
                        let _ = scope.add_symbol(sym_decl.name.id, Symbol::Module(id));

                        let name = ctx.intern().get_str(sym_decl.name.id);
                        let name_ext = name.to_string().clone() + ".lang";
                        let mut origin = ctx.file(scope.module.file_id).path.clone();
                        origin.pop();
                        let path1 = origin.clone().join(name_ext);
                        let path2 = origin.join(name).join("mod.lang");
                        println!("module paths: {:?} / {:?}", path1, path2);

                        //tasks.push(ScopeTreeTask {
                        //    module: scope.module,
                        //    parent: Some((scope_id, id)),
                        //});
                    }
                }
                Decl::Import(..) => {}
                Decl::Global(sym_decl) => {
                    let scope = context.get_scope(scope_id);
                    if let Some(existing) = scope.get_symbol(sym_decl.name.id) {
                        let src = SourceLoc::new(sym_decl.name.span, scope.module.file_id);
                        let info_src = context.get_symbol_src(existing);
                        report("symbol redefinition", ctx, src);
                        report_info("already declared here", ctx, info_src)
                    } else {
                        let id = context.add_global(GlobalData {
                            from_id: scope_id,
                            decl: sym_decl,
                        });
                        let scope = context.get_scope_mut(scope_id);
                        let _ = scope.add_symbol(sym_decl.name.id, Symbol::Global(id));
                    }
                }
                Decl::Proc(sym_decl) => {
                    let scope = context.get_scope(scope_id);
                    if let Some(existing) = scope.get_symbol(sym_decl.name.id) {
                        let src = SourceLoc::new(sym_decl.name.span, scope.module.file_id);
                        let info_src = context.get_symbol_src(existing);
                        report("symbol redefinition", ctx, src);
                        report_info("already declared here", ctx, info_src)
                    } else {
                        let id = context.add_proc(ProcData {
                            from_id: scope_id,
                            decl: sym_decl,
                        });
                        let scope = context.get_scope_mut(scope_id);
                        let _ = scope.add_symbol(sym_decl.name.id, Symbol::Proc(id));
                    }
                }
                Decl::Enum(sym_decl) => {
                    let scope = context.get_scope(scope_id);
                    if let Some(existing) = scope.get_symbol(sym_decl.name.id) {
                        let src = SourceLoc::new(sym_decl.name.span, scope.module.file_id);
                        let info_src = context.get_symbol_src(existing);
                        report("symbol redefinition", ctx, src);
                        report_info("already declared here", ctx, info_src)
                    } else {
                        let id = context.add_enum(EnumData {
                            from_id: scope_id,
                            decl: sym_decl,
                        });
                        let scope = context.get_scope_mut(scope_id);
                        let _ = scope.add_symbol(sym_decl.name.id, Symbol::Enum(id));
                    }
                }
                Decl::Union(sym_decl) => {
                    let scope = context.get_scope(scope_id);
                    if let Some(existing) = scope.get_symbol(sym_decl.name.id) {
                        let src = SourceLoc::new(sym_decl.name.span, scope.module.file_id);
                        let info_src = context.get_symbol_src(existing);
                        report("symbol redefinition", ctx, src);
                        report_info("already declared here", ctx, info_src)
                    } else {
                        let id = context.add_union(UnionData {
                            from_id: scope_id,
                            decl: sym_decl,
                            size: 0,
                            align: 0,
                        });
                        let scope = context.get_scope_mut(scope_id);
                        let _ = scope.add_symbol(sym_decl.name.id, Symbol::Union(id));
                    }
                }
                Decl::Struct(sym_decl) => {
                    let scope = context.get_scope(scope_id);
                    if let Some(existing) = scope.get_symbol(sym_decl.name.id) {
                        let src = SourceLoc::new(sym_decl.name.span, scope.module.file_id);
                        let info_src = context.get_symbol_src(existing);
                        report("symbol redefinition", ctx, src);
                        report_info("already declared here", ctx, info_src)
                    } else {
                        let id = context.add_struct(StructData {
                            from_id: scope_id,
                            decl: sym_decl,
                            size: 0,
                            align: 0,
                        });
                        let scope = context.get_scope_mut(scope_id);
                        let _ = scope.add_symbol(sym_decl.name.id, Symbol::Struct(id));
                    }
                }
            }
        }
    }
}
