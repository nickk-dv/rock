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
    pass_3_check_main_decl(&context, ctx);
    pass_3_typecheck(&context, ctx);
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
    let mut span = Span::new(path.span_start, path.span_start);

    let from_id = match path.kind {
        PathKind::None => ctx.scope_id,
        PathKind::Super => {
            span.end += 5;
            match ctx.scope.parent_id {
                Some(parent_id) => parent_id,
                None => {
                    report(
                        "cannot use `super` in root module",
                        ctx.comp_ctx,
                        ctx.scope.src(span),
                    );
                    return ItemResolved::None;
                }
            }
        }
        PathKind::Package => {
            span.end += 7;
            ScopeID(0)
        }
    };

    if path.names.is_empty() {
        report("incomplete access path", ctx.comp_ctx, ctx.scope.src(span));
        return ItemResolved::None;
    }

    for name in path.names {
        let from_scope = ctx.context.get_scope(from_id);
        let symbol = if from_id == ctx.scope_id {
            from_scope.get_symbol(name.id)
        } else {
            from_scope.get_declared_symbol(name.id)
        };
        match symbol {
            Some(s) => {}
            None => {
                report(
                    "symbol not found in scope",
                    ctx.comp_ctx,
                    ctx.scope.src(name.span),
                );
                return ItemResolved::None;
            }
        }

        //match ctx.proc_scope.find_local(name.id) {
        //    Some(local) => return ItemResolved::Local(local),
        //    None => {}
        //}
    }

    ItemResolved::None
}

struct TypeCtx<'a> {
    scope_id: ScopeID,
    scope: &'a Scope,
    context: &'a Context,
    comp_ctx: &'a CompCtx,
    proc_return_ty: Option<&'a Type>,
    proc_scope: &'a mut ProcScope,
}

fn pass_3_typecheck(context: &Context, ctx: &CompCtx) {
    for scope_id in context.scope_iter() {
        let mut type_ctx = TypeCtx {
            scope_id,
            scope: context.get_scope(scope_id),
            context,
            comp_ctx: ctx,
            proc_return_ty: None,
            proc_scope: &mut ProcScope::new(),
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
    } else {
        proc_decl.return_ty = Some(Type::unit());
    };
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

fn typecheck_stmt(ctx: &TypeCtx, stmt: Stmt, expect: &Type) -> Type {
    match stmt.kind {
        StmtKind::Break => Type::unit(),
        StmtKind::Continue => Type::unit(),
        StmtKind::Return(ret) => Type::unit(), //@typecheck against proc return type
        StmtKind::Defer(defer) => {
            typecheck_expr(ctx, defer, &Type::unit());
            Type::unit()
        }
        StmtKind::ForLoop(for_) => Type::unit(), //@ignored
        StmtKind::VarDecl(var_decl) => Type::unit(), //@ignored
        StmtKind::VarAssign(var_assign) => Type::unit(), //@ignored
        StmtKind::ExprSemi(expr) => {
            //@maybe this is not correct handling of expr semi
            typecheck_expr(ctx, expr, &Type::unit());
            Type::unit()
        }
        StmtKind::ExprTail(expr) => typecheck_expr(ctx, expr, expect),
    }
}

fn typecheck_expr(ctx: &TypeCtx, mut expr: P<Expr>, expect: &Type) -> Type {
    let ty = match expr.kind {
        ExprKind::Unit => Type::unit(),
        ExprKind::Discard => todo!(), //@ discard is only allowed in variable bindings
        ExprKind::LitNull => Type::basic(BasicType::Rawptr),
        ExprKind::LitBool { .. } => Type::basic(BasicType::Bool),
        ExprKind::LitUint { ref mut ty, .. } => {
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
                            if let Some(name) = var_decl.name {
                                report(
                                    "variable type must be known",
                                    ctx.comp_ctx,
                                    ctx.scope.src(name.span),
                                );
                            }
                            Type::poison()
                        }
                    },
                },
                ItemResolved::None => {
                    //@assuming that all items are a local variable
                    report("variable not found", ctx.comp_ctx, ctx.scope.src(expr.span));
                    Type::poison()
                }
            }
        }
        ExprKind::ProcCall { path, input } => Type::unit(), //@ignored
        ExprKind::StructInit { path, input } => Type::unit(), //@ignored
        ExprKind::ArrayInit { input } => Type::unit(),      //@ignored
        ExprKind::ArrayRepeat { expr, size } => Type::unit(), //@ignored
        ExprKind::UnaryExpr { op, rhs } => Type::unit(),    //@ignored
        ExprKind::BinaryExpr { op, lhs, rhs } => Type::unit(), //@ignored
    };
    if !Type::matches(&ty, expect) {
        //@printout is a temporary reporting strategy
        report("type mismatch", ctx.comp_ctx, ctx.scope.src(expr.span));
        eprint!("expected: ");
        eprint_type(&expect);
        eprint!("\ngot:      ");
        eprint_type(&ty);
        eprint!("\n\n");
    }
    ty
}

fn eprint_type(ty: &Type) {
    for _ in 0..ty.ptr.level() {
        eprint!("* <MUT?> ");
    }
    match ty.kind {
        TypeKind::Basic(basic) => match basic {
            BasicType::Unit => eprint!("()"),
            BasicType::Bool => eprint!("bool"),
            BasicType::S8 => eprint!("s8"),
            BasicType::S16 => eprint!("s16"),
            BasicType::S32 => eprint!("s32"),
            BasicType::S64 => eprint!("s64"),
            BasicType::Ssize => eprint!("ssize"),
            BasicType::U8 => eprint!("u8"),
            BasicType::U16 => eprint!("u16"),
            BasicType::U32 => eprint!("u32"),
            BasicType::U64 => eprint!("u64"),
            BasicType::Usize => eprint!("usize"),
            BasicType::F32 => eprint!("f32"),
            BasicType::F64 => eprint!("f64"),
            BasicType::Char => eprint!("char"),
            BasicType::Rawptr => eprint!("rawptr"),
        },
        TypeKind::Custom(..) => {
            eprint!("<CUSTOM>");
        }
        TypeKind::ArraySlice(slice) => {
            match slice.mutt {
                Mut::Mutable => eprint!("[mut]"),
                Mut::Immutable => eprint!("[]"),
            }
            eprint_type(&slice.ty);
        }
        TypeKind::ArrayStatic(array) => {
            eprint!("[<SIZE>]");
            eprint_type(&array.ty);
        }
        TypeKind::Enum(id) => eprint!("enum({:?})", id),
        TypeKind::Union(id) => eprint!("union({:?})", id),
        TypeKind::Struct(id) => eprint!("struct({:?})", id),
        TypeKind::Poison => eprint!("<POISON>"),
    }
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
            match local {
                LocalVar::Param(param) => {
                    if param.name.id == id {
                        return Some(local);
                    }
                }
                LocalVar::Local(var_decl) => {
                    if let Some(name) = var_decl.name {
                        if name.id == id {
                            return Some(local);
                        }
                    }
                }
            }
        }
        None
    }
}
