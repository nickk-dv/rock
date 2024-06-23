use super::ast_layer as cst;
use super::syntax_tree::SyntaxTree;
use crate::arena::Arena;
use crate::ast;
use crate::intern::InternPool;
use crate::temp_buffer::TempBuffer;

//@rename some ast:: nodes to match ast_layer and syntax names (eg: ProcParam)
pub struct AstBuild<'ast, 'syn> {
    arena: Arena<'ast>,
    intern_name: InternPool<'ast>,   //@hack same as ast lifetime
    intern_string: InternPool<'ast>, //@hack same as ast lifetime
    string_is_cstr: Vec<bool>,
    tree: &'syn SyntaxTree<'syn>,
    source: &'ast str, //@hack same as ast lifetime
    items: TempBuffer<ast::Item<'ast>>,
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

pub fn source_file<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    source_file: cst::SourceFile,
) -> &'ast [ast::Item<'ast>] {
    let offset = ctx.items.start();
    for item_cst in source_file.items(ctx.tree) {
        item(ctx, item_cst);
    }
    ctx.items.take(offset, &mut ctx.arena)
}

fn item<'ast>(ctx: &mut AstBuild<'ast, '_>, item: cst::Item) {
    let item = match item {
        cst::Item::Proc(item) => ast::Item::Proc(proc_item(ctx, item)),
        cst::Item::Enum(item) => ast::Item::Enum(enum_item(ctx, item)),
        cst::Item::Struct(item) => ast::Item::Struct(struct_item(ctx, item)),
        cst::Item::Const(item) => ast::Item::Const(const_item(ctx, item)),
        cst::Item::Global(item) => ast::Item::Global(global_item(ctx, item)),
        cst::Item::Import(item) => ast::Item::Import(import_item(ctx, item)),
    };
    ctx.items.add(item);
}

fn attribute<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    attr: Option<cst::Attribute>,
) -> Option<ast::Attribute> {
    todo!()
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

fn proc_item<'ast>(ctx: &mut AstBuild<'ast, '_>, item: cst::ProcItem) -> &'ast ast::ProcItem<'ast> {
    let attr = attribute(ctx, item.attribute(ctx.tree));
    let vis = vis(item.visiblity(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());

    let offset = ctx.params.start();
    let param_list = item.param_list(ctx.tree).unwrap();
    for param_cst in param_list.params(ctx.tree) {
        param(ctx, param_cst);
    }
    let params = ctx.params.take(offset, &mut ctx.arena);

    let is_variadic = param_list.is_variadic(ctx.tree);
    let return_ty = item.return_ty(ctx.tree).map(|t| ty(ctx, t));
    let block = item.block(ctx.tree).map(|b| block(ctx, b));

    let proc_item = ast::ProcItem {
        attr,
        vis,
        name,
        params,
        is_variadic,
        return_ty,
        block,
    };
    ctx.arena.alloc(proc_item)
}

fn param<'ast>(ctx: &mut AstBuild<'ast, '_>, param: cst::Param) {
    let mutt = mutt(param.is_mut(ctx.tree));
    let name = name(ctx, param.name(ctx.tree).unwrap());
    let ty = ty(ctx, param.ty(ctx.tree).unwrap());

    let param = ast::ProcParam { mutt, name, ty };
    ctx.params.add(param);
}

fn enum_item<'ast>(ctx: &mut AstBuild<'ast, '_>, item: cst::EnumItem) -> &'ast ast::EnumItem<'ast> {
    //@add ast::EnumItem attr
    let attr = attribute(ctx, item.attribute(ctx.tree));
    let vis = vis(item.visiblity(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let basic = item.type_basic(ctx.tree).map(|tb| tb.basic(ctx.tree));

    let offset = ctx.variants.start();
    let variant_list = item.variant_list(ctx.tree).unwrap();
    for variant_cst in variant_list.variants(ctx.tree) {
        variant(ctx, variant_cst);
    }
    let variants = ctx.variants.take(offset, &mut ctx.arena);

    let enum_item = ast::EnumItem {
        vis,
        name,
        basic,
        variants,
    };
    ctx.arena.alloc(enum_item)
}

fn variant<'ast>(ctx: &mut AstBuild<'ast, '_>, variant: cst::Variant) {
    //@value is optional in grammar but required in ast due to
    // const expr resolve limitation, will panic for now
    let name = name(ctx, variant.name(ctx.tree).unwrap());
    let value = ast::ConstExpr(expr(ctx, variant.value(ctx.tree).unwrap()));

    let variant = ast::EnumVariant { name, value };
    ctx.variants.add(variant);
}

fn struct_item<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    item: cst::StructItem,
) -> &'ast ast::StructItem<'ast> {
    //@add ast::StructItem attr
    let attr = attribute(ctx, item.attribute(ctx.tree));
    let vis = vis(item.visiblity(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());

    let offset = ctx.fields.start();
    let field_list = item.field_list(ctx.tree).unwrap();
    for field_cst in field_list.fields(ctx.tree) {
        field(ctx, field_cst);
    }
    let fields = ctx.fields.take(offset, &mut ctx.arena);

    let struct_item = ast::StructItem { vis, name, fields };
    ctx.arena.alloc(struct_item)
}

fn field<'ast>(ctx: &mut AstBuild<'ast, '_>, field: cst::Field) {
    let vis = ast::Vis::Public; //@remove support for field visibility (not checked anyways)?
    let name = name(ctx, field.name(ctx.tree).unwrap());
    let ty = ty(ctx, field.ty(ctx.tree).unwrap());

    let field = ast::StructField { vis, name, ty };
    ctx.fields.add(field);
}

fn const_item<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    item: cst::ConstItem,
) -> &'ast ast::ConstItem<'ast> {
    //@add ast::ConstItem attr
    let attr = attribute(ctx, item.attribute(ctx.tree));
    let vis = vis(item.visiblity(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let ty = ty(ctx, item.ty(ctx.tree).unwrap());
    let value = ast::ConstExpr(expr(ctx, item.value(ctx.tree).unwrap()));

    let const_item = ast::ConstItem {
        vis,
        name,
        ty,
        value,
    };
    ctx.arena.alloc(const_item)
}

fn global_item<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    item: cst::GlobalItem,
) -> &'ast ast::GlobalItem<'ast> {
    let attr = attribute(ctx, item.attribute(ctx.tree));
    let vis = vis(item.visiblity(ctx.tree).is_some());
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let mutt = mutt(item.is_mut(ctx.tree));
    let ty = ty(ctx, item.ty(ctx.tree).unwrap());
    let value = ast::ConstExpr(expr(ctx, item.value(ctx.tree).unwrap()));

    let global_item = ast::GlobalItem {
        attr,
        vis,
        name,
        mutt,
        ty,
        value,
    };
    ctx.arena.alloc(global_item)
}

fn import_item<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    item: cst::ImportItem,
) -> &'ast ast::ImportItem<'ast> {
    //@add ast::ImportItem attr
    let import_symbols = if let Some(symbol_list) = item.import_symbol_list(ctx.tree) {
        let offset = ctx.fields.start();
        for symbol_cst in symbol_list.import_symbols(ctx.tree) {
            import_symbol(ctx, symbol_cst);
        }
        ctx.fields.take(offset, &mut ctx.arena)
    } else {
        &[]
    };

    todo!();
}

fn import_symbol<'ast>(ctx: &mut AstBuild<'ast, '_>, import_symbol: cst::ImportSymbol) {
    let name_ast = name(ctx, import_symbol.name(ctx.tree).unwrap());
    let alias = import_symbol
        .name_alias(ctx.tree)
        .map(|a| name(ctx, a.name(ctx.tree).unwrap()));

    let import_symbol = ast::ImportSymbol {
        name: name_ast,
        alias,
    };
    ctx.import_symbols.add(import_symbol);
}

fn name<'ast>(ctx: &mut AstBuild<'ast, '_>, name: cst::Name) -> ast::Name {
    let range = name.range(ctx.tree);
    let string = &ctx.source[range.as_usize()];
    let id = ctx.intern_name.intern(string);
    ast::Name { range, id }
}

fn path<'ast>(ctx: &mut AstBuild<'ast, '_>, path: cst::Path) -> &'ast ast::Path<'ast> {
    let offset = ctx.names.start();
    for name_cst in path.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.names.add(name);
    }
    let names = ctx.names.take(offset, &mut ctx.arena);

    ctx.arena.alloc(ast::Path { names })
}

fn ty<'ast>(ctx: &mut AstBuild<'ast, '_>, ty_cst: cst::Type) -> ast::Type<'ast> {
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
            let offset = ctx.types.start();
            let param_type_list = proc_ty.param_type_list(ctx.tree).unwrap();
            for ty_cst in param_type_list.param_types(ctx.tree) {
                let ty = ty(ctx, ty_cst);
                ctx.types.add(ty);
            }
            let param_types = ctx.types.take(offset, &mut ctx.arena);

            let is_variadic = param_type_list.is_variadic(ctx.tree);
            let return_ty = proc_ty.return_ty(ctx.tree).map(|t| ty(ctx, t));

            let proc_ty = ast::ProcType {
                params: param_types, //@rename params to param_types
                return_ty,
                is_variadic, //@reorder after params
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

fn stmt<'ast>(ctx: &mut AstBuild<'ast, '_>, stmt: cst::Stmt) -> ast::Stmt<'ast> {
    let range = stmt.range(ctx.tree);

    let kind = match stmt {
        cst::Stmt::Break(_) => ast::StmtKind::Break,
        cst::Stmt::Continue(_) => ast::StmtKind::Continue,
        cst::Stmt::Return(_) => todo!(),
        cst::Stmt::Defer(_) => todo!(),
        cst::Stmt::Loop(_) => todo!(),
        cst::Stmt::Local(_) => todo!(),
        cst::Stmt::Assign(_) => todo!(),
        cst::Stmt::ExprSemi(_) => todo!(),
        cst::Stmt::ExprTail(_) => todo!(),
    };

    ast::Stmt { kind, range }
}

fn expr<'ast>(ctx: &mut AstBuild<'ast, '_>, expr: cst::Expr) -> &'ast ast::Expr<'ast> {
    let range = expr.range(ctx.tree);

    let kind = match expr {
        cst::Expr::Paren(paren) => {
            //@problem with range and inner expr
            todo!();
        }
        cst::Expr::LitNull(_) => ast::ExprKind::LitNull,
        cst::Expr::LitBool(_) => todo!(), //@add get on actual token type
        cst::Expr::LitInt(_) => todo!(),
        cst::Expr::LitFloat(_) => todo!(),
        cst::Expr::LitChar(_) => todo!(),
        cst::Expr::LitString(_) => todo!(),
        cst::Expr::If(_) => todo!(),
        cst::Expr::Block(_) => todo!(),
        cst::Expr::Match(_) => todo!(),
        cst::Expr::Field(_) => todo!(),
        cst::Expr::Index(_) => todo!(),
        cst::Expr::Call(_) => todo!(),
        cst::Expr::Cast(_) => todo!(),
        cst::Expr::Sizeof(sizeof) => {
            let ty = ty(ctx, sizeof.ty(ctx.tree).unwrap());
            let ty_ref = ctx.arena.alloc(ty);
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
        cst::Expr::StructInit(_) => todo!(),
        cst::Expr::ArrayInit(_) => todo!(),
        cst::Expr::ArrayRepeat(_) => todo!(),
        cst::Expr::Deref(_) => todo!(),
        cst::Expr::Address(_) => todo!(),
        cst::Expr::Unary(_) => todo!(),
        cst::Expr::Binary(_) => todo!(),
    };

    let expr = ast::Expr { kind, range };
    ctx.arena.alloc(expr)
}

fn block<'ast>(ctx: &mut AstBuild<'ast, '_>, block: cst::Block) -> ast::Block<'ast> {
    let offset = ctx.stmts.start();
    for stmt_cst in block.stmts(ctx.tree) {
        let stmt = stmt(ctx, stmt_cst);
        ctx.stmts.add(stmt);
    }
    let stmts = ctx.stmts.take(offset, &mut ctx.arena);

    ast::Block {
        stmts,
        range: block.range(ctx.tree),
    }
}
