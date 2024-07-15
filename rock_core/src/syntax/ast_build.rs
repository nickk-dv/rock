use super::ast_layer as cst;
use super::syntax_tree::SyntaxTree;
use crate::arena::Arena;
use crate::ast;
use crate::error::{DiagnosticCollection, ErrorComp, ResultComp, SourceRange};
use crate::intern::InternPool;
use crate::session::{ModuleID, Session};
use crate::temp_buffer::TempBuffer;
use crate::text::TextRange;
use crate::timer::Timer;

//@rename some ast:: nodes to match ast_layer and syntax names (eg: ProcParam)
struct AstBuild<'ast, 'syn, 'src, 'state> {
    tree: &'syn SyntaxTree<'syn>,
    int_id: u32,
    char_id: u32,
    string_id: u32,
    module_id: ModuleID,
    source: &'src str,
    s: &'state mut AstBuildState<'ast>,
}

struct AstBuildState<'ast> {
    arena: Arena<'ast>,
    intern_name: InternPool<'ast>,
    intern_string: InternPool<'ast>,
    string_is_cstr: Vec<bool>,
    modules: Vec<ast::Module<'ast>>,
    errors: Vec<ErrorComp>,

    items: TempBuffer<ast::Item<'ast>>,
    attrs: TempBuffer<ast::Attribute>,
    params: TempBuffer<ast::ProcParam<'ast>>,
    variants: TempBuffer<ast::EnumVariant<'ast>>,
    fields: TempBuffer<ast::StructField<'ast>>,
    import_symbols: TempBuffer<ast::ImportSymbol>,
    names: TempBuffer<ast::Name>,
    types: TempBuffer<ast::Type<'ast>>,
    stmts: TempBuffer<ast::Stmt<'ast>>,
    exprs: TempBuffer<&'ast ast::Expr<'ast>>,
    branches: TempBuffer<ast::Branch<'ast>>,
    match_arms: TempBuffer<ast::MatchArm<'ast>>,
    field_inits: TempBuffer<ast::FieldInit<'ast>>,
}

impl<'ast, 'syn, 'src, 'state> AstBuild<'ast, 'syn, 'src, 'state> {
    fn new(
        tree: &'syn SyntaxTree<'syn>,
        source: &'src str,
        module_id: ModuleID,
        state: &'state mut AstBuildState<'ast>,
    ) -> Self {
        AstBuild {
            tree,
            int_id: 0,
            char_id: 0,
            string_id: 0,
            module_id,
            source,
            s: state,
        }
    }
}

impl<'ast> AstBuildState<'ast> {
    fn new(intern_name: InternPool<'ast>) -> Self {
        AstBuildState {
            arena: Arena::new(),
            intern_name,
            intern_string: InternPool::new(),
            string_is_cstr: Vec::with_capacity(128),
            modules: Vec::new(),
            errors: Vec::new(),

            items: TempBuffer::new(128),
            attrs: TempBuffer::new(32),
            params: TempBuffer::new(32),
            variants: TempBuffer::new(32),
            fields: TempBuffer::new(32),
            import_symbols: TempBuffer::new(32),
            names: TempBuffer::new(32),
            types: TempBuffer::new(32),
            stmts: TempBuffer::new(32),
            exprs: TempBuffer::new(32),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(32),
            field_inits: TempBuffer::new(32),
        }
    }
}

pub fn parse<'ast, 'intern: 'ast>(
    session: &Session,
    intern_name: InternPool<'intern>,
) -> ResultComp<ast::Ast<'ast, 'intern>> {
    let t_total = Timer::new();
    let mut state = AstBuildState::new(intern_name);

    for module_id in session.module_ids() {
        let module = session.module(module_id);
        let tree_result = super::parse_tree_complete(&module.source, module_id, false);

        match tree_result {
            Ok(tree) => {
                let mut ctx = AstBuild::new(&tree, &module.source, module_id, &mut state);
                let items = source_file(&mut ctx, tree.source_file());
                state.modules.push(ast::Module { items });
            }
            Err(errors) => {
                state.errors.extend(errors);
            }
        }
    }

    t_total.stop("ast parse (new) total");
    if state.errors.is_empty() {
        let ast = ast::Ast {
            arena: state.arena,
            intern_name: state.intern_name,
            intern_string: state.intern_string,
            string_is_cstr: state.string_is_cstr,
            modules: state.modules,
        };
        ResultComp::Ok((ast, vec![]))
    } else {
        ResultComp::Err(DiagnosticCollection::new().join_errors(state.errors))
    }
}

fn source_file<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    source_file: cst::SourceFile,
) -> &'ast [ast::Item<'ast>] {
    let offset = ctx.s.items.start();
    for item_cst in source_file.items(ctx.tree) {
        item(ctx, item_cst);
    }
    ctx.s.items.take(offset, &mut ctx.s.arena)
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

fn attribute_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    attr_list: Option<cst::AttributeList>,
) -> &'ast [ast::Attribute] {
    if let Some(attr_list) = attr_list {
        let offset = ctx.s.attrs.start();
        for attr_cst in attr_list.attrs(ctx.tree) {
            let attr = attribute(ctx, attr_cst);
            ctx.s.attrs.add(attr);
        }
        ctx.s.attrs.take(offset, &mut ctx.s.arena)
    } else {
        &[]
    }
}

fn attribute(ctx: &mut AstBuild, attr: cst::Attribute) -> ast::Attribute {
    //@assuming range of ident token without any trivia
    let name_cst = attr.name(ctx.tree).unwrap();
    let range = name_cst.range(ctx.tree);
    let string = &ctx.source[range.as_usize()];

    ast::Attribute {
        kind: ast::AttributeKind::from_str(string),
        range: attr.range(ctx.tree),
    }
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
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::ProcItem,
) -> &'ast ast::ProcItem<'ast> {
    let attrs = attribute_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());

    let offset = ctx.s.params.start();
    let param_list = item.param_list(ctx.tree).unwrap();
    for param_cst in param_list.params(ctx.tree) {
        param(ctx, param_cst);
    }
    let params = ctx.s.params.take(offset, &mut ctx.s.arena);

    let is_variadic = param_list.is_variadic(ctx.tree);
    let return_ty = item.return_ty(ctx.tree).map(|t| ty(ctx, t));
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
    ctx.s.arena.alloc(proc_item)
}

fn param(ctx: &mut AstBuild, param: cst::Param) {
    let mutt = mutt(param.is_mut(ctx.tree));
    let name = name(ctx, param.name(ctx.tree).unwrap());
    let ty = ty(ctx, param.ty(ctx.tree).unwrap());

    let param = ast::ProcParam { mutt, name, ty };
    ctx.s.params.add(param);
}

fn enum_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::EnumItem,
) -> &'ast ast::EnumItem<'ast> {
    let attrs = attribute_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let basic = item
        .type_basic(ctx.tree)
        .map(|tb| (tb.basic(ctx.tree), tb.range(ctx.tree)));

    let offset = ctx.s.variants.start();
    let variant_list = item.variant_list(ctx.tree).unwrap();
    for variant_cst in variant_list.variants(ctx.tree) {
        variant(ctx, variant_cst);
    }
    let variants = ctx.s.variants.take(offset, &mut ctx.s.arena);

    let enum_item = ast::EnumItem {
        attrs,
        vis,
        name,
        basic,
        variants,
    };
    ctx.s.arena.alloc(enum_item)
}

fn variant(ctx: &mut AstBuild, variant: cst::Variant) {
    //@value is optional in grammar but required in ast due to
    // const expr resolve limitation, will panic for now
    let name = name(ctx, variant.name(ctx.tree).unwrap());
    let value = ast::ConstExpr(expr(ctx, variant.value(ctx.tree).unwrap()));

    let variant = ast::EnumVariant { name, value };
    ctx.s.variants.add(variant);
}

fn struct_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::StructItem,
) -> &'ast ast::StructItem<'ast> {
    let attrs = attribute_list(ctx, item.attr_list(ctx.tree));
    let vis = vis(item.visibility(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());

    let offset = ctx.s.fields.start();
    let field_list = item.field_list(ctx.tree).unwrap();
    for field_cst in field_list.fields(ctx.tree) {
        field(ctx, field_cst);
    }
    let fields = ctx.s.fields.take(offset, &mut ctx.s.arena);

    let struct_item = ast::StructItem {
        attrs,
        vis,
        name,
        fields,
    };
    ctx.s.arena.alloc(struct_item)
}

fn field(ctx: &mut AstBuild, field: cst::Field) {
    let vis = vis(field.visibility(ctx.tree).is_some());
    let name = name(ctx, field.name(ctx.tree).unwrap());
    let ty = ty(ctx, field.ty(ctx.tree).unwrap());

    let field = ast::StructField { vis, name, ty };
    ctx.s.fields.add(field);
}

fn const_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::ConstItem,
) -> &'ast ast::ConstItem<'ast> {
    let attrs = attribute_list(ctx, item.attr_list(ctx.tree));
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
    ctx.s.arena.alloc(const_item)
}

fn global_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::GlobalItem,
) -> &'ast ast::GlobalItem<'ast> {
    let attrs = attribute_list(ctx, item.attr_list(ctx.tree));
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
    ctx.s.arena.alloc(global_item)
}

fn import_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::ImportItem,
) -> &'ast ast::ImportItem<'ast> {
    let attrs = attribute_list(ctx, item.attr_list(ctx.tree));
    let package = item.package(ctx.tree).map(|n| name(ctx, n));

    let offset = ctx.s.names.start();
    let import_path = item.import_path(ctx.tree).unwrap();
    for name_cst in import_path.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.s.names.add(name);
    }
    let import_path = ctx.s.names.take(offset, &mut ctx.s.arena);
    let rename = symbol_rename(ctx, item.rename(ctx.tree));

    let symbols = if let Some(symbol_list) = item.import_symbol_list(ctx.tree) {
        let offset = ctx.s.import_symbols.start();
        for symbol_cst in symbol_list.import_symbols(ctx.tree) {
            import_symbol(ctx, symbol_cst);
        }
        ctx.s.import_symbols.take(offset, &mut ctx.s.arena)
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
    ctx.s.arena.alloc(import_item)
}

fn import_symbol(ctx: &mut AstBuild, import_symbol: cst::ImportSymbol) {
    let name = name(ctx, import_symbol.name(ctx.tree).unwrap());
    let rename = symbol_rename(ctx, import_symbol.rename(ctx.tree));

    let import_symbol = ast::ImportSymbol { name, rename };
    ctx.s.import_symbols.add(import_symbol);
}

fn symbol_rename(ctx: &mut AstBuild, rename: Option<cst::SymbolRename>) -> ast::SymbolRename {
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
    let id = ctx.s.intern_name.intern(string);
    ast::Name { range, id }
}

fn path<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, path: cst::Path) -> &'ast ast::Path<'ast> {
    let offset = ctx.s.names.start();
    for name_cst in path.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.s.names.add(name);
    }
    let names = ctx.s.names.take(offset, &mut ctx.s.arena);

    ctx.s.arena.alloc(ast::Path { names })
}

fn ty<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, ty_cst: cst::Type) -> ast::Type<'ast> {
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
            ast::TypeKind::Reference(ctx.s.arena.alloc(ref_ty), mutt)
        }
        cst::Type::Procedure(proc_ty) => {
            let offset = ctx.s.types.start();
            let type_list = proc_ty.type_list(ctx.tree).unwrap();
            for ty_cst in type_list.types(ctx.tree) {
                let ty = ty(ctx, ty_cst);
                ctx.s.types.add(ty);
            }
            let param_types = ctx.s.types.take(offset, &mut ctx.s.arena);

            let is_variadic = type_list.is_variadic(ctx.tree);
            let return_ty = proc_ty.return_ty(ctx.tree).map(|t| ty(ctx, t));

            let proc_ty = ast::ProcType {
                param_types,
                is_variadic,
                return_ty,
            };
            ast::TypeKind::Procedure(ctx.s.arena.alloc(proc_ty))
        }
        cst::Type::ArraySlice(slice) => {
            let mutt = mutt(slice.is_mut(ctx.tree));
            let elem_ty = ty(ctx, slice.elem_ty(ctx.tree).unwrap());

            let slice = ast::ArraySlice { mutt, elem_ty };
            ast::TypeKind::ArraySlice(ctx.s.arena.alloc(slice))
        }
        cst::Type::ArrayStatic(array) => {
            let len = ast::ConstExpr(expr(ctx, array.len(ctx.tree).unwrap()));
            let elem_ty = ty(ctx, array.elem_ty(ctx.tree).unwrap());

            let array = ast::ArrayStatic { len, elem_ty };
            ast::TypeKind::ArrayStatic(ctx.s.arena.alloc(array))
        }
    };

    ast::Type { kind, range }
}

fn stmt<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, stmt: cst::Stmt) -> ast::Stmt<'ast> {
    let range = stmt.range(ctx.tree);

    let kind = match stmt {
        cst::Stmt::Break(_) => ast::StmtKind::Break,
        cst::Stmt::Continue(_) => ast::StmtKind::Continue,
        cst::Stmt::Return(ret) => {
            let expr = ret.expr(ctx.tree).map(|e| expr(ctx, e));
            ast::StmtKind::Return(expr)
        }
        cst::Stmt::Defer(defer) => ast::StmtKind::Defer(stmt_defer(ctx, defer)),
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

fn stmt_defer<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    defer: cst::StmtDefer,
) -> &'ast ast::Block<'ast> {
    if let Some(short_block) = defer.short_block(ctx.tree) {
        let stmt_cst = short_block.stmt(ctx.tree).unwrap();
        let stmt = stmt(ctx, stmt_cst);

        let offset = ctx.s.stmts.start();
        ctx.s.stmts.add(stmt);
        let stmts = ctx.s.stmts.take(offset, &mut ctx.s.arena);

        let block = ast::Block {
            stmts,
            range: stmt.range,
        };
        ctx.s.arena.alloc(block)
    } else {
        let block_cst = defer.block(ctx.tree).unwrap();
        let block = block(ctx, block_cst);
        ctx.s.arena.alloc(block)
    }
}

fn stmt_loop<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
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
    ctx.s.arena.alloc(loop_)
}

fn stmt_local<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    local: cst::StmtLocal,
) -> &'ast ast::Local<'ast> {
    let mutt = mutt(local.is_mut(ctx.tree));
    let name = name(ctx, local.name(ctx.tree).unwrap());

    let kind = if let Some(ty_cst) = local.ty(ctx.tree) {
        let ty = ty(ctx, ty_cst);
        if let Some(expr_cst) = local.expr(ctx.tree) {
            let expr = expr(ctx, expr_cst);
            ast::LocalKind::Init(Some(ty), expr)
        } else {
            ast::LocalKind::Decl(ty)
        }
    } else {
        let expr = expr(ctx, local.expr(ctx.tree).unwrap());
        ast::LocalKind::Init(None, expr)
    };

    let local = ast::Local { mutt, name, kind };
    ctx.s.arena.alloc(local)
}

fn stmt_assign<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
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
    ctx.s.arena.alloc(assign)
}

//@rename expr_cst back to expr when each arm has a function
fn expr<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, expr_cst: cst::Expr) -> &'ast ast::Expr<'ast> {
    let range = expr_cst.range(ctx.tree);

    let kind = match expr_cst {
        cst::Expr::Paren(paren) => {
            //@problem with range and inner expr
            //@get expr_kind with separate function
            todo!();
        }
        cst::Expr::LitNull(_) => ast::ExprKind::LitNull,
        cst::Expr::LitBool(lit) => {
            let val = lit.value(ctx.tree);

            ast::ExprKind::LitBool { val }
        }
        cst::Expr::LitInt(_) => {
            let val = ctx.tree.tokens().int(ctx.int_id as usize);
            ctx.int_id += 1;

            ast::ExprKind::LitInt { val }
        }
        cst::Expr::LitFloat(lit) => {
            //@assuming that range of Node == range of Token
            let range = lit.range(ctx.tree);
            let string = &ctx.source[range.as_usize()];

            let val = match string.parse::<f64>() {
                Ok(value) => value,
                Err(error) => {
                    ctx.s.errors.push(ErrorComp::new(
                        format!("parse float error: {}", error),
                        SourceRange::new(ctx.module_id, range),
                        None,
                    ));
                    0.0
                }
            };

            ast::ExprKind::LitFloat { val }
        }
        cst::Expr::LitChar(_) => {
            let val = ctx.tree.tokens().char(ctx.char_id as usize);
            ctx.char_id += 1;

            ast::ExprKind::LitChar { val }
        }
        cst::Expr::LitString(_) => {
            let (string, c_string) = ctx.tree.tokens().string(ctx.string_id as usize);
            let id = ctx.s.intern_string.intern(string);
            ctx.string_id += 1;

            if id.index() >= ctx.s.string_is_cstr.len() {
                ctx.s.string_is_cstr.push(c_string);
            } else if c_string {
                ctx.s.string_is_cstr[id.index()] = true;
            }

            ast::ExprKind::LitString { id, c_string }
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
            let branches = ctx.s.branches.take(offset, &mut ctx.s.arena);
            let else_block = if_.else_block(ctx.tree).map(|b| block(ctx, b));

            let if_ = ast::If {
                entry,
                branches,
                else_block,
            };
            let if_ = ctx.s.arena.alloc(if_);
            ast::ExprKind::If { if_ }
        }
        cst::Expr::Block(expr_block) => {
            let block = block(ctx, expr_block.into_block());

            let block = ctx.s.arena.alloc(block);
            ast::ExprKind::Block { block }
        }
        cst::Expr::Match(match_) => {
            let on_expr = expr(ctx, match_.on_expr(ctx.tree).unwrap());

            let offset = ctx.s.match_arms.start();
            let match_arm_lit = match_.match_arm_list(ctx.tree).unwrap();
            for match_arm_cst in match_arm_lit.match_arms(ctx.tree) {
                let mut pat_expr_iter = match_arm_cst.pat_expr_iter(ctx.tree);
                let match_arm = ast::MatchArm {
                    pat: ast::ConstExpr(expr(ctx, pat_expr_iter.next().unwrap())),
                    expr: expr(ctx, pat_expr_iter.next().unwrap()),
                };
                ctx.s.match_arms.add(match_arm);
            }
            let match_arms = ctx.s.match_arms.take(offset, &mut ctx.s.arena);

            let (fallback, fallback_range) = match match_arm_lit.fallback(ctx.tree) {
                Some(fallback) => {
                    let expr = expr(ctx, fallback.expr(ctx.tree).unwrap());
                    let range = fallback.range(ctx.tree);
                    (Some(expr), range)
                }
                None => (None, TextRange::empty_at(0.into())),
            };

            //@sync names `arms` vs `match_arms`
            let match_ = ast::Match {
                on_expr,
                arms: match_arms,
                fallback,
                fallback_range,
            };
            let match_ = ctx.s.arena.alloc(match_);
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

            let offset = ctx.s.exprs.start();
            let argument_list = call.call_argument_list(ctx.tree).unwrap();
            for input in argument_list.inputs(ctx.tree) {
                let expr = expr(ctx, input);
                ctx.s.exprs.add(expr);
            }
            let input = ctx.s.exprs.take(offset, &mut ctx.s.arena);

            let input = ctx.s.arena.alloc(input);
            ast::ExprKind::Call { target, input }
        }
        cst::Expr::Cast(cast) => {
            let target = expr(ctx, cast.target(ctx.tree).unwrap());
            let into = ty(ctx, cast.into_ty(ctx.tree).unwrap());

            let into = ctx.s.arena.alloc(into);
            ast::ExprKind::Cast { target, into }
        }
        cst::Expr::Sizeof(sizeof) => {
            let ty = ty(ctx, sizeof.ty(ctx.tree).unwrap());

            let ty_ref = ctx.s.arena.alloc(ty);
            ast::ExprKind::Sizeof { ty: ty_ref }
        }
        cst::Expr::Item(item) => {
            let path = path(ctx, item.path(ctx.tree).unwrap());

            ast::ExprKind::Item { path }
        }
        cst::Expr::Variant(variant) => {
            let name = name(ctx, variant.name(ctx.tree).unwrap());

            ast::ExprKind::Variant { name }
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
                        ast::ExprKind::Item { path } => path.names[0],
                        _ => unreachable!(),
                    }
                };
                let field_init = ast::FieldInit { name, expr };
                ctx.s.field_inits.add(field_init);
            }
            let input = ctx.s.field_inits.take(offset, &mut ctx.s.arena);

            let struct_init = ast::StructInit { path, input };
            let struct_init = ctx.s.arena.alloc(struct_init);
            ast::ExprKind::StructInit { struct_init }
        }
        cst::Expr::ArrayInit(array_init) => {
            let offset = ctx.s.exprs.start();
            for input in array_init.inputs(ctx.tree) {
                let expr = expr(ctx, input);
                ctx.s.exprs.add(expr);
            }
            let input = ctx.s.exprs.take(offset, &mut ctx.s.arena);

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
        cst::Expr::RangeFull(_) => {
            let range = ast::Range::Full;
            let range = ctx.s.arena.alloc(range);
            ast::ExprKind::Range { range }
        }
        cst::Expr::RangeTo(range) => {
            let end = expr(ctx, range.end(ctx.tree).unwrap());

            let range = ast::Range::RangeTo(end);
            let range = ctx.s.arena.alloc(range);
            ast::ExprKind::Range { range }
        }
        cst::Expr::RangeToInclusive(range) => {
            let end = expr(ctx, range.end(ctx.tree).unwrap());

            let range = ast::Range::RangeToInclusive(end);
            let range = ctx.s.arena.alloc(range);
            ast::ExprKind::Range { range }
        }
        cst::Expr::RangeFrom(range) => {
            let start = expr(ctx, range.start(ctx.tree).unwrap());

            let range = ast::Range::RangeFrom(start);
            let range = ctx.s.arena.alloc(range);
            ast::ExprKind::Range { range }
        }
        cst::Expr::Range(range) => {
            let mut start_end_iter = range.start_end_iter(ctx.tree);
            let start = expr(ctx, start_end_iter.next().unwrap());
            let end = expr(ctx, start_end_iter.next().unwrap());

            let range = ast::Range::Range(start, end);
            let range = ctx.s.arena.alloc(range);
            ast::ExprKind::Range { range }
        }
        cst::Expr::RangeInclusive(range) => {
            let mut start_end_iter = range.start_end_iter(ctx.tree);
            let start = expr(ctx, start_end_iter.next().unwrap());
            let end = expr(ctx, start_end_iter.next().unwrap());

            let range = ast::Range::RangeInclusive(start, end);
            let range = ctx.s.arena.alloc(range);
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
            let bin = ctx.s.arena.alloc(bin);
            ast::ExprKind::Binary { op, op_range, bin }
        }
    };

    let expr = ast::Expr { kind, range };
    ctx.s.arena.alloc(expr)
}

fn block<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, block: cst::Block) -> ast::Block<'ast> {
    let offset = ctx.s.stmts.start();
    for stmt_cst in block.stmts(ctx.tree) {
        let stmt = stmt(ctx, stmt_cst);
        ctx.s.stmts.add(stmt);
    }
    let stmts = ctx.s.stmts.take(offset, &mut ctx.s.arena);

    ast::Block {
        stmts,
        range: block.range(ctx.tree),
    }
}
