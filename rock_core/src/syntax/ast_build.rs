use super::ast_layer as cst;
use super::syntax_tree::SyntaxTree;
use crate::ast;
use crate::error::ErrorBuffer;
use crate::intern::{InternLit, InternName, InternPool};
use crate::session::Session;
use crate::support::{Arena, TempBuffer, Timer, ID};
use crate::text::TextRange;

struct AstBuild<'ast, 'syn, 'src, 'state, 's> {
    arena: Arena<'ast>,
    tree: &'syn SyntaxTree<'syn>,
    int_id: ID<u64>,
    float_id: ID<f64>,
    char_id: ID<char>,
    string_id: ID<(ID<InternLit>, bool)>,
    source: &'src str,
    intern_name: &'src mut InternPool<'s, InternName>,
    s: &'state mut AstBuildState<'ast>,
}

struct AstBuildState<'ast> {
    errors: ErrorBuffer,
    items: TempBuffer<ast::Item<'ast>>,
    attrs: TempBuffer<ast::Attr<'ast>>,
    attr_params: TempBuffer<ast::AttrParam>,
    params: TempBuffer<ast::Param<'ast>>,
    variants: TempBuffer<ast::Variant<'ast>>,
    fields: TempBuffer<ast::Field<'ast>>,
    import_symbols: TempBuffer<ast::ImportSymbol>,
    types: TempBuffer<ast::Type<'ast>>,
    stmts: TempBuffer<ast::Stmt<'ast>>,
    exprs: TempBuffer<&'ast ast::Expr<'ast>>,
    branches: TempBuffer<ast::Branch<'ast>>,
    match_arms: TempBuffer<ast::MatchArm<'ast>>,
    patterns: TempBuffer<ast::Pat<'ast>>,
    field_inits: TempBuffer<ast::FieldInit<'ast>>,
    names: TempBuffer<ast::Name>,
    binds: TempBuffer<ast::Binding>,
}

impl<'ast, 'syn, 'src, 'state, 's> AstBuild<'ast, 'syn, 'src, 'state, 's> {
    fn new(
        tree: &'syn SyntaxTree<'syn>,
        source: &'src str,
        intern_name: &'src mut InternPool<'s, InternName>,
        state: &'state mut AstBuildState<'ast>,
    ) -> Self {
        AstBuild {
            arena: Arena::new(),
            tree,
            int_id: ID::new_raw(0),
            float_id: ID::new_raw(0),
            char_id: ID::new_raw(0),
            string_id: ID::new_raw(0),
            source,
            intern_name,
            s: state,
        }
    }

    fn finish(self, items: &'ast [ast::Item<'ast>]) -> ast::Ast<'ast> {
        ast::Ast {
            arena: self.arena,
            items,
        }
    }
}

impl<'ast> AstBuildState<'ast> {
    fn new() -> AstBuildState<'ast> {
        AstBuildState {
            errors: ErrorBuffer::default(),
            items: TempBuffer::new(128),
            attrs: TempBuffer::new(32),
            attr_params: TempBuffer::new(32),
            params: TempBuffer::new(32),
            variants: TempBuffer::new(32),
            fields: TempBuffer::new(32),
            import_symbols: TempBuffer::new(32),
            types: TempBuffer::new(32),
            stmts: TempBuffer::new(32),
            exprs: TempBuffer::new(32),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(32),
            patterns: TempBuffer::new(32),
            field_inits: TempBuffer::new(32),
            names: TempBuffer::new(32),
            binds: TempBuffer::new(32),
        }
    }
}

//@wasteful since asts are not needed
// if any of the syntax trees failed to parse
// current `in-bulk` apis are bad
pub fn parse<'ast>(session: &mut Session) -> Result<(), ErrorBuffer> {
    let t_total = Timer::new();

    //@state could be re-used via storing it in Session,
    // try later if no lifetime conflits will occur
    let mut state = AstBuildState::new();

    for module_id in session.pkg_storage.module_ids() {
        let module = session.pkg_storage.module(module_id);
        //@with trivia depends on a task, fmt / ls require it, basic compile doesnt need it.
        let tree_result =
            super::parse_tree_complete(&module.source, &mut session.intern_lit, module_id, true);

        match tree_result {
            Ok(tree) => {
                let mut ctx =
                    AstBuild::new(&tree, &module.source, &mut session.intern_name, &mut state);
                let items = source_file(&mut ctx, tree.source_file());
                let ast = ctx.finish(items);
                //@will not lineup with ModuleID's if some Asts failed to be created
                session.module_asts.push(ast);
                session.module_trees.push(Some(tree));
            }
            Err(errors) => {
                state.errors.join_e(errors);
                session.module_trees.push(None);
            }
        }
    }

    t_total.stop("ast parse (tree) total");
    state.errors.result(())
}

fn source_file<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    source_file: cst::SourceFile,
) -> &'ast [ast::Item<'ast>] {
    let offset = ctx.s.items.start();
    for item_cst in source_file.items(ctx.tree) {
        item(ctx, item_cst);
    }
    ctx.s.items.take(offset, &mut ctx.arena)
}

fn item(ctx: &mut AstBuild, item: cst::Item) {
    let item = match item {
        cst::Item::Proc(item) => ast::Item::Proc(proc_item(ctx, item)),
        cst::Item::Enum(item) => ast::Item::Enum(enum_item(ctx, item)),
        cst::Item::Struct(item) => ast::Item::Struct(struct_item(ctx, item)),
        cst::Item::Const(item) => ast::Item::Const(const_item(ctx, item)),
        cst::Item::Global(item) => ast::Item::Global(global_item(ctx, item)),
        cst::Item::Import(item) => ast::Item::Import(import_item(ctx, item)),
    };
    ctx.s.items.add(item);
}

fn attr_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    attr_list: Option<cst::AttrList>,
) -> &'ast [ast::Attr<'ast>] {
    if let Some(attr_list) = attr_list {
        let offset = ctx.s.attrs.start();
        for attr_cst in attr_list.attrs(ctx.tree) {
            let attr = attribute(ctx, attr_cst);
            ctx.s.attrs.add(attr);
        }
        ctx.s.attrs.take(offset, &mut ctx.arena)
    } else {
        &[]
    }
}

fn attribute<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_, '_>, attr: cst::Attr) -> ast::Attr<'ast> {
    //@assuming range of ident token without any trivia
    let name = name(ctx, attr.name(ctx.tree).unwrap());
    let params = attr_param_list(ctx, attr.param_list(ctx.tree));
    let range = attr.range(ctx.tree);

    ast::Attr {
        name,
        params,
        range,
    }
}

//@in general make sure range doesnt include trivia tokens
fn attr_param_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    param_list: Option<cst::AttrParamList>,
) -> Option<(&'ast [ast::AttrParam], TextRange)> {
    if let Some(param_list) = param_list {
        let offset = ctx.s.attr_params.start();
        for param_cst in param_list.params(ctx.tree) {
            let param = attr_param(ctx, param_cst);
            ctx.s.attr_params.add(param);
        }
        let params = ctx.s.attr_params.take(offset, &mut ctx.arena);
        Some((params, param_list.range(ctx.tree)))
    } else {
        None
    }
}

//@allowing and ignoring c_string
fn attr_param(ctx: &mut AstBuild, param: cst::AttrParam) -> ast::AttrParam {
    let name = name(ctx, param.name(ctx.tree).unwrap());
    let value = match param.value(ctx.tree) {
        Some(cst_string) => {
            //@only using id, these literals are included in codegen (wrong)
            let value = string_lit(ctx).id;
            let range = cst_string.range(ctx.tree);
            Some((value, range))
        }
        None => None,
    };
    ast::AttrParam { name, value }
}

fn vis(is_pub: bool) -> ast::Vis {
    if is_pub {
        ast::Vis::Public
    } else {
        ast::Vis::Private
    }
}

fn mutt(is_mut: bool) -> ast::Mut {
    if is_mut {
        ast::Mut::Mutable
    } else {
        ast::Mut::Immutable
    }
}

fn proc_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    item: cst::ProcItem,
) -> &'ast ast::ProcItem<'ast> {
    let attrs = attr_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());

    let offset = ctx.s.params.start();
    let param_list = item.param_list(ctx.tree).unwrap();
    for param_cst in param_list.params(ctx.tree) {
        param(ctx, param_cst);
    }
    let params = ctx.s.params.take(offset, &mut ctx.arena);

    let is_variadic = param_list.is_variadic(ctx.tree);
    let return_ty = ty(ctx, item.return_ty(ctx.tree).unwrap());
    let block = item.block(ctx.tree).map(|b| block(ctx, b));

    let proc_item = ast::ProcItem {
        attrs,
        vis,
        name,
        params,
        is_variadic,
        return_ty,
        block,
    };
    ctx.arena.alloc(proc_item)
}

fn param(ctx: &mut AstBuild, param: cst::Param) {
    let mutt = mutt(param.is_mut(ctx.tree));
    let name = name(ctx, param.name(ctx.tree).unwrap());
    let ty = ty(ctx, param.ty(ctx.tree).unwrap());

    let param = ast::Param { mutt, name, ty };
    ctx.s.params.add(param);
}

fn enum_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    item: cst::EnumItem,
) -> &'ast ast::EnumItem<'ast> {
    let attrs = attr_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());

    let offset = ctx.s.variants.start();
    let variant_list = item.variant_list(ctx.tree).unwrap();
    for variant_cst in variant_list.variants(ctx.tree) {
        variant(ctx, variant_cst);
    }
    let variants = ctx.s.variants.take(offset, &mut ctx.arena);

    let enum_item = ast::EnumItem {
        attrs,
        vis,
        name,
        variants,
    };
    ctx.arena.alloc(enum_item)
}

fn variant(ctx: &mut AstBuild, variant: cst::Variant) {
    let name = name(ctx, variant.name(ctx.tree).unwrap());

    let kind = if let Some(value) = variant.value(ctx.tree) {
        let value = ast::ConstExpr(expr(ctx, value));
        ast::VariantKind::Constant(value)
    } else if let Some(field_list) = variant.field_list(ctx.tree) {
        let offset = ctx.s.types.start();
        for ty_cst in field_list.fields(ctx.tree) {
            let ty = ty(ctx, ty_cst);
            ctx.s.types.add(ty);
        }
        let types = ctx.s.types.take(offset, &mut ctx.arena);
        ast::VariantKind::HasValues(types)
    } else {
        ast::VariantKind::Default
    };

    let variant = ast::Variant { name, kind };
    ctx.s.variants.add(variant);
}

fn struct_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    item: cst::StructItem,
) -> &'ast ast::StructItem<'ast> {
    let attrs = attr_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());

    let offset = ctx.s.fields.start();
    let field_list = item.field_list(ctx.tree).unwrap();
    for field_cst in field_list.fields(ctx.tree) {
        field(ctx, field_cst);
    }
    let fields = ctx.s.fields.take(offset, &mut ctx.arena);

    let struct_item = ast::StructItem {
        attrs,
        vis,
        name,
        fields,
    };
    ctx.arena.alloc(struct_item)
}

fn field(ctx: &mut AstBuild, field: cst::Field) {
    let vis = vis(field.visibility(ctx.tree).is_some());
    let name = name(ctx, field.name(ctx.tree).unwrap());
    let ty = ty(ctx, field.ty(ctx.tree).unwrap());

    let field = ast::Field { vis, name, ty };
    ctx.s.fields.add(field);
}

fn const_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    item: cst::ConstItem,
) -> &'ast ast::ConstItem<'ast> {
    let attrs = attr_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let ty = ty(ctx, item.ty(ctx.tree).unwrap());
    let value = ast::ConstExpr(expr(ctx, item.value(ctx.tree).unwrap()));

    let const_item = ast::ConstItem {
        attrs,
        vis,
        name,
        ty,
        value,
    };
    ctx.arena.alloc(const_item)
}

fn global_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    item: cst::GlobalItem,
) -> &'ast ast::GlobalItem<'ast> {
    let attrs = attr_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let mutt = mutt(item.is_mut(ctx.tree));
    let ty = ty(ctx, item.ty(ctx.tree).unwrap());
    let value = ast::ConstExpr(expr(ctx, item.value(ctx.tree).unwrap()));

    let global_item = ast::GlobalItem {
        attrs,
        vis,
        name,
        mutt,
        ty,
        value,
    };
    ctx.arena.alloc(global_item)
}

fn import_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    item: cst::ImportItem,
) -> &'ast ast::ImportItem<'ast> {
    let attrs = attr_list(ctx, item.attr_list(ctx.tree));
    let package = item.package(ctx.tree).map(|n| name(ctx, n));

    let offset = ctx.s.names.start();
    let import_path = item.import_path(ctx.tree).unwrap();
    for name_cst in import_path.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.s.names.add(name);
    }
    let import_path = ctx.s.names.take(offset, &mut ctx.arena);
    let rename = import_symbol_rename(ctx, item.rename(ctx.tree));

    let symbols = if let Some(symbol_list) = item.import_symbol_list(ctx.tree) {
        let offset = ctx.s.import_symbols.start();
        for symbol_cst in symbol_list.import_symbols(ctx.tree) {
            import_symbol(ctx, symbol_cst);
        }
        ctx.s.import_symbols.take(offset, &mut ctx.arena)
    } else {
        &[]
    };

    let import_item = ast::ImportItem {
        attrs,
        package,
        import_path,
        rename,
        symbols,
    };
    ctx.arena.alloc(import_item)
}

fn import_symbol(ctx: &mut AstBuild, import_symbol: cst::ImportSymbol) {
    let name = name(ctx, import_symbol.name(ctx.tree).unwrap());
    let rename = import_symbol_rename(ctx, import_symbol.rename(ctx.tree));

    let import_symbol = ast::ImportSymbol { name, rename };
    ctx.s.import_symbols.add(import_symbol);
}

fn import_symbol_rename(
    ctx: &mut AstBuild,
    rename: Option<cst::ImportSymbolRename>,
) -> ast::SymbolRename {
    if let Some(rename) = rename {
        if let Some(alias) = rename.alias(ctx.tree) {
            ast::SymbolRename::Alias(name(ctx, alias))
        } else {
            let (_, range) = rename.discard(ctx.tree);
            ast::SymbolRename::Discard(range)
        }
    } else {
        ast::SymbolRename::None
    }
}

fn name(ctx: &mut AstBuild, name: cst::Name) -> ast::Name {
    let range = name.range(ctx.tree);
    let string = &ctx.source[range.as_usize()];
    let id = ctx.intern_name.intern(string);
    ast::Name { range, id }
}

fn path<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_, '_>, path: cst::Path) -> &'ast ast::Path<'ast> {
    let offset = ctx.s.names.start();
    for name_cst in path.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.s.names.add(name);
    }
    let names = ctx.s.names.take(offset, &mut ctx.arena);

    ctx.arena.alloc(ast::Path { names })
}

fn bind(ctx: &mut AstBuild, bind: cst::Bind) -> ast::Binding {
    if let Some(name_cst) = bind.name(ctx.tree) {
        let mutt = mutt(bind.is_mut(ctx.tree));
        let name = name(ctx, name_cst);
        ast::Binding::Named(mutt, name)
    } else {
        //@in general use `content range` without including any trivia tokens, verify all range semantics
        let range = bind.range(ctx.tree); //@should use token range instead of this?
        ast::Binding::Discard(range)
    }
}

fn bind_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    bind_list: cst::BindList,
) -> &'ast ast::BindingList<'ast> {
    let offset = ctx.s.binds.start();
    for bind_cst in bind_list.binds(ctx.tree) {
        let bind = bind(ctx, bind_cst);
        ctx.s.binds.add(bind);
    }
    let binds = ctx.s.binds.take(offset, &mut ctx.arena);

    let range = bind_list.range(ctx.tree);
    let bind_list = ast::BindingList { binds, range };
    ctx.arena.alloc(bind_list)
}

fn args_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    args_list: cst::ArgsList,
) -> &'ast ast::ArgumentList<'ast> {
    let offset = ctx.s.exprs.start();
    for expr_cst in args_list.exprs(ctx.tree) {
        let expr = expr(ctx, expr_cst);
        ctx.s.exprs.add(expr);
    }
    let exprs = ctx.s.exprs.take(offset, &mut ctx.arena);

    let range = args_list.range(ctx.tree);
    let args_list = ast::ArgumentList { exprs, range };
    ctx.arena.alloc(args_list)
}

fn ty<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_, '_>, ty_cst: cst::Type) -> ast::Type<'ast> {
    let range = ty_cst.range(ctx.tree);

    let kind = match ty_cst {
        cst::Type::Basic(ty_cst) => {
            let basic = ty_cst.basic(ctx.tree);
            ast::TypeKind::Basic(basic)
        }
        cst::Type::Custom(ty_cst) => {
            let path = path(ctx, ty_cst.path(ctx.tree).unwrap());
            ast::TypeKind::Custom(path)
        }
        cst::Type::Reference(ty_cst) => {
            let mutt = mutt(ty_cst.is_mut(ctx.tree));
            let ref_ty = ty(ctx, ty_cst.ref_ty(ctx.tree).unwrap());
            ast::TypeKind::Reference(ctx.arena.alloc(ref_ty), mutt)
        }
        cst::Type::Procedure(proc_ty) => {
            let offset = ctx.s.types.start();
            let type_list = proc_ty.type_list(ctx.tree).unwrap();
            for ty_cst in type_list.types(ctx.tree) {
                let ty = ty(ctx, ty_cst);
                ctx.s.types.add(ty);
            }
            let param_types = ctx.s.types.take(offset, &mut ctx.arena);

            let is_variadic = type_list.is_variadic(ctx.tree);
            let return_ty = proc_ty.return_ty(ctx.tree).unwrap();
            let return_ty = ty(ctx, return_ty);

            let proc_ty = ast::ProcType {
                param_types,
                is_variadic,
                return_ty,
            };
            ast::TypeKind::Procedure(ctx.arena.alloc(proc_ty))
        }
        cst::Type::ArraySlice(slice) => {
            let mutt = mutt(slice.is_mut(ctx.tree));
            let elem_ty = ty(ctx, slice.elem_ty(ctx.tree).unwrap());

            let slice = ast::ArraySlice { mutt, elem_ty };
            ast::TypeKind::ArraySlice(ctx.arena.alloc(slice))
        }
        cst::Type::ArrayStatic(array) => {
            let len = ast::ConstExpr(expr(ctx, array.len(ctx.tree).unwrap()));
            let elem_ty = ty(ctx, array.elem_ty(ctx.tree).unwrap());

            let array = ast::ArrayStatic { len, elem_ty };
            ast::TypeKind::ArrayStatic(ctx.arena.alloc(array))
        }
    };

    ast::Type { kind, range }
}

fn stmt<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_, '_>, stmt: cst::Stmt) -> ast::Stmt<'ast> {
    let range = stmt.range(ctx.tree);

    let kind = match stmt {
        cst::Stmt::Break(_) => ast::StmtKind::Break,
        cst::Stmt::Continue(_) => ast::StmtKind::Continue,
        cst::Stmt::Return(ret) => {
            let expr = ret.expr(ctx.tree).map(|e| expr(ctx, e));
            ast::StmtKind::Return(expr)
        }
        cst::Stmt::Defer(defer) => {
            let block = block(ctx, defer.block(ctx.tree).unwrap());
            let block = ctx.arena.alloc(block);
            ast::StmtKind::Defer(block)
        }
        cst::Stmt::Loop(loop_) => ast::StmtKind::Loop(stmt_loop(ctx, loop_)),
        cst::Stmt::Local(local) => ast::StmtKind::Local(stmt_local(ctx, local)),
        cst::Stmt::Assign(assign) => ast::StmtKind::Assign(stmt_assign(ctx, assign)),
        cst::Stmt::ExprSemi(semi) => {
            let expr = expr(ctx, semi.expr(ctx.tree).unwrap());
            ast::StmtKind::ExprSemi(expr)
        }
        cst::Stmt::ExprTail(tail) => {
            let expr = expr(ctx, tail.expr(ctx.tree).unwrap());
            ast::StmtKind::ExprTail(expr)
        }
    };

    ast::Stmt { kind, range }
}

fn stmt_loop<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    loop_: cst::StmtLoop,
) -> &'ast ast::Loop<'ast> {
    let kind = if let Some(while_header) = loop_.while_header(ctx.tree) {
        let cond = expr(ctx, while_header.cond(ctx.tree).unwrap());
        ast::LoopKind::While { cond }
    } else if let Some(clike_header) = loop_.clike_header(ctx.tree) {
        let local = stmt_local(ctx, clike_header.local(ctx.tree).unwrap());
        let cond = expr(ctx, clike_header.cond(ctx.tree).unwrap());
        let assign = stmt_assign(ctx, clike_header.assign(ctx.tree).unwrap());
        ast::LoopKind::ForLoop {
            local,
            cond,
            assign,
        }
    } else {
        ast::LoopKind::Loop
    };

    let block = block(ctx, loop_.block(ctx.tree).unwrap());
    let loop_ = ast::Loop { kind, block };
    ctx.arena.alloc(loop_)
}

fn stmt_local<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    local: cst::StmtLocal,
) -> &'ast ast::Local<'ast> {
    let bind = bind(ctx, local.bind(ctx.tree).unwrap());
    let ty = if let Some(ty_cst) = local.ty(ctx.tree) {
        Some(ty(ctx, ty_cst))
    } else {
        None
    };
    let init = expr(ctx, local.init(ctx.tree).unwrap());

    let local = ast::Local { bind, ty, init };
    ctx.arena.alloc(local)
}

fn stmt_assign<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    assign: cst::StmtAssign,
) -> &'ast ast::Assign<'ast> {
    let (op, op_range) = assign.assign_op_with_range(ctx.tree).unwrap();
    let mut lhs_rhs_iter: cst::AstNodeIterator<cst::Expr> = assign.lhs_rhs_iter(ctx.tree);
    let lhs = expr(ctx, lhs_rhs_iter.next().unwrap());
    let rhs = expr(ctx, lhs_rhs_iter.next().unwrap());

    let assign = ast::Assign {
        op,
        op_range,
        lhs,
        rhs,
    };
    ctx.arena.alloc(assign)
}

//@rename expr_cst back to expr when each arm has a function
fn expr<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    expr_cst: cst::Expr,
) -> &'ast ast::Expr<'ast> {
    let kind = expr_kind(ctx, expr_cst);
    let range = expr_cst.range(ctx.tree);

    let expr = ast::Expr { kind, range };
    ctx.arena.alloc(expr)
}

fn expr_kind<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    expr_cst: cst::Expr,
) -> ast::ExprKind<'ast> {
    match expr_cst {
        cst::Expr::Paren(paren) => {
            let inner = paren.expr(ctx.tree).unwrap();
            expr_kind(ctx, inner)
        }
        cst::Expr::Lit(lit_cst) => {
            let lit = lit(ctx, lit_cst);
            ast::ExprKind::Lit { lit }
        }
        cst::Expr::If(if_) => {
            let entry = if_.entry_branch(ctx.tree).unwrap();
            let entry = ast::Branch {
                cond: expr(ctx, entry.cond(ctx.tree).unwrap()),
                block: block(ctx, entry.block(ctx.tree).unwrap()),
            };

            let offset = ctx.s.branches.start();
            for branch_cst in if_.else_if_branches(ctx.tree) {
                let branch = ast::Branch {
                    cond: expr(ctx, branch_cst.cond(ctx.tree).unwrap()),
                    block: block(ctx, branch_cst.block(ctx.tree).unwrap()),
                };
                ctx.s.branches.add(branch);
            }
            let branches = ctx.s.branches.take(offset, &mut ctx.arena);
            let else_block = if_.else_block(ctx.tree).map(|b| block(ctx, b));

            let if_ = ast::If {
                entry,
                branches,
                else_block,
            };
            let if_ = ctx.arena.alloc(if_);
            ast::ExprKind::If { if_ }
        }
        cst::Expr::Block(block_cst) => {
            let block = block(ctx, block_cst);

            let block = ctx.arena.alloc(block);
            ast::ExprKind::Block { block }
        }
        cst::Expr::Match(match_) => {
            let on_expr = expr(ctx, match_.on_expr(ctx.tree).unwrap());
            let arms = match_arm_list(ctx, match_.match_arm_list(ctx.tree).unwrap());

            let match_ = ast::Match { on_expr, arms };
            let match_ = ctx.arena.alloc(match_);
            ast::ExprKind::Match { match_ }
        }
        cst::Expr::Field(field) => {
            let target = expr(ctx, field.target(ctx.tree).unwrap());
            let name = name(ctx, field.name(ctx.tree).unwrap());
            ast::ExprKind::Field { target, name }
        }
        cst::Expr::Index(index) => {
            let mut target_index_iter = index.target_index_iter(ctx.tree);
            let target = expr(ctx, target_index_iter.next().unwrap());
            let mutt = mutt(index.is_mut(ctx.tree));
            let index = expr(ctx, target_index_iter.next().unwrap());

            ast::ExprKind::Index {
                target,
                mutt,
                index,
            }
        }
        cst::Expr::Call(call) => {
            let target = expr(ctx, call.target(ctx.tree).unwrap());
            let args_list = args_list(ctx, call.args_list(ctx.tree).unwrap());
            ast::ExprKind::Call { target, args_list }
        }
        cst::Expr::Cast(cast) => {
            let target = expr(ctx, cast.target(ctx.tree).unwrap());
            let into = ty(ctx, cast.into_ty(ctx.tree).unwrap());
            let into = ctx.arena.alloc(into);
            ast::ExprKind::Cast { target, into }
        }
        cst::Expr::Sizeof(sizeof) => {
            let ty = ty(ctx, sizeof.ty(ctx.tree).unwrap());
            let ty = ctx.arena.alloc(ty);
            ast::ExprKind::Sizeof { ty }
        }
        cst::Expr::Item(item) => {
            let path = path(ctx, item.path(ctx.tree).unwrap());
            let args_list = item.args_list(ctx.tree).map(|al| args_list(ctx, al));
            ast::ExprKind::Item { path, args_list }
        }
        cst::Expr::Variant(variant) => {
            let name = name(ctx, variant.name(ctx.tree).unwrap());
            let args_list = variant.args_list(ctx.tree).map(|al| args_list(ctx, al));
            ast::ExprKind::Variant { name, args_list }
        }
        cst::Expr::StructInit(struct_init) => {
            let path = struct_init.path(ctx.tree).map(|p| path(ctx, p));

            let offset = ctx.s.field_inits.start();
            let field_init_list = struct_init.field_init_list(ctx.tree).unwrap();
            for field_init_cst in field_init_list.field_inits(ctx.tree) {
                let expr = expr(ctx, field_init_cst.expr(ctx.tree).unwrap());
                let name = if let Some(name_cst) = field_init_cst.name(ctx.tree) {
                    name(ctx, name_cst)
                } else {
                    match expr.kind {
                        ast::ExprKind::Item { path, .. } => path.names[0],
                        _ => unreachable!(),
                    }
                };
                let field_init = ast::FieldInit { name, expr };
                ctx.s.field_inits.add(field_init);
            }
            let input = ctx.s.field_inits.take(offset, &mut ctx.arena);

            let struct_init = ast::StructInit { path, input };
            let struct_init = ctx.arena.alloc(struct_init);
            ast::ExprKind::StructInit { struct_init }
        }
        cst::Expr::ArrayInit(array_init) => {
            let offset = ctx.s.exprs.start();
            for input in array_init.inputs(ctx.tree) {
                let expr = expr(ctx, input);
                ctx.s.exprs.add(expr);
            }
            let input = ctx.s.exprs.take(offset, &mut ctx.arena);

            ast::ExprKind::ArrayInit { input }
        }
        cst::Expr::ArrayRepeat(array_repeat) => {
            let mut expr_len = array_repeat.expr_len_iter(ctx.tree);
            let value = expr(ctx, expr_len.next().unwrap());
            let len = ast::ConstExpr(expr(ctx, expr_len.next().unwrap()));

            ast::ExprKind::ArrayRepeat { expr: value, len }
        }
        cst::Expr::Deref(deref) => {
            let expr = expr(ctx, deref.expr(ctx.tree).unwrap());

            ast::ExprKind::Deref { rhs: expr }
        }
        cst::Expr::Address(address) => {
            let mutt = mutt(address.is_mut(ctx.tree));
            let expr = expr(ctx, address.expr(ctx.tree).unwrap());

            ast::ExprKind::Address { mutt, rhs: expr }
        }
        cst::Expr::Range(range_cst) => {
            let range = range(ctx, range_cst);

            let range = ctx.arena.alloc(range);
            ast::ExprKind::Range { range }
        }
        cst::Expr::Unary(unary) => {
            let (op, op_range) = unary.un_op_with_range(ctx.tree);
            let rhs = expr(ctx, unary.rhs(ctx.tree).unwrap());

            ast::ExprKind::Unary { op, op_range, rhs }
        }
        cst::Expr::Binary(binary) => {
            let (op, op_range) = binary.bin_op_with_range(ctx.tree);
            let mut lhs_rhs_iter = binary.lhs_rhs_iter(ctx.tree);
            let lhs = expr(ctx, lhs_rhs_iter.next().unwrap());
            let rhs = expr(ctx, lhs_rhs_iter.next().unwrap());

            let bin = ast::BinExpr { lhs, rhs };
            let bin = ctx.arena.alloc(bin);
            ast::ExprKind::Binary { op, op_range, bin }
        }
    }
}

fn match_arm_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    match_arm_list: cst::MatchArmList,
) -> &'ast [ast::MatchArm<'ast>] {
    let offset = ctx.s.match_arms.start();
    for arm in match_arm_list.match_arms(ctx.tree) {
        let arm = match_arm(ctx, arm);
        ctx.s.match_arms.add(arm)
    }
    ctx.s.match_arms.take(offset, &mut ctx.arena)
}

fn match_arm<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_, '_>,
    arm: cst::MatchArm,
) -> ast::MatchArm<'ast> {
    let pat = pat(ctx, arm.pat(ctx.tree).unwrap());
    let expr = expr(ctx, arm.expr(ctx.tree).unwrap());
    ast::MatchArm { pat, expr }
}

fn pat<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_, '_>, pat_cst: cst::Pat) -> ast::Pat<'ast> {
    let kind = match pat_cst {
        cst::Pat::Wild(_) => ast::PatKind::Wild,
        cst::Pat::Lit(pat) => {
            let lit = lit(ctx, pat.lit(ctx.tree).unwrap());
            ast::PatKind::Lit { lit }
        }
        cst::Pat::Item(pat) => {
            let path = path(ctx, pat.path(ctx.tree).unwrap());
            let bind_list = pat.bind_list(ctx.tree).map(|bl| bind_list(ctx, bl));
            ast::PatKind::Item { path, bind_list }
        }
        cst::Pat::Variant(pat) => {
            let name = name(ctx, pat.name(ctx.tree).unwrap());
            let bind_list = pat.bind_list(ctx.tree).map(|bl| bind_list(ctx, bl));
            ast::PatKind::Variant { name, bind_list }
        }
        cst::Pat::Or(pat_or) => {
            let offset = ctx.s.patterns.start();
            for pat_cst in pat_or.patterns(ctx.tree) {
                let pat = pat(ctx, pat_cst);
                ctx.s.patterns.add(pat);
            }
            let patterns = ctx.s.patterns.take(offset, &mut ctx.arena);
            ast::PatKind::Or { patterns }
        }
    };

    let range = pat_cst.range(ctx.tree);
    ast::Pat { kind, range }
}

fn lit(ctx: &mut AstBuild, lit: cst::Lit) -> ast::Lit {
    match lit {
        cst::Lit::Null(_) => ast::Lit::Null,
        cst::Lit::Bool(lit) => {
            let val = lit.value(ctx.tree);
            ast::Lit::Bool(val)
        }
        cst::Lit::Int(_) => {
            let val = ctx.tree.tokens().int(ctx.int_id);
            ctx.int_id = ctx.int_id.inc();
            ast::Lit::Int(val)
        }
        cst::Lit::Float(_) => {
            let val = ctx.tree.tokens().float(ctx.float_id);
            ctx.float_id = ctx.float_id.inc();
            ast::Lit::Float(val)
        }
        cst::Lit::Char(_) => {
            let val = ctx.tree.tokens().char(ctx.char_id);
            ctx.char_id = ctx.char_id.inc();
            ast::Lit::Char(val)
        }
        cst::Lit::String(_) => {
            let string_lit = string_lit(ctx);
            ast::Lit::String(string_lit)
        }
    }
}

//@separated due to being used for attr param value
fn string_lit(ctx: &mut AstBuild) -> ast::StringLit {
    let (id, c_string) = ctx.tree.tokens().string(ctx.string_id);
    ctx.string_id = ctx.string_id.inc();
    ast::StringLit { id, c_string }
}

fn range<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_, '_>, range: cst::Range) -> ast::Range<'ast> {
    match range {
        cst::Range::Full(_) => ast::Range::Full,
        cst::Range::ToExclusive(range) => {
            let end = expr(ctx, range.end(ctx.tree).unwrap());
            ast::Range::ToExclusive(end)
        }
        cst::Range::ToInclusive(range) => {
            let end = expr(ctx, range.end(ctx.tree).unwrap());
            ast::Range::ToInclusive(end)
        }
        cst::Range::From(range) => {
            let start = expr(ctx, range.start(ctx.tree).unwrap());
            ast::Range::From(start)
        }
        cst::Range::Exclusive(range) => {
            let mut start_end_iter = range.start_end_iter(ctx.tree);
            let start = expr(ctx, start_end_iter.next().unwrap());
            let end = expr(ctx, start_end_iter.next().unwrap());
            ast::Range::Exclusive(start, end)
        }
        cst::Range::Inclusive(range) => {
            let mut start_end_iter = range.start_end_iter(ctx.tree);
            let start = expr(ctx, start_end_iter.next().unwrap());
            let end = expr(ctx, start_end_iter.next().unwrap());
            ast::Range::Inclusive(start, end)
        }
    }
}

fn block<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_, '_>, block: cst::Block) -> ast::Block<'ast> {
    let offset = ctx.s.stmts.start();
    for stmt_cst in block.stmts(ctx.tree) {
        let stmt = stmt(ctx, stmt_cst);
        ctx.s.stmts.add(stmt);
    }
    let stmts = ctx.s.stmts.take(offset, &mut ctx.arena);

    ast::Block {
        stmts,
        range: block.range(ctx.tree),
    }
}
