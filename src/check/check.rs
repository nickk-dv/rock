use super::*;
use crate::ast::CompCtx;
use crate::err::ansi;
use crate::err::error_new::*;
use crate::err::span_fmt;
use crate::mem::Arena;

//@not added as Error since its "global" to project
//@always printed out in the cli
fn report_no_src(message: &'static str) {
    let ansi_red = ansi::Color::as_ansi_str(ansi::Color::BoldRed);
    let ansi_clear = "\x1B[0m";
    eprintln!("{}error:{} {}", ansi_red, ansi_clear, message);
}

fn report(message: &'static str, _: &CompCtx, src: SourceLoc) {
    unsafe {
        ERRORS.push(CompError::new(src, Message::Str(message)));
    }
}

fn report_msg(msg: Message, _: &CompCtx, src: SourceLoc) {
    unsafe {
        ERRORS.push(CompError::new(src, msg));
    }
}

fn report_info(marker: &'static str, _: &CompCtx, src: SourceLoc) {
    unsafe {
        if let Some(error) = ERRORS.last_mut() {
            error.add_context(ErrorContext::MessageSource {
                ctx_src: src,
                msg: Message::Str(marker),
            });
        }
    }
}

fn report_info_msg(msg: Message, _: &CompCtx, src: SourceLoc) {
    unsafe {
        if let Some(error) = ERRORS.last_mut() {
            error.add_context(ErrorContext::MessageSource { ctx_src: src, msg });
        }
    }
}

pub fn report_check_errors_cli(ctx: &CompCtx, errors: &[CompError]) {
    for error in errors {
        let ansi_red = ansi::Color::as_ansi_str(ansi::Color::BoldRed);
        let ansi_clear = "\x1B[0m";
        eprintln!("\n{}error:{} {}", ansi_red, ansi_clear, error.msg.as_str());
        span_fmt::print_simple(ctx.file(error.src.file_id), error.src.span, None, false);

        for context in error.context.iter() {
            match context {
                ErrorContext::Message { msg } => {
                    eprintln!("{}", msg.as_str());
                }
                ErrorContext::MessageSource { ctx_src, msg } => {
                    span_fmt::print_simple(
                        ctx.file(ctx_src.file_id),
                        ctx_src.span,
                        Some(msg.as_str()),
                        true,
                    );
                }
            }
        }
    }
}

static mut ERRORS: Vec<CompError> = Vec::new();

pub fn check(ctx: &CompCtx, ast: &mut Ast) -> Result<(), Vec<CompError>> {
    unsafe { ERRORS.clear() };

    let mut context = Context::new();
    pass_0_populate_scopes(&mut context, &ast, ctx);
    pass_1_check_namesets(&context, ctx);
    pass_2_import_symbols(&mut context, ctx);
    pass_3_check_main_decl(&context, ctx);
    pass_3_typecheck(&context, ctx, &mut ast.arena);

    unsafe {
        if ERRORS.len() > 0 {
            Result::Err(ERRORS.to_vec())
        } else {
            Ok(())
        }
    }
}

fn pass_0_populate_scopes(context: &mut Context, ast: &Ast, ctx: &CompCtx) {
    use std::path::PathBuf;
    struct ScopeTreeTask {
        module: P<Module>,
        parent: Option<(ScopeID, ModID)>,
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
        if let Some((.., mod_id)) = task.parent {
            let parent_mod = context.get_mod_mut(mod_id);
            parent_mod.target_id = Some(scope_id);
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
                Decl::Mod(mod_decl) => {
                    #[rustfmt::skip]
                    add_declared!(
                        mod_decl, add_mod, Mod,
                        ModData { from_id: scope_id, decl: mod_decl, target_id: None }
                    );

                    let scope = context.get_scope(scope_id);
                    let mod_id = ModID((context.mods.len() - 1) as u32);

                    let name = ctx.intern().get_str(mod_decl.name.id);
                    let name_ext = name.to_string().clone() + ".lang";
                    let mut origin = ctx.file(scope.module.file_id).path.clone();
                    origin.pop();
                    let path1 = origin.clone().join(name_ext);
                    let path2 = origin.join(name).join("mod.lang");

                    let src = scope.src(mod_decl.name.span);
                    let target = match (
                        module_map.get(&path1).cloned(),
                        module_map.get(&path2).cloned(),
                    ) {
                        (None, None) => {
                            let msg =
                                format!("both module paths are missing:\n{:?}\n{:?}", path1, path2);
                            report_msg(Message::String(msg), ctx, src);
                            continue;
                        }
                        (Some(..), Some(..)) => {
                            let msg =
                                format!("both module paths are present\n{:?}\n{:?}", path1, path2);
                            report_msg(Message::String(msg), ctx, src);
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
                                continue;
                            }
                        },
                    };

                    tasks.push(ScopeTreeTask {
                        module: target,
                        parent: Some((scope_id, mod_id)),
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
                Decl::Use(..) => {}
                Decl::Const(const_decl) => {} //@todo
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
        import_decl: P<UseDecl>,
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
                Decl::Use(import_decl) => import_tasks.push(ImportTask {
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
                    let mod_id = match from_scope.get_mod(name.id) {
                        Ok(mod_id) => mod_id,
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
                    let mod_data = context.get_mod(mod_id);
                    from_id = match mod_data.target_id {
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

fn pass_3_check_main_decl(context: &Context, ctx: &CompCtx) {
    //@will crash if not root scope existed
    // early return or be aware of this on all passes
    // this pass should be skipped if compiling a library
    let id = match ctx.intern().try_get_str_id("main") {
        Some(id) => id,
        None => {
            report_no_src("missing main procedure");
            return;
        }
    };
    let root = context.get_scope(ScopeID(0));
    match root.get_declared_proc(id) {
        Ok(proc_id) => {
            let _ = context.get_proc(proc_id).decl;
            //@check decl to match: main :: () -> s32 { }
        }
        Err(_) => {
            report_no_src("missing main procedure");
            return;
        }
    }
}

fn nameresolve_type(ctx: &TypeCtx, ty: &mut Type) {
    match ty.kind {
        TypeKind::Basic(_) => {}
        TypeKind::Custom(path) => {
            // @ find a way to re-use path resolving
            // logic as much as possible between path usages in the Ast
            let kind = path.kind;
            for name in path.names.iter() {}
            //@resolve into one of those:
            ty.kind = TypeKind::Enum(EnumID(0));
            ty.kind = TypeKind::Union(UnionID(0));
            ty.kind = TypeKind::Struct(StructID(0));
            ty.kind = TypeKind::Poison;
            //@unchanged
            ty.kind = TypeKind::Custom(path);
        }
        TypeKind::ArraySlice(mut slice) => nameresolve_type(ctx, &mut slice.ty),
        TypeKind::ArrayStatic(mut array) => nameresolve_type(ctx, &mut array.ty), //@ size ConstExpr is not touched
        TypeKind::Enum(_) => panic!("nameresolve_type redundant"),
        TypeKind::Union(_) => panic!("nameresolve_type redundant"),
        TypeKind::Struct(_) => panic!("nameresolve_type redundant"),
        TypeKind::Poison => panic!("nameresolve_type redundant"),
    }
}

enum ItemResolved<'a> {
    Local(&'a LocalVar),
    None,
}

fn nameresolve_path<'a>(ctx: &'a TypeCtx, path: P<Path>) -> ItemResolved<'a> {
    let mut path_span = Span::new(path.span_start, path.span_start);

    let from_id = match path.kind {
        PathKind::None => ctx.scope_id,
        PathKind::Super => {
            path_span.end += 5;
            match ctx.scope.parent_id {
                Some(parent_id) => parent_id,
                None => {
                    report(
                        "cannot use `super` in root module",
                        ctx.comp_ctx,
                        ctx.scope.src(path_span),
                    );
                    return ItemResolved::None;
                }
            }
        }
        PathKind::Package => {
            path_span.end += 7;
            ScopeID(0)
        }
    };

    if path.names.is_empty() {
        report(
            "incomplete access path",
            ctx.comp_ctx,
            ctx.scope.src(path_span),
        );
        return ItemResolved::None;
    }

    for name in path.names {
        //let from_scope = ctx.context.get_scope(from_id);
        //let symbol = if from_id == ctx.scope_id {
        //    from_scope.get_symbol(name.id)
        //} else {
        //    from_scope.get_declared_symbol(name.id)
        //};
        //match symbol {
        //    Some(s) => {}
        //    None => {
        //        report(
        //            "symbol not found in scope",
        //            ctx.comp_ctx,
        //            ctx.scope.src(name.span),
        //        );
        //        return ItemResolved::None;
        //    }
        //}

        match ctx.proc_scope.find_local(name.id) {
            Some(local) => return ItemResolved::Local(local),
            None => {
                report(
                    "symbol is not a found as local variable",
                    ctx.comp_ctx,
                    ctx.scope.src(name.span),
                );
            }
        }
    }

    ItemResolved::None
}

struct TypeCtx<'a> {
    scope_id: ScopeID,
    scope: &'a Scope,
    context: &'a Context,
    comp_ctx: &'a CompCtx,
    proc_return_ty: Type,
    proc_scope: &'a mut ProcScope,
    ast_arena: &'a mut Arena,
}

fn pass_3_typecheck(context: &Context, ctx: &CompCtx, arena: &mut Arena) {
    for scope_id in context.scope_iter() {
        let mut type_ctx = TypeCtx {
            scope_id,
            scope: context.get_scope(scope_id),
            context,
            comp_ctx: ctx,
            proc_return_ty: Type::unit(),
            proc_scope: &mut ProcScope::new(),
            ast_arena: arena,
        };
        for decl in type_ctx.scope.module.decls {
            match decl {
                Decl::Proc(proc_decl) => typecheck_proc(&mut type_ctx, proc_decl),
                _ => {}
            }
        }
    }
}

fn typecheck_proc(ctx: &mut TypeCtx, mut proc_decl: P<ProcDecl>) {
    for param in proc_decl.params.iter_mut() {
        nameresolve_type(ctx, &mut param.ty);
    }
    if let Some(ref mut ty) = proc_decl.return_ty {
        nameresolve_type(ctx, ty);
        ctx.proc_return_ty = *ty;
    }
    if let Some(block) = proc_decl.block {
        if let Some(ref return_ty) = proc_decl.return_ty {
            ctx.proc_scope.push_stack_frame();
            for param in proc_decl.params {
                ctx.proc_scope.push_local(LocalVar::Param(param));
            }
            typecheck_expr(ctx, block, return_ty);
        }
    }
}

fn typecheck_stmt(ctx: &mut TypeCtx, stmt: Stmt, expect: &Type) -> Type {
    match stmt.kind {
        StmtKind::Break => Type::unit(),    //@is it correct to return unit?
        StmtKind::Continue => Type::unit(), //@is it correct to return unit?
        StmtKind::Return(ret) => {
            if let Some(expr) = ret {
                let ret_ty = ctx.proc_return_ty;
                typecheck_expr(ctx, expr, &ret_ty);
            }
            Type::poison() // @ignored @is it correct to return poison?
        }
        StmtKind::Defer(defer) => {
            typecheck_expr(ctx, defer, &Type::unit());
            Type::unit()
        }
        StmtKind::ForLoop(for_) => {
            Type::unit() //@ignored
        }
        StmtKind::VarDecl(mut var_decl) => {
            if let Some(ref mut ty) = var_decl.ty {
                nameresolve_type(ctx, ty);
            }
            if let Some(expr) = var_decl.expr {
                match var_decl.ty {
                    Some(ref ty) => {
                        typecheck_expr(ctx, expr, ty);
                    }
                    None => {
                        var_decl.ty = Some(typecheck_expr(ctx, expr, &Type::poison()));
                    }
                }
            }
            ctx.proc_scope.push_local(LocalVar::Local(var_decl));
            Type::unit() //@is it correct to return unit?
        }
        StmtKind::VarAssign(var_assign) => {
            Type::unit() //@is it correct to return unit?
        }
        StmtKind::ExprSemi(expr) => {
            //@maybe this is not correct handling of expr semi
            typecheck_expr(ctx, expr, &Type::unit());
            Type::unit() //@is it correct to return unit?
        }
        StmtKind::ExprTail(expr) => typecheck_expr(ctx, expr, expect),
    }
}

fn typecheck_expr(ctx: &mut TypeCtx, mut expr: P<Expr>, expect: &Type) -> Type {
    let ty = match expr.kind {
        ExprKind::Unit => Type::unit(),
        ExprKind::LitNull => Type::basic(BasicType::Rawptr),
        ExprKind::LitBool { .. } => Type::basic(BasicType::Bool),
        ExprKind::LitInt { ref mut ty, .. } => {
            let basic = match *ty {
                Some(basic) => basic,
                None => {
                    let expect_basic = if expect.ptr.level() == 0 {
                        match expect.kind {
                            TypeKind::Basic(expect_basic) => match expect_basic {
                                BasicType::S8
                                | BasicType::S16
                                | BasicType::S32
                                | BasicType::S64
                                | BasicType::Ssize
                                | BasicType::U8
                                | BasicType::U16
                                | BasicType::U32
                                | BasicType::U64
                                | BasicType::Usize => Some(expect_basic),
                                _ => None,
                            },
                            _ => None,
                        }
                    } else {
                        None
                    };
                    match expect_basic {
                        Some(expect_basic) => {
                            *ty = Some(expect_basic);
                            expect_basic
                        }
                        None => {
                            *ty = Some(BasicType::S32);
                            BasicType::S32
                        }
                    }
                }
            };
            Type::basic(basic)
        }
        ExprKind::LitFloat { ref mut ty, .. } => {
            let basic = match *ty {
                Some(basic) => basic,
                None => {
                    let expect_basic = if expect.ptr.level() == 0 {
                        match expect.kind {
                            TypeKind::Basic(expect_basic) => match expect_basic {
                                BasicType::F32 | BasicType::F64 => Some(expect_basic),
                                _ => None,
                            },
                            _ => None,
                        }
                    } else {
                        None
                    };
                    match expect_basic {
                        Some(expect_basic) => {
                            *ty = Some(expect_basic);
                            expect_basic
                        }
                        None => {
                            *ty = Some(BasicType::F64);
                            BasicType::F64
                        }
                    }
                }
            };
            Type::basic(basic)
        }
        ExprKind::LitChar { .. } => Type::basic(BasicType::Char),
        ExprKind::LitString { .. } => Type::new_ptr(Mut::Immutable, TypeKind::Basic(BasicType::U8)),
        ExprKind::If { if_ } => {
            let mut curr_if = if_;
            let mut closed = false;
            loop {
                match curr_if.else_ {
                    Some(Else::If { else_if }) => curr_if = else_if,
                    Some(Else::Block { .. }) => {
                        closed = true;
                        break;
                    }
                    _ => break,
                }
            }

            let mut block_expect = &Type::unit();
            if closed {
                block_expect = expect;
            };

            curr_if = if_;
            loop {
                typecheck_expr(ctx, curr_if.cond, &Type::basic(BasicType::Bool));
                typecheck_expr(ctx, curr_if.block, block_expect);
                match curr_if.else_ {
                    Some(Else::If { else_if }) => curr_if = else_if,
                    Some(Else::Block { block }) => {
                        typecheck_expr(ctx, block, block_expect);
                        break;
                    }
                    _ => break,
                }
            }

            *block_expect
        }
        ExprKind::Block { stmts } => {
            let mut block_ty = Type::unit();
            for (stmt, last) in stmts.iter_last() {
                if last {
                    block_ty = typecheck_stmt(ctx, stmt, expect);
                } else {
                    typecheck_stmt(ctx, stmt, &Type::unit());
                }
            }
            //@block as expr can trigger "typemismatch" multiple
            // times both on last expr and on block itself
            // thats not the best behavior.
            block_ty
        }
        ExprKind::Match { on_expr, arms } => {
            let on_ty = typecheck_expr(ctx, on_expr, &Type::poison()); // `poison` = no expectation
            Type::unit() //@ignored check arms
        }
        ExprKind::Field { target, name } => {
            let target_ty = typecheck_expr(ctx, target, &Type::poison()); // `poison` = no expectation
            Type::unit() //@ignored check target + field name
        }
        ExprKind::Index { target, index } => {
            let target_ty = typecheck_expr(ctx, target, &Type::poison()); // `poison` = no expectation
            typecheck_expr(ctx, index, &Type::basic(BasicType::Usize));
            Type::unit() //@return indexed type if operation is valid
        }
        ExprKind::Cast { target, mut ty } => {
            let target_ty = typecheck_expr(ctx, target, &Type::poison()); // `poison` = no expectation
            nameresolve_type(ctx, &mut ty);
            //@ignored check target + cast
            *ty
        }
        ExprKind::Sizeof { mut ty } => {
            nameresolve_type(ctx, &mut ty);
            Type::basic(BasicType::Usize)
        }
        ExprKind::Item { path } => {
            let item = nameresolve_path(ctx, path);
            match item {
                ItemResolved::Local(local) => match local {
                    LocalVar::Param(param) => param.ty,
                    LocalVar::Local(var_decl) => match var_decl.ty {
                        Some(ty) => ty,
                        None => {
                            //@need to track if this error was already emitted
                            // to not cause diagnostic on each use of invalid variable
                            // only applies to local vars type of which can be missing
                            report(
                                "variable type must be known at this point",
                                ctx.comp_ctx,
                                ctx.scope.src(var_decl.name.span),
                            );
                            Type::poison()
                        }
                    },
                },
                ItemResolved::None => {
                    //@temp assuming that all items are a local variable
                    report("variable not found", ctx.comp_ctx, ctx.scope.src(expr.span));
                    Type::poison()
                }
            }
        }
        ExprKind::ProcCall { path, input } => {
            // check inputs
            // based on if function name was resolved correctly

            let item = nameresolve_path(ctx, path);
            Type::poison() //@ignored
        }
        ExprKind::StructInit { path, input } => {
            // invalid => poison
            // if union has at least 1 field require only 1 input
            // if struct require field_count inputs
            // expect field / member => matching type

            let item = nameresolve_path(ctx, path);
            Type::poison() //@ignored
        }
        ExprKind::ArrayInit { input } => {
            if input.iter().nth(0).is_none() {
                report(
                    "array initializer must have at least 1 element",
                    ctx.comp_ctx,
                    ctx.scope.src(expr.span),
                );
                Type::poison()
            } else {
                //@false positive match when first doesnt match expected array type
                // not clear on what to base array type
                // contexts: [expect_ty] [no expect_ty]

                // expect ArrayStatic.ty
                let mut expect_ty = if expect.ptr.level() == 0 {
                    match expect.kind {
                        TypeKind::ArrayStatic(array) => array.ty,
                        _ => Type::poison(),
                    }
                } else {
                    Type::poison()
                };

                let mut first = true;
                let mut array_size = 0;
                let no_expect = matches!(expect_ty.kind, TypeKind::Poison);

                // expect expect_ty or first ty
                for expr in input {
                    array_size += 1;
                    if first {
                        first = false;
                        let ty = typecheck_expr(ctx, expr, &expect_ty);
                        if no_expect {
                            expect_ty = ty;
                        }
                    } else {
                        typecheck_expr(ctx, expr, &expect_ty);
                    }
                }

                // alloc ArrayStatic type if not poison
                match expect_ty.kind {
                    TypeKind::Poison => Type::poison(),
                    _ => {
                        //@Have to generate ConstExpr without any span
                        // rework array type handling during typecheck
                        let mut array_ty = ctx.ast_arena.alloc::<ArrayStatic>();
                        let mut size_expr = ConstExpr(ctx.ast_arena.alloc::<Expr>());
                        size_expr.0.kind = ExprKind::LitInt {
                            val: array_size,
                            ty: Some(BasicType::Usize),
                        };
                        array_ty.ty = expect_ty;
                        array_ty.size = size_expr;
                        Type {
                            ptr: PtrLevel::new(),
                            kind: TypeKind::ArrayStatic(array_ty),
                        }
                    }
                }
            }
        }
        ExprKind::ArrayRepeat { expr, size } => {
            //@ConstExpr size not properly resolved
            //@unknown size arrays not supported during typecheck

            // expect ArrayStatic.ty
            let expect_ty = if expect.ptr.level() == 0 {
                match expect.kind {
                    TypeKind::ArrayStatic(array) => array.ty,
                    _ => Type::poison(),
                }
            } else {
                Type::poison()
            };

            let ty = typecheck_expr(ctx, expr, &expect_ty);
            typecheck_expr(ctx, size.0, &Type::basic(BasicType::Usize)); //@Resolve as ConstExpr

            // alloc ArrayStatic type if not poison
            match ty.kind {
                TypeKind::Poison => Type::poison(),
                _ => {
                    let mut array_ty = ctx.ast_arena.alloc::<ArrayStatic>();
                    array_ty.ty = ty;
                    array_ty.size = size; //@storing same ConstExpr, no option to have `_` unknown size
                    Type {
                        ptr: PtrLevel::new(),
                        kind: TypeKind::ArrayStatic(array_ty),
                    }
                }
            }
        }
        ExprKind::UnaryExpr { op, rhs } => {
            let _ = typecheck_expr(ctx, rhs, &Type::poison()); // `poison` = no expectation
            match op {
                UnOp::Neg => Type::poison(),
                UnOp::BitNot => Type::poison(),
                UnOp::LogicNot => Type::poison(),
                UnOp::Addr(mutt) => Type::poison(),
                UnOp::Deref => Type::poison(),
            }
        }
        ExprKind::BinaryExpr { op, lhs, rhs } => {
            //@peer type resolve on ints doesnt exactly work
            // the lhs => rhs expectation works fine
            // how to achieve this "coloring" behaviour?
            // let x = 2 + 2 * 3usize; // all become usize in rust

            //@unfinished
            // expectation is ignored when resolving
            // can be used to pass expectation in case of arith ops (to affect literals)
            match op {
                BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Rem
                | BinOp::BitAnd
                | BinOp::BitOr
                | BinOp::BitXor
                | BinOp::BitShl
                | BinOp::BitShr => {
                    let ty_lhs = typecheck_expr(ctx, lhs, &Type::poison()); // `poison` = no expectation
                    let _ = typecheck_expr(ctx, rhs, &ty_lhs);
                    Type::poison() //@ignored
                }
                BinOp::CmpIsEq
                | BinOp::CmpNotEq
                | BinOp::CmpLt
                | BinOp::CmpLtEq
                | BinOp::CmpGt
                | BinOp::CmpGtEq => {
                    let ty_lhs = typecheck_expr(ctx, lhs, &Type::poison()); // `poison` = no expectation
                    let _ = typecheck_expr(ctx, rhs, &ty_lhs);
                    Type::basic(BasicType::Bool)
                }
                BinOp::LogicAnd | BinOp::LogicOr => {
                    let bool_ty = Type::basic(BasicType::Bool);
                    typecheck_expr(ctx, lhs, &bool_ty);
                    typecheck_expr(ctx, rhs, &bool_ty);
                    bool_ty
                }
            }
        }
    };
    if !Type::matches(&ty, expect) {
        report_msg(
            Message::String(format!(
                "type mismatch\nexpected: `{}`, found `{}`",
                stringify_type(expect),
                stringify_type(&ty)
            )),
            ctx.comp_ctx,
            ctx.scope.src(expr.span),
        );
    }
    ty
}

fn stringify_type(ty: &Type) -> String {
    let mut string = String::new();
    for _ in 0..ty.ptr.level() {
        string.push_str("* <MUT?> ");
    }
    match ty.kind {
        TypeKind::Basic(basic) => match basic {
            BasicType::Unit => string.push_str("()"),
            BasicType::Bool => string.push_str("bool"),
            BasicType::S8 => string.push_str("s8"),
            BasicType::S16 => string.push_str("s16"),
            BasicType::S32 => string.push_str("s32"),
            BasicType::S64 => string.push_str("s64"),
            BasicType::Ssize => string.push_str("ssize"),
            BasicType::U8 => string.push_str("u8"),
            BasicType::U16 => string.push_str("u16"),
            BasicType::U32 => string.push_str("u32"),
            BasicType::U64 => string.push_str("u64"),
            BasicType::Usize => string.push_str("usize"),
            BasicType::F32 => string.push_str("f32"),
            BasicType::F64 => string.push_str("f64"),
            BasicType::Char => string.push_str("char"),
            BasicType::Rawptr => string.push_str("rawptr"),
        },
        TypeKind::Custom(..) => string.push_str("<CUSTOM>"),
        TypeKind::ArraySlice(slice) => {
            match slice.mutt {
                Mut::Mutable => string.push_str("[mut]"),
                Mut::Immutable => string.push_str("[]"),
            }
            string.push_str(stringify_type(&slice.ty).as_str());
        }
        TypeKind::ArrayStatic(array) => {
            string.push_str("[<SIZE>]");
            string.push_str(stringify_type(&array.ty).as_str());
        }
        TypeKind::Enum(id) => string.push_str(format!("enum({:?})", id).as_str()),
        TypeKind::Union(id) => string.push_str(format!("union({:?})", id).as_str()),
        TypeKind::Struct(id) => string.push_str(format!("struct({:?})", id).as_str()),
        TypeKind::Poison => string.push_str("<POISON>"),
    }
    string
}

struct ProcScope {
    locals: Vec<LocalVar>,
    stack_frames: Vec<StackFrame>,
}

enum LocalVar {
    Param(ProcParam),
    Local(P<VarDecl>),
}

struct StackFrame {
    local_count: u32,
}

impl ProcScope {
    fn new() -> Self {
        Self {
            locals: Vec::new(),
            stack_frames: Vec::new(),
        }
    }

    fn push_stack_frame(&mut self) {
        self.stack_frames.push(StackFrame { local_count: 0 });
    }

    fn push_local(&mut self, local: LocalVar) {
        self.locals.push(local);
        match self.stack_frames.last_mut() {
            Some(frame) => frame.local_count += 1,
            None => panic!("push_local with 0 stack frames"),
        }
    }

    fn pop_stack_frame(&mut self) {
        match self.stack_frames.pop() {
            Some(frame) => {
                for _ in 0..frame.local_count {
                    self.locals.pop();
                }
            }
            None => panic!("pop_stack_frame with 0 stack frames"),
        }
    }

    fn find_local(&self, id: InternID) -> Option<&LocalVar> {
        for local in self.locals.iter() {
            let name_id = match local {
                LocalVar::Param(param) => param.name.id,
                LocalVar::Local(var_decl) => var_decl.name.id,
            };
            if name_id == id {
                return Some(local);
            }
        }
        None
    }
}
