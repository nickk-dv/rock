use super::ast_layer::{self as cst, AstNode};
use super::syntax_tree::SyntaxTree;
use crate::ast;
use crate::intern::{InternPool, LitID, NameID};
use crate::support::{Arena, TempBuffer};
use crate::text::TextRange;

pub struct AstBuild<'ast, 'state, 's, 'sref> {
    arena: Arena<'ast>,
    tree: &'sref SyntaxTree<'s>,
    source: &'sref str,
    intern_name: &'sref mut InternPool<'s, NameID>,
    s: &'state mut AstBuildState<'ast>,
}

pub struct AstBuildState<'ast> {
    items: TempBuffer<ast::Item<'ast>>,
    params: TempBuffer<ast::Param<'ast>>,
    variants: TempBuffer<ast::Variant<'ast>>,
    fields: TempBuffer<ast::Field<'ast>>,
    import_symbols: TempBuffer<ast::ImportSymbol>,
    directives: TempBuffer<ast::Directive<'ast>>,
    directive_params: TempBuffer<ast::DirectiveParam>,
    types: TempBuffer<ast::Type<'ast>>,
    proc_type_params: TempBuffer<ast::ParamKind<'ast>>,
    stmts: TempBuffer<ast::Stmt<'ast>>,
    exprs: TempBuffer<&'ast ast::Expr<'ast>>,
    branches: TempBuffer<ast::Branch<'ast>>,
    match_arms: TempBuffer<ast::MatchArm<'ast>>,
    pats: TempBuffer<ast::Pat<'ast>>,
    field_inits: TempBuffer<ast::FieldInit<'ast>>,
    names: TempBuffer<ast::Name>,
    binds: TempBuffer<ast::Binding>,
    segments: TempBuffer<ast::PathSegment<'ast>>,
}

impl<'ast, 'state, 's, 'sref> AstBuild<'ast, 'state, 's, 'sref> {
    pub fn new(
        tree: &'sref SyntaxTree<'s>,
        source: &'sref str,
        intern_name: &'sref mut InternPool<'s, NameID>,
        state: &'state mut AstBuildState<'ast>,
    ) -> Self {
        AstBuild { arena: Arena::new(), tree, source, intern_name, s: state }
    }

    pub fn finish(self, items: &'ast [ast::Item<'ast>]) -> ast::Ast<'ast> {
        ast::Ast { arena: self.arena, items }
    }
}

impl<'ast> AstBuildState<'ast> {
    pub fn new() -> AstBuildState<'ast> {
        AstBuildState {
            items: TempBuffer::new(128),
            params: TempBuffer::new(32),
            variants: TempBuffer::new(32),
            fields: TempBuffer::new(32),
            import_symbols: TempBuffer::new(32),
            directives: TempBuffer::new(32),
            directive_params: TempBuffer::new(32),
            types: TempBuffer::new(32),
            proc_type_params: TempBuffer::new(32),
            stmts: TempBuffer::new(32),
            exprs: TempBuffer::new(32),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(32),
            pats: TempBuffer::new(32),
            field_inits: TempBuffer::new(32),
            names: TempBuffer::new(32),
            binds: TempBuffer::new(32),
            segments: TempBuffer::new(32),
        }
    }
}

pub fn source_file<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
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
        cst::Item::Directive(dir_list) => ast::Item::Directive(directive_list(ctx, dir_list)),
    };
    ctx.s.items.push(item);
}

#[inline]
fn mutt(t_mut: Option<TextRange>) -> ast::Mut {
    if t_mut.is_some() {
        ast::Mut::Mutable
    } else {
        ast::Mut::Immutable
    }
}

fn proc_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::ProcItem,
) -> &'ast ast::ProcItem<'ast> {
    let dir_list = directive_list_opt(ctx, item.dir_list(ctx.tree));
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let poly_params = item.poly_params(ctx.tree).map(|poly| polymorph_params(ctx, poly));

    let offset = ctx.s.params.start();
    let param_list = item.param_list(ctx.tree).unwrap();
    for param_cst in param_list.params(ctx.tree) {
        param(ctx, param_cst);
    }
    let params = ctx.s.params.take(offset, &mut ctx.arena);

    let return_ty = ty(ctx, item.return_ty(ctx.tree).unwrap());
    let block = item.block(ctx.tree).map(|b| block(ctx, b));

    let proc_item = ast::ProcItem { dir_list, name, poly_params, params, return_ty, block };
    ctx.arena.alloc(proc_item)
}

fn param(ctx: &mut AstBuild, param: cst::Param) {
    let mutt = mutt(param.t_mut(ctx.tree));
    let name = name(ctx, param.name(ctx.tree).unwrap());

    let kind = if let Some(ty_cst) = param.ty(ctx.tree) {
        let ty = ty(ctx, ty_cst);
        ast::ParamKind::Normal(ty)
    } else {
        let dir = param.directive(ctx.tree).unwrap();
        let dir = directive(ctx, dir);
        ast::ParamKind::Implicit(ctx.arena.alloc(dir))
    };

    let param = ast::Param { mutt, name, kind };
    ctx.s.params.push(param);
}

fn enum_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::EnumItem,
) -> &'ast ast::EnumItem<'ast> {
    let dir_list = directive_list_opt(ctx, item.dir_list(ctx.tree));
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let poly_params = item.poly_params(ctx.tree).map(|poly| polymorph_params(ctx, poly));

    let tag_ty = if let Some((basic, range)) = item.tag_ty(ctx.tree) {
        let tag_ty = ast::EnumTagType { basic, range };
        Some(ctx.arena.alloc(tag_ty))
    } else {
        None
    };

    let offset = ctx.s.variants.start();
    let variant_list = item.variant_list(ctx.tree).unwrap();
    for variant_cst in variant_list.variants(ctx.tree) {
        variant(ctx, variant_cst);
    }
    let variants = ctx.s.variants.take(offset, &mut ctx.arena);

    let enum_item = ast::EnumItem { dir_list, name, poly_params, tag_ty, variants };
    ctx.arena.alloc(enum_item)
}

fn variant(ctx: &mut AstBuild, variant: cst::Variant) {
    let dir_list = directive_list_opt(ctx, variant.dir_list(ctx.tree));
    let name = name(ctx, variant.name(ctx.tree).unwrap());

    let kind = if let Some(value) = variant.value(ctx.tree) {
        let value = ast::ConstExpr(expr(ctx, value));
        ast::VariantKind::Constant(value)
    } else if let Some(field_list) = variant.field_list(ctx.tree) {
        let offset = ctx.s.types.start();
        for ty_cst in field_list.fields(ctx.tree) {
            let ty = ty(ctx, ty_cst);
            ctx.s.types.push(ty);
        }
        let types = ctx.s.types.take(offset, &mut ctx.arena);

        let field_list = ast::VariantFieldList { types, range: field_list.range() };
        let field_list = ctx.arena.alloc(field_list);
        ast::VariantKind::HasFields(field_list)
    } else {
        ast::VariantKind::Default
    };

    let variant = ast::Variant { dir_list, name, kind };
    ctx.s.variants.push(variant);
}

fn struct_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::StructItem,
) -> &'ast ast::StructItem<'ast> {
    let dir_list = directive_list_opt(ctx, item.dir_list(ctx.tree));
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let poly_params = item.poly_params(ctx.tree).map(|poly| polymorph_params(ctx, poly));

    let offset = ctx.s.fields.start();
    let field_list = item.field_list(ctx.tree).unwrap();
    for field_cst in field_list.fields(ctx.tree) {
        field(ctx, field_cst);
    }
    let fields = ctx.s.fields.take(offset, &mut ctx.arena);

    let struct_item = ast::StructItem { dir_list, name, poly_params, fields };
    ctx.arena.alloc(struct_item)
}

fn field(ctx: &mut AstBuild, field: cst::Field) {
    let dir_list = directive_list_opt(ctx, field.dir_list(ctx.tree));
    let name = name(ctx, field.name(ctx.tree).unwrap());
    let ty = ty(ctx, field.ty(ctx.tree).unwrap());

    let field = ast::Field { dir_list, name, ty };
    ctx.s.fields.push(field);
}

fn const_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::ConstItem,
) -> &'ast ast::ConstItem<'ast> {
    let dir_list = directive_list_opt(ctx, item.dir_list(ctx.tree));
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let ty = item.ty(ctx.tree).map(|ty_cst| ty(ctx, ty_cst));
    let value = ast::ConstExpr(expr(ctx, item.value(ctx.tree).unwrap()));

    #[rustfmt::skip]
    let const_item = ast::ConstItem { dir_list, name, ty, value };
    ctx.arena.alloc(const_item)
}

fn global_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::GlobalItem,
) -> &'ast ast::GlobalItem<'ast> {
    let dir_list = directive_list_opt(ctx, item.dir_list(ctx.tree));
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let mutt = mutt(item.t_mut(ctx.tree));
    let ty = ty(ctx, item.ty(ctx.tree).unwrap());

    let init = if let Some(value) = item.value(ctx.tree) {
        let value = ast::ConstExpr(expr(ctx, value));
        ast::GlobalInit::Init(value)
    } else {
        ast::GlobalInit::Zeroed
    };

    #[rustfmt::skip]
    let global_item = ast::GlobalItem { dir_list, name, mutt, ty, init };
    ctx.arena.alloc(global_item)
}

fn import_item<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    item: cst::ImportItem,
) -> &'ast ast::ImportItem<'ast> {
    let dir_list = directive_list_opt(ctx, item.dir_list(ctx.tree));
    let package = item.package(ctx.tree).map(|n| name(ctx, n));

    let offset = ctx.s.names.start();
    let import_path = item.import_path(ctx.tree).unwrap();
    for name_cst in import_path.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.s.names.push(name);
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

    let import_item = ast::ImportItem { dir_list, package, import_path, rename, symbols };
    ctx.arena.alloc(import_item)
}

fn import_symbol(ctx: &mut AstBuild, import_symbol: cst::ImportSymbol) {
    let name = name(ctx, import_symbol.name(ctx.tree).unwrap());
    let rename = import_symbol_rename(ctx, import_symbol.rename(ctx.tree));

    let import_symbol = ast::ImportSymbol { name, rename };
    ctx.s.import_symbols.push(import_symbol);
}

fn import_symbol_rename(
    ctx: &mut AstBuild,
    rename: Option<cst::ImportSymbolRename>,
) -> ast::SymbolRename {
    if let Some(rename) = rename {
        if let Some(alias) = rename.alias(ctx.tree) {
            ast::SymbolRename::Alias(name(ctx, alias))
        } else {
            let range = rename.t_discard(ctx.tree).unwrap();
            ast::SymbolRename::Discard(range)
        }
    } else {
        ast::SymbolRename::None
    }
}

fn directive_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    dir_list: cst::DirectiveList,
) -> &'ast ast::DirectiveList<'ast> {
    let offset = ctx.s.directives.start();
    for directive_cst in dir_list.directives(ctx.tree) {
        let directive = directive(ctx, directive_cst);
        ctx.s.directives.push(directive);
    }
    let directives = ctx.s.directives.take(offset, &mut ctx.arena);
    ctx.arena.alloc(ast::DirectiveList { directives })
}

fn directive_list_opt<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    dir_list: Option<cst::DirectiveList>,
) -> Option<&'ast ast::DirectiveList<'ast>> {
    if let Some(dir_list) = dir_list {
        Some(directive_list(ctx, dir_list))
    } else {
        None
    }
}

fn directive<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    directive: cst::Directive,
) -> ast::Directive<'ast> {
    let kind = match directive {
        cst::Directive::Simple(dir) => {
            let name = name(ctx, dir.name(ctx.tree).unwrap());
            match ctx.intern_name.get(name.id) {
                "c_call" => ast::DirectiveKind::CCall,
                "inline" => ast::DirectiveKind::Inline,
                "variadic" => ast::DirectiveKind::Variadic,
                "c_variadic" => ast::DirectiveKind::CVariadic,
                "caller_location" => ast::DirectiveKind::CallerLocation,
                "scope_public" => ast::DirectiveKind::ScopePublic,
                "scope_package" => ast::DirectiveKind::ScopePackage,
                "scope_private" => ast::DirectiveKind::ScopePrivate,
                _ => ast::DirectiveKind::Error(name),
            }
        }
        cst::Directive::WithParams(dir) => {
            let name = name(ctx, dir.name(ctx.tree).unwrap());
            let params = directive_param_list(ctx, dir.param_list(ctx.tree).unwrap());
            match ctx.intern_name.get(name.id) {
                "config" => ast::DirectiveKind::Config(params),
                "config_any" => ast::DirectiveKind::ConfigAny(params),
                "config_not" => ast::DirectiveKind::ConfigNot(params),
                _ => unreachable!(),
            }
        }
    };

    ast::Directive { kind, range: directive.range() }
}

fn directive_param_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    param_list: cst::DirectiveParamList,
) -> &'ast [ast::DirectiveParam] {
    let offset = ctx.s.directive_params.start();
    for param_cst in param_list.params(ctx.tree) {
        let param = directive_param(ctx, param_cst);
        ctx.s.directive_params.push(param);
    }
    ctx.s.directive_params.take(offset, &mut ctx.arena)
}

fn directive_param(ctx: &mut AstBuild, param: cst::DirectiveParam) -> ast::DirectiveParam {
    let name = name(ctx, param.name(ctx.tree).unwrap());
    let lit_string = param.value(ctx.tree).unwrap();
    let value = string_lit(ctx, lit_string);
    let value_range = lit_string.range();

    ast::DirectiveParam { name, value, value_range }
}

fn ty<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, ty_cst: cst::Type) -> ast::Type<'ast> {
    let range = ty_cst.range();

    let kind = match ty_cst {
        cst::Type::Basic(ty_cst) => {
            let (basic, _) = ty_cst.basic(ctx.tree).unwrap();
            ast::TypeKind::Basic(basic)
        }
        cst::Type::Custom(ty_cst) => {
            let path = path(ctx, ty_cst.path(ctx.tree).unwrap());
            ast::TypeKind::Custom(path)
        }
        cst::Type::Reference(ty_cst) => {
            let mutt = mutt(ty_cst.t_mut(ctx.tree));
            let ref_ty = ty(ctx, ty_cst.ref_ty(ctx.tree).unwrap());
            ast::TypeKind::Reference(mutt, ctx.arena.alloc(ref_ty))
        }
        cst::Type::MultiReference(ty_cst) => {
            let mutt = mutt(ty_cst.t_mut(ctx.tree));
            let ref_ty = ty(ctx, ty_cst.ref_ty(ctx.tree).unwrap());
            ast::TypeKind::MultiReference(mutt, ctx.arena.alloc(ref_ty))
        }
        cst::Type::Procedure(proc_ty) => {
            let directive = proc_ty.directive(ctx.tree).map(|d| {
                let directive = directive(ctx, d);
                ctx.arena.alloc(directive)
            });

            let offset = ctx.s.proc_type_params.start();
            let param_list = proc_ty.param_list(ctx.tree).unwrap();
            for param in param_list.params(ctx.tree) {
                let kind = proc_type_param(ctx, param);
                ctx.s.proc_type_params.push(kind);
            }
            let params = ctx.s.proc_type_params.take(offset, &mut ctx.arena);

            let return_ty = proc_ty.return_ty(ctx.tree).unwrap();
            let return_ty = ty(ctx, return_ty);

            let proc_ty = ast::ProcType { directive, params, return_ty };
            ast::TypeKind::Procedure(ctx.arena.alloc(proc_ty))
        }
        cst::Type::ArraySlice(slice) => {
            let mutt = mutt(slice.t_mut(ctx.tree));
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

fn proc_type_param<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    param: cst::ProcTypeParam,
) -> ast::ParamKind<'ast> {
    if let Some(ty_cst) = param.ty(ctx.tree) {
        let ty = ty(ctx, ty_cst);
        ast::ParamKind::Normal(ty)
    } else {
        let dir = param.directive(ctx.tree).unwrap();
        let dir = directive(ctx, dir);
        ast::ParamKind::Implicit(ctx.arena.alloc(dir))
    }
}

fn stmt<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, stmt_cst: cst::Stmt) -> ast::Stmt<'ast> {
    let range = stmt_cst.range();

    let kind = match stmt_cst {
        cst::Stmt::Break(_) => ast::StmtKind::Break,
        cst::Stmt::Continue(_) => ast::StmtKind::Continue,
        cst::Stmt::Return(ret) => {
            let expr = ret.expr(ctx.tree).map(|e| expr(ctx, e));
            ast::StmtKind::Return(expr)
        }
        cst::Stmt::Defer(defer) => ast::StmtKind::Defer(stmt_defer(ctx, defer)),
        cst::Stmt::For(for_) => ast::StmtKind::For(stmt_for(ctx, for_)),
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
        cst::Stmt::WithDirective(stmt_dir) => {
            let dir_list = directive_list_opt(ctx, stmt_dir.dir_list(ctx.tree));
            let stmt = stmt(ctx, stmt_dir.stmt(ctx.tree).unwrap());

            let stmt = ast::StmtWithDirective { dir_list, stmt };
            let stmt = ctx.arena.alloc(stmt);
            ast::StmtKind::WithDirective(stmt)
        }
    };

    ast::Stmt { kind, range }
}

fn stmt_defer<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    defer: cst::StmtDefer,
) -> &'ast ast::Block<'ast> {
    if let Some(block_cst) = defer.block(ctx.tree) {
        let block = block(ctx, block_cst);
        ctx.arena.alloc(block)
    } else {
        let stmt = stmt(ctx, defer.stmt(ctx.tree).unwrap());
        let stmts = ctx.arena.alloc_slice(&[stmt]);
        let block = ast::Block { stmts, range: stmt.range };
        ctx.arena.alloc(block)
    }
}

fn stmt_for<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    for_: cst::StmtFor,
) -> &'ast ast::For<'ast> {
    let header = if let Some(header) = for_.header_cond(ctx.tree) {
        let expr = expr(ctx, header.expr(ctx.tree).unwrap());
        ast::ForHeader::Cond(expr)
    } else if let Some(header) = for_.header_elem(ctx.tree) {
        let ref_mut = if header.t_ampersand(ctx.tree).is_some() {
            Some(mutt(header.t_mut(ctx.tree)))
        } else {
            None
        };
        let value = for_bind(ctx, header.value(ctx.tree).unwrap());
        let index =
            if let Some(bind) = header.index(ctx.tree) { for_bind(ctx, bind) } else { None };
        let reverse = header.t_rev(ctx.tree).is_some();
        let expr = expr(ctx, header.expr(ctx.tree).unwrap());

        let header = ast::ForHeaderElem { ref_mut, value, index, reverse, expr };
        ast::ForHeader::Elem(ctx.arena.alloc(header))
    } else if let Some(header) = for_.header_range(ctx.tree) {
        let ref_start = header.t_ampersand(ctx.tree).map(|range| range.start());
        let value = for_bind(ctx, header.value(ctx.tree).unwrap());
        let index =
            if let Some(bind) = header.index(ctx.tree) { for_bind(ctx, bind) } else { None };
        let reverse_start = header.t_rev(ctx.tree).map(|range| range.start());
        let (start, end, kind) = if header.t_exclusive(ctx.tree).is_some() {
            (
                header.start_exclusive(ctx.tree).unwrap(),
                header.end_exclusive(ctx.tree).unwrap(),
                ast::RangeKind::Exclusive,
            )
        } else {
            (
                header.start_inclusive(ctx.tree).unwrap(),
                header.end_inclusive(ctx.tree).unwrap(),
                ast::RangeKind::Inclusive,
            )
        };
        let start = expr(ctx, start);
        let end = expr(ctx, end);

        let header =
            ast::ForHeaderRange { ref_start, value, index, reverse_start, start, end, kind };
        ast::ForHeader::Range(ctx.arena.alloc(header))
    } else if let Some(header) = for_.header_pat(ctx.tree) {
        let pat = pat(ctx, header.pat(ctx.tree).unwrap());
        let expr = expr(ctx, header.expr(ctx.tree).unwrap());

        let header = ast::ForHeaderPat { pat, expr };
        ast::ForHeader::Pat(ctx.arena.alloc(header))
    } else {
        ast::ForHeader::Loop
    };

    let block = block(ctx, for_.block(ctx.tree).unwrap());
    let for_ = ast::For { header, block };
    ctx.arena.alloc(for_)
}

fn for_bind(ctx: &mut AstBuild, bind: cst::ForBind) -> Option<ast::Name> {
    bind.name(ctx.tree).map(|n| name(ctx, n))
}

fn stmt_local<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    local: cst::StmtLocal,
) -> &'ast ast::Local<'ast> {
    let bind = bind(ctx, local.bind(ctx.tree).unwrap());
    let ty = local.ty(ctx.tree).map(|ty_cst| ty(ctx, ty_cst));

    let init = if let Some(expr_cst) = local.init(ctx.tree) {
        ast::LocalInit::Init(expr(ctx, expr_cst))
    } else if let Some(range) = local.t_zeroed(ctx.tree) {
        ast::LocalInit::Zeroed(range)
    } else {
        ast::LocalInit::Undefined(local.t_undefined(ctx.tree).unwrap())
    };

    let local = ast::Local { bind, ty, init };
    ctx.arena.alloc(local)
}

fn stmt_assign<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    assign: cst::StmtAssign,
) -> &'ast ast::Assign<'ast> {
    let (op, op_range) = assign.assign_op(ctx.tree).unwrap();
    let lhs = expr(ctx, assign.lhs(ctx.tree).unwrap());
    let rhs = expr(ctx, assign.rhs(ctx.tree).unwrap());

    let assign = ast::Assign { op, op_range, lhs, rhs };
    ctx.arena.alloc(assign)
}

fn expr<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, expr_cst: cst::Expr) -> &'ast ast::Expr<'ast> {
    let range = expr_cst.range();
    let kind = expr_kind(ctx, expr_cst);
    let expr = ast::Expr { kind, range };
    ctx.arena.alloc(expr)
}

//@rename expr_cst back to expr when each arm has a function
fn expr_kind<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
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
            let offset = ctx.s.branches.start();
            for branch in if_.branches(ctx.tree) {
                let branch = ast::Branch {
                    cond: expr(ctx, branch.cond(ctx.tree).unwrap()),
                    block: block(ctx, branch.block(ctx.tree).unwrap()),
                };
                ctx.s.branches.push(branch);
            }
            let branches = ctx.s.branches.take(offset, &mut ctx.arena);
            let else_block = if_.else_block(ctx.tree).map(|b| block(ctx, b));

            let if_ = ast::If { branches, else_block };
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
            let target = expr(ctx, index.target(ctx.tree).unwrap());
            let index = expr(ctx, index.index(ctx.tree).unwrap());
            ast::ExprKind::Index { target, index }
        }
        cst::Expr::Slice(slice) => {
            let target = expr(ctx, slice.target(ctx.tree).unwrap());
            let range = if slice.t_exclusive(ctx.tree).is_some() {
                let start = slice.start_exclusive(ctx.tree).map(|e| expr(ctx, e));
                let end = expr(ctx, slice.end_exclusive(ctx.tree).unwrap());
                ast::SliceRange { start, end: Some((ast::RangeKind::Exclusive, end)) }
            } else if slice.t_inclusive(ctx.tree).is_some() {
                let start = slice.start_inclusive(ctx.tree).map(|e| expr(ctx, e));
                let end = expr(ctx, slice.end_inclusive(ctx.tree).unwrap());
                ast::SliceRange { start, end: Some((ast::RangeKind::Inclusive, end)) }
            } else {
                let start = slice.start_full(ctx.tree).map(|e| expr(ctx, e));
                ast::SliceRange { start, end: None }
            };
            ast::ExprKind::Slice { target, range: ctx.arena.alloc(range) }
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
        cst::Expr::Builtin(builtin) => {
            let builtin = match builtin {
                cst::Builtin::Error(builtin) => {
                    let name = name(ctx, builtin.name(ctx.tree).unwrap());
                    ast::Builtin::Error(name)
                }
                cst::Builtin::WithType(builtin) => {
                    let name = name(ctx, builtin.name(ctx.tree).unwrap());
                    let ty = ty(ctx, builtin.ty(ctx.tree).unwrap());
                    match ctx.intern_name.get(name.id) {
                        "size_of" => ast::Builtin::SizeOf(ty),
                        "align_of" => ast::Builtin::AlignOf(ty),
                        _ => ast::Builtin::Error(name),
                    }
                }
                cst::Builtin::Transmute(builtin) => ast::Builtin::Transmute(
                    expr(ctx, builtin.expr(ctx.tree).unwrap()),
                    ty(ctx, builtin.into_ty(ctx.tree).unwrap()),
                ),
            };
            let builtin = ctx.arena.alloc(builtin);
            ast::ExprKind::Builtin { builtin }
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
            let field_init_range = field_init_list.range();

            for field_init_cst in field_init_list.field_inits(ctx.tree) {
                let name = name(ctx, field_init_cst.name(ctx.tree).unwrap());

                let expr = if let Some(expr_cst) = field_init_cst.expr(ctx.tree) {
                    expr(ctx, expr_cst)
                } else {
                    let segment = ast::PathSegment { name, poly_args: None };
                    let segments = ctx.arena.alloc_slice(&[segment]);
                    let path = ctx.arena.alloc(ast::Path { segments });

                    let kind = ast::ExprKind::Item { path, args_list: None };
                    let expr = ast::Expr { kind, range: name.range };
                    ctx.arena.alloc(expr)
                };

                let field_init = ast::FieldInit { name, expr };
                ctx.s.field_inits.push(field_init);
            }
            let input = ctx.s.field_inits.take(offset, &mut ctx.arena);

            let input_start = field_init_range.start() + 1.into();
            let struct_init = ast::StructInit { path, input, input_start };
            let struct_init = ctx.arena.alloc(struct_init);
            ast::ExprKind::StructInit { struct_init }
        }
        cst::Expr::ArrayInit(array_init) => {
            let offset = ctx.s.exprs.start();
            for expr_cst in array_init.input(ctx.tree) {
                let expr = expr(ctx, expr_cst);
                ctx.s.exprs.push(expr);
            }
            let input = ctx.s.exprs.take(offset, &mut ctx.arena);

            ast::ExprKind::ArrayInit { input }
        }
        cst::Expr::ArrayRepeat(array_repeat) => {
            let value = expr(ctx, array_repeat.value(ctx.tree).unwrap());
            let len = ast::ConstExpr(expr(ctx, array_repeat.len(ctx.tree).unwrap()));

            ast::ExprKind::ArrayRepeat { value, len }
        }
        cst::Expr::Deref(deref) => {
            let expr = expr(ctx, deref.expr(ctx.tree).unwrap());

            ast::ExprKind::Deref { rhs: expr }
        }
        cst::Expr::Address(address) => {
            let mutt = mutt(address.t_mut(ctx.tree));
            let expr = expr(ctx, address.expr(ctx.tree).unwrap());

            ast::ExprKind::Address { mutt, rhs: expr }
        }
        cst::Expr::Unary(unary) => {
            let (op, op_range) = unary.un_op(ctx.tree).unwrap();
            let rhs = expr(ctx, unary.rhs(ctx.tree).unwrap());

            ast::ExprKind::Unary { op, op_range, rhs }
        }
        cst::Expr::Binary(binary) => {
            let (op, op_range) = binary.bin_op(ctx.tree).unwrap();
            let lhs = expr(ctx, binary.lhs(ctx.tree).unwrap());
            let rhs = expr(ctx, binary.rhs(ctx.tree).unwrap());

            ast::ExprKind::Binary { op, op_start: op_range.start(), lhs, rhs }
        }
    }
}

fn match_arm_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    match_arm_list: cst::MatchArmList,
) -> &'ast [ast::MatchArm<'ast>] {
    let offset = ctx.s.match_arms.start();
    for arm in match_arm_list.match_arms(ctx.tree) {
        let arm = match_arm(ctx, arm);
        ctx.s.match_arms.push(arm)
    }
    ctx.s.match_arms.take(offset, &mut ctx.arena)
}

fn match_arm<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    arm: cst::MatchArm,
) -> ast::MatchArm<'ast> {
    let pat = pat(ctx, arm.pat(ctx.tree).unwrap());
    let expr = expr(ctx, arm.expr(ctx.tree).unwrap());
    ast::MatchArm { pat, expr }
}

fn pat<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, pat_cst: cst::Pat) -> ast::Pat<'ast> {
    let range = pat_cst.range();

    let kind = match pat_cst {
        cst::Pat::Wild(_) => ast::PatKind::Wild,
        cst::Pat::Lit(pat) => {
            let lit_cst = pat.lit(ctx.tree).unwrap();
            let lit = lit(ctx, lit_cst);

            let lit_expr = ast::Expr { kind: ast::ExprKind::Lit { lit }, range: lit_cst.range() };
            let lit_expr = ctx.arena.alloc(lit_expr);

            let expr = if let Some((op, op_range)) = pat.un_op(ctx.tree) {
                let un_expr = ast::Expr {
                    kind: ast::ExprKind::Unary { op, op_range, rhs: lit_expr },
                    range: pat.range(),
                };
                ctx.arena.alloc(un_expr)
            } else {
                lit_expr
            };

            ast::PatKind::Lit { expr }
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
            let offset = ctx.s.pats.start();
            for pat_cst in pat_or.pats(ctx.tree) {
                let pat = pat(ctx, pat_cst);
                ctx.s.pats.push(pat);
            }
            let pats = ctx.s.pats.take(offset, &mut ctx.arena);
            ast::PatKind::Or { pats }
        }
    };

    ast::Pat { kind, range }
}

fn lit(ctx: &mut AstBuild, lit: cst::Lit) -> ast::Lit {
    match lit {
        cst::Lit::Void(_) => ast::Lit::Void,
        cst::Lit::Null(_) => ast::Lit::Null,
        cst::Lit::Bool(lit) => {
            let (val, _) = lit.value(ctx.tree).unwrap();
            ast::Lit::Bool(val)
        }
        cst::Lit::Int(lit) => {
            let token_id = lit.t_int_lit_id(ctx.tree).unwrap();
            let val = ctx.tree.tokens().int(token_id);
            ast::Lit::Int(val)
        }
        cst::Lit::Float(lit) => {
            let token_id = lit.t_float_lit_id(ctx.tree).unwrap();
            let val = ctx.tree.tokens().float(token_id);
            ast::Lit::Float(val)
        }
        cst::Lit::Char(lit) => {
            let token_id = lit.t_char_lit_id(ctx.tree).unwrap();
            let val = ctx.tree.tokens().char(token_id);
            ast::Lit::Char(val)
        }
        cst::Lit::String(lit) => ast::Lit::String(string_lit(ctx, lit)),
    }
}

//@separated due to being used for attr param value
fn string_lit(ctx: &mut AstBuild, lit: cst::LitString) -> LitID {
    let token_id = lit.t_string_lit_id(ctx.tree).unwrap();
    ctx.tree.tokens().string(token_id)
}

fn block<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, block: cst::Block) -> ast::Block<'ast> {
    let range = block.range();

    let offset = ctx.s.stmts.start();
    for stmt_cst in block.stmts(ctx.tree) {
        let stmt = stmt(ctx, stmt_cst);
        ctx.s.stmts.push(stmt);
    }
    let stmts = ctx.s.stmts.take(offset, &mut ctx.arena);

    ast::Block { stmts, range }
}

fn name(ctx: &mut AstBuild, name: cst::Name) -> ast::Name {
    let range = name.range();
    let string = &ctx.source[range.as_usize()];
    let id = ctx.intern_name.intern(string);
    ast::Name { range, id }
}

fn bind(ctx: &mut AstBuild, bind: cst::Bind) -> ast::Binding {
    if let Some(name_cst) = bind.name(ctx.tree) {
        let mutt = mutt(bind.t_mut(ctx.tree));
        let name = name(ctx, name_cst);
        ast::Binding::Named(mutt, name)
    } else {
        let range = bind.t_discard(ctx.tree).unwrap();
        ast::Binding::Discard(range)
    }
}

fn bind_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    bind_list: cst::BindList,
) -> &'ast ast::BindingList<'ast> {
    let range = bind_list.range();

    let offset = ctx.s.binds.start();
    for bind_cst in bind_list.binds(ctx.tree) {
        let bind = bind(ctx, bind_cst);
        ctx.s.binds.push(bind);
    }
    let binds = ctx.s.binds.take(offset, &mut ctx.arena);

    let bind_list = ast::BindingList { binds, range };
    ctx.arena.alloc(bind_list)
}

fn args_list<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    args_list: cst::ArgsList,
) -> &'ast ast::ArgumentList<'ast> {
    let range = args_list.range();

    let offset = ctx.s.exprs.start();
    for expr_cst in args_list.exprs(ctx.tree) {
        let expr = expr(ctx, expr_cst);
        ctx.s.exprs.push(expr);
    }
    let exprs = ctx.s.exprs.take(offset, &mut ctx.arena);

    let args_list = ast::ArgumentList { exprs, range };
    ctx.arena.alloc(args_list)
}

fn path<'ast>(ctx: &mut AstBuild<'ast, '_, '_, '_>, path: cst::Path) -> &'ast ast::Path<'ast> {
    let offset = ctx.s.segments.start();
    for segment in path.segments(ctx.tree) {
        let segment = path_segment(ctx, segment);
        ctx.s.segments.push(segment);
    }
    let segments = ctx.s.segments.take(offset, &mut ctx.arena);

    let path = ast::Path { segments };
    ctx.arena.alloc(path)
}

fn path_segment<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    segment: cst::PathSegment,
) -> ast::PathSegment<'ast> {
    let name = name(ctx, segment.name(ctx.tree).unwrap());
    let poly_args = segment.poly_args(ctx.tree).map(|pa| polymorph_args(ctx, pa));
    ast::PathSegment { name, poly_args }
}

fn polymorph_args<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    poly_args: cst::PolymorphArgs,
) -> &'ast ast::PolymorphArgs<'ast> {
    let range = poly_args.range();

    let offset = ctx.s.types.start();
    for ty_cst in poly_args.types(ctx.tree) {
        let ty = ty(ctx, ty_cst);
        ctx.s.types.push(ty);
    }
    let types = ctx.s.types.take(offset, &mut ctx.arena);

    let poly_args = ast::PolymorphArgs { types, range };
    ctx.arena.alloc(poly_args)
}

fn polymorph_params<'ast>(
    ctx: &mut AstBuild<'ast, '_, '_, '_>,
    poly_params: cst::PolymorphParams,
) -> &'ast ast::PolymorphParams<'ast> {
    let range = poly_params.range();

    let offset = ctx.s.names.start();
    for name_cst in poly_params.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.s.names.push(name);
    }
    let names = ctx.s.names.take(offset, &mut ctx.arena);

    let params = ast::PolymorphParams { names, range };
    ctx.arena.alloc(params)
}
