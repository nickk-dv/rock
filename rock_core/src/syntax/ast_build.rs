use super::ast_layer as cst;
use super::syntax_tree::SyntaxTree;
use crate::arena::Arena;
use crate::ast;
use crate::temp_buffer::TempBuffer;

//@rename some ast:: nodes to match ast_layer and syntax names (eg: ProcParam)
pub struct AstBuild<'ast, 'syn> {
    tree: &'syn SyntaxTree<'syn>,
    arena: Arena<'ast>,
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
    todo!();
}

fn import_symbol<'ast>(ctx: &mut AstBuild<'ast, '_>, import_symbol: cst::ImportSymbol) {
    todo!();
}

fn name<'ast>(ctx: &mut AstBuild<'ast, '_>, ty: cst::Name) -> ast::Name {
    todo!();
}

fn path<'ast>(ctx: &mut AstBuild<'ast, '_>, path: cst::Path) -> &'ast ast::Path<'ast> {
    todo!();
}

fn ty<'ast>(ctx: &mut AstBuild<'ast, '_>, ty: cst::Type) -> ast::Type<'ast> {
    todo!();
}

fn stmt<'ast>(ctx: &mut AstBuild<'ast, '_>, ty: cst::Type) {
    todo!();
}

fn expr<'ast>(ctx: &mut AstBuild<'ast, '_>, ty: cst::Expr) -> &'ast ast::Expr<'ast> {
    todo!();
}

fn block<'ast>(ctx: &mut AstBuild<'ast, '_>, block: cst::Block) -> ast::Block<'ast> {
    todo!();
}
