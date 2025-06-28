use super::ast_layer::{self as cst, AstNode};
use super::syntax_tree::SyntaxTree;
use crate::ast;
use crate::intern::{InternPool, LitID, NameID};
use crate::support::{Arena, TempBuffer};
use crate::text::TextRange;

struct AstBuild<'s, 'sref> {
    arena: Arena<'s>,
    tree: &'sref SyntaxTree,
    source: &'sref str,
    intern: &'sref mut InternPool<'s, NameID>,
    s: &'sref mut AstBuildState<'s>,
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
    array_inits: TempBuffer<ast::ArrayInit<'ast>>,
    names: TempBuffer<ast::Name>,
    binds: TempBuffer<ast::Binding>,
    segments: TempBuffer<ast::PathSegment<'ast>>,
}

impl<'ast> AstBuildState<'ast> {
    pub fn new() -> AstBuildState<'ast> {
        AstBuildState {
            items: TempBuffer::new(128),
            params: TempBuffer::new(16),
            variants: TempBuffer::new(64),
            fields: TempBuffer::new(32),
            import_symbols: TempBuffer::new(16),
            directives: TempBuffer::new(16),
            directive_params: TempBuffer::new(16),
            types: TempBuffer::new(16),
            proc_type_params: TempBuffer::new(16),
            stmts: TempBuffer::new(64),
            exprs: TempBuffer::new(64),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(64),
            pats: TempBuffer::new(16),
            field_inits: TempBuffer::new(32),
            array_inits: TempBuffer::new(64),
            names: TempBuffer::new(16),
            binds: TempBuffer::new(16),
            segments: TempBuffer::new(16),
        }
    }
}

pub fn ast<'s, 'sref>(
    tree: &'sref SyntaxTree,
    source: &'sref str,
    intern: &'sref mut InternPool<'s, NameID>,
    s: &'sref mut AstBuildState<'s>,
) -> ast::Ast<'s> {
    let mut ctx = AstBuild { arena: Arena::new(), tree, source, intern, s };
    let source = ctx.tree.source_file();
    let items = source_file(&mut ctx, source);
    ast::Ast { arena: ctx.arena, items }
}

fn source_file<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
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

fn proc_item<'ast>(ctx: &mut AstBuild<'ast, '_>, item: cst::ProcItem) -> &'ast ast::ProcItem<'ast> {
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

fn enum_item<'ast>(ctx: &mut AstBuild<'ast, '_>, item: cst::EnumItem) -> &'ast ast::EnumItem<'ast> {
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
        for field in field_list.fields(ctx.tree) {
            let ty = ty(ctx, field.ty(ctx.tree).unwrap());
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
    ctx: &mut AstBuild<'ast, '_>,
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
    ctx: &mut AstBuild<'ast, '_>,
    item: cst::ConstItem,
) -> &'ast ast::ConstItem<'ast> {
    let dir_list = directive_list_opt(ctx, item.dir_list(ctx.tree));
    let name = name(ctx, item.name(ctx.tree).unwrap());
    let ty = item.ty(ctx.tree).map(|ty_cst| ty(ctx, ty_cst));
    let value = ast::ConstExpr(expr(ctx, item.value(ctx.tree).unwrap()));

    let const_item = ast::ConstItem { dir_list, name, ty, value };
    ctx.arena.alloc(const_item)
}

fn global_item<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
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

    let global_item = ast::GlobalItem { dir_list, name, mutt, ty, init };
    ctx.arena.alloc(global_item)
}

fn import_item<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
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

//==================== DIRECTIVE ====================

fn directive_list<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
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
    ctx: &mut AstBuild<'ast, '_>,
    dir_list: Option<cst::DirectiveList>,
) -> Option<&'ast ast::DirectiveList<'ast>> {
    if let Some(dir_list) = dir_list {
        Some(directive_list(ctx, dir_list))
    } else {
        None
    }
}

fn directive<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    directive: cst::Directive,
) -> ast::Directive<'ast> {
    let kind = match directive {
        cst::Directive::Simple(dir) => {
            let name = name(ctx, dir.name(ctx.tree).unwrap());
            match ctx.intern.get(name.id) {
                "c_call" => ast::DirectiveKind::CCall,
                "inline_never" => ast::DirectiveKind::InlineNever,
                "inline_always" => ast::DirectiveKind::InlineAlways,
                "intrinsic" => ast::DirectiveKind::Intrinsic,
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
            match ctx.intern.get(name.id) {
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
    ctx: &mut AstBuild<'ast, '_>,
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
    ast::DirectiveParam { name, value, value_range: lit_string.range() }
}

//==================== TYPE ====================

fn ty<'ast>(ctx: &mut AstBuild<'ast, '_>, ty_cst: cst::Type) -> ast::Type<'ast> {
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
    ast::Type { kind, range: ty_cst.range() }
}

fn proc_type_param<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
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

//==================== STMT ====================

fn block<'ast>(ctx: &mut AstBuild<'ast, '_>, block: cst::Block) -> ast::Block<'ast> {
    let offset = ctx.s.stmts.start();
    for stmt_cst in block.stmts(ctx.tree) {
        let stmt = stmt(ctx, stmt_cst);
        ctx.s.stmts.push(stmt);
    }
    let stmts = ctx.s.stmts.take(offset, &mut ctx.arena);
    ast::Block { stmts, range: block.range() }
}

fn stmt<'ast>(ctx: &mut AstBuild<'ast, '_>, stmt_cst: cst::Stmt) -> ast::Stmt<'ast> {
    let kind = match stmt_cst {
        cst::Stmt::Break(_) => ast::StmtKind::Break,
        cst::Stmt::Continue(_) => ast::StmtKind::Continue,
        cst::Stmt::Return(ret) => ast::StmtKind::Return(ret.expr(ctx.tree).map(|e| expr(ctx, e))),
        cst::Stmt::Defer(defer) => ast::StmtKind::Defer(stmt_defer(ctx, defer)),
        cst::Stmt::For(for_) => ast::StmtKind::For(stmt_for(ctx, for_)),
        cst::Stmt::Local(local) => ast::StmtKind::Local(stmt_local(ctx, local)),
        cst::Stmt::Assign(assign) => ast::StmtKind::Assign(stmt_assign(ctx, assign)),
        cst::Stmt::ExprSemi(semi) => {
            ast::StmtKind::ExprSemi(expr(ctx, semi.expr(ctx.tree).unwrap()))
        }
        cst::Stmt::ExprTail(tail) => {
            ast::StmtKind::ExprTail(expr(ctx, tail.expr(ctx.tree).unwrap()))
        }
        cst::Stmt::WithDirective(stmt_dir) => {
            ast::StmtKind::WithDirective(stmt_with_directive(ctx, stmt_dir))
        }
    };
    ast::Stmt { kind, range: stmt_cst.range() }
}

fn stmt_defer<'ast>(ctx: &mut AstBuild<'ast, '_>, defer: cst::StmtDefer) -> &'ast ast::Block<'ast> {
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

fn stmt_for<'ast>(ctx: &mut AstBuild<'ast, '_>, for_: cst::StmtFor) -> &'ast ast::For<'ast> {
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

fn stmt_local<'ast>(ctx: &mut AstBuild<'ast, '_>, local: cst::StmtLocal) -> &'ast ast::Local<'ast> {
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
    ctx: &mut AstBuild<'ast, '_>,
    assign: cst::StmtAssign,
) -> &'ast ast::Assign<'ast> {
    let (op, op_range) = assign.assign_op(ctx.tree).unwrap();
    let lhs = expr(ctx, assign.lhs(ctx.tree).unwrap());
    let rhs = expr(ctx, assign.rhs(ctx.tree).unwrap());

    let assign = ast::Assign { op, op_range, lhs, rhs };
    ctx.arena.alloc(assign)
}

fn stmt_with_directive<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    stmt_dir: cst::StmtWithDirective,
) -> &'ast ast::StmtWithDirective<'ast> {
    let dir_list = directive_list_opt(ctx, stmt_dir.dir_list(ctx.tree));
    let stmt = stmt(ctx, stmt_dir.stmt(ctx.tree).unwrap());
    let stmt = ast::StmtWithDirective { dir_list, stmt };
    ctx.arena.alloc(stmt)
}

//==================== EXPR ====================

fn expr<'ast>(ctx: &mut AstBuild<'ast, '_>, expr_cst: cst::Expr) -> &'ast ast::Expr<'ast> {
    let kind = expr_kind(ctx, expr_cst);
    let expr = ast::Expr { kind, range: expr_cst.range() };
    ctx.arena.alloc(expr)
}

fn expr_kind<'ast>(ctx: &mut AstBuild<'ast, '_>, expr_cst: cst::Expr) -> ast::ExprKind<'ast> {
    match expr_cst {
        cst::Expr::Paren(paren) => expr_kind(ctx, paren.expr(ctx.tree).unwrap()),
        cst::Expr::Lit(lit_cst) => ast::ExprKind::Lit { lit: lit(ctx, lit_cst) },
        cst::Expr::If(if_) => expr_if(ctx, if_),
        cst::Expr::Block(block_cst) => {
            let block = block(ctx, block_cst);
            ast::ExprKind::Block { block: ctx.arena.alloc(block) }
        }
        cst::Expr::Match(match_) => {
            let on_expr = expr(ctx, match_.on_expr(ctx.tree).unwrap());
            let arms = match_arm_list(ctx, match_.match_arm_list(ctx.tree).unwrap());
            let match_ = ctx.arena.alloc(ast::Match { on_expr, arms });
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
        cst::Expr::Slice(slice) => expr_slice(ctx, slice),
        cst::Expr::Call(call) => {
            let target = expr(ctx, call.target(ctx.tree).unwrap());
            let args_list = args_list(ctx, call.args_list(ctx.tree).unwrap());
            ast::ExprKind::Call { target, args_list }
        }
        cst::Expr::Cast(cast) => {
            let target = expr(ctx, cast.target(ctx.tree).unwrap());
            let into = ty(ctx, cast.into_ty(ctx.tree).unwrap());
            ast::ExprKind::Cast { target, into: ctx.arena.alloc(into) }
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
        cst::Expr::StructInit(struct_init) => expr_struct_init(ctx, struct_init),
        cst::Expr::ArrayInit(array_init) => expr_array_init(ctx, array_init),
        cst::Expr::ArrayRepeat(array_repeat) => {
            let value = expr(ctx, array_repeat.value(ctx.tree).unwrap().expr(ctx.tree).unwrap());
            let len = ast::ConstExpr(expr(ctx, array_repeat.len(ctx.tree).unwrap()));
            ast::ExprKind::ArrayRepeat { value, len }
        }
        cst::Expr::Deref(deref) => {
            let rhs = expr(ctx, deref.expr(ctx.tree).unwrap());
            ast::ExprKind::Deref { rhs }
        }
        cst::Expr::Address(address) => {
            let mutt = mutt(address.t_mut(ctx.tree));
            let rhs = expr(ctx, address.expr(ctx.tree).unwrap());
            ast::ExprKind::Address { mutt, rhs }
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

fn expr_struct_init<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    struct_init: cst::ExprStructInit,
) -> ast::ExprKind<'ast> {
    let path = struct_init.path(ctx.tree).map(|p| path(ctx, p));
    let offset = ctx.s.field_inits.start();
    let field_init_list = struct_init.field_init_list(ctx.tree).unwrap();

    for field_init_cst in field_init_list.field_inits(ctx.tree) {
        let name = name(ctx, field_init_cst.name(ctx.tree).unwrap());
        let expr = if let Some(expr_cst) = field_init_cst.expr(ctx.tree) {
            expr(ctx, expr_cst)
        } else {
            let segment = ast::PathSegment { name, poly_args: None };
            let segments = ctx.arena.alloc_slice(&[segment]);
            let path = ctx.arena.alloc(ast::Path { segments });
            let kind = ast::ExprKind::Item { path, args_list: None };
            ctx.arena.alloc(ast::Expr { kind, range: name.range })
        };
        let field_init = ast::FieldInit { name, expr };
        ctx.s.field_inits.push(field_init);
    }
    let input = ctx.s.field_inits.take(offset, &mut ctx.arena);

    let input_start = field_init_list.range().start() + 1.into();
    let struct_init = ast::StructInit { path, input, input_start };
    ast::ExprKind::StructInit { struct_init: ctx.arena.alloc(struct_init) }
}

fn expr_array_init<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    array_init: cst::ExprArrayInit,
) -> ast::ExprKind<'ast> {
    let offset = ctx.s.array_inits.start();
    for init in array_init.input(ctx.tree) {
        let init = if init.t_equals(ctx.tree).is_some() {
            ast::ArrayInit {
                variant: Some(ast::ConstExpr(expr(ctx, init.variant(ctx.tree).unwrap()))),
                expr: expr(ctx, init.variant_expr(ctx.tree).unwrap()),
            }
        } else {
            ast::ArrayInit { variant: None, expr: expr(ctx, init.expr(ctx.tree).unwrap()) }
        };
        ctx.s.array_inits.push(init);
    }
    let input = ctx.s.array_inits.take(offset, &mut ctx.arena);
    ast::ExprKind::ArrayInit { input }
}

fn expr_if<'ast>(ctx: &mut AstBuild<'ast, '_>, if_: cst::ExprIf) -> ast::ExprKind<'ast> {
    let offset = ctx.s.branches.start();
    for branch in if_.branches(ctx.tree) {
        let (kind, block) = match branch {
            cst::Branch::Cond(branch) => {
                let cond = expr(ctx, branch.cond(ctx.tree).unwrap());
                let block = block(ctx, branch.block(ctx.tree).unwrap());
                (ast::BranchKind::Cond(cond), block)
            }
            cst::Branch::Pat(branch) => {
                let pat = pat(ctx, branch.pat(ctx.tree).unwrap());
                let pat = ctx.arena.alloc(pat);
                let expr = expr(ctx, branch.expr(ctx.tree).unwrap());
                let block = block(ctx, branch.block(ctx.tree).unwrap());
                (ast::BranchKind::Pat(pat, expr), block)
            }
        };
        let branch = ast::Branch { kind, block };
        ctx.s.branches.push(branch);
    }
    let branches = ctx.s.branches.take(offset, &mut ctx.arena);
    let else_block = if_.else_block(ctx.tree).map(|b| block(ctx, b));

    let if_ = ctx.arena.alloc(ast::If { branches, else_block });
    ast::ExprKind::If { if_ }
}

fn match_arm_list<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    match_arm_list: cst::MatchArmList,
) -> &'ast [ast::MatchArm<'ast>] {
    let offset = ctx.s.match_arms.start();
    for arm in match_arm_list.match_arms(ctx.tree) {
        let arm = match_arm(ctx, arm);
        ctx.s.match_arms.push(arm)
    }
    ctx.s.match_arms.take(offset, &mut ctx.arena)
}

fn match_arm<'ast>(ctx: &mut AstBuild<'ast, '_>, arm: cst::MatchArm) -> ast::MatchArm<'ast> {
    let pat = pat(ctx, arm.pat(ctx.tree).unwrap());
    let stmt = stmt(ctx, arm.stmt(ctx.tree).unwrap());
    let range = stmt.range;
    let block = ast::Block { stmts: ctx.arena.alloc_slice(&[stmt]), range };
    let expr = ast::Expr { kind: ast::ExprKind::Block { block: ctx.arena.alloc(block) }, range };
    ast::MatchArm { pat, expr: ctx.arena.alloc(expr) }
}

fn expr_slice<'ast>(ctx: &mut AstBuild<'ast, '_>, slice: cst::ExprSlice) -> ast::ExprKind<'ast> {
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

//==================== PAT ====================

fn pat<'ast>(ctx: &mut AstBuild<'ast, '_>, pat_cst: cst::Pat) -> ast::Pat<'ast> {
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
    ast::Pat { kind, range: pat_cst.range() }
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

fn string_lit(ctx: &mut AstBuild, lit: cst::LitString) -> LitID {
    let token_id = lit.t_string_lit_id(ctx.tree).unwrap();
    ctx.tree.tokens().string(token_id)
}

//==================== COMMON ====================

fn name(ctx: &mut AstBuild, name: cst::Name) -> ast::Name {
    let range = name.range();
    let string = &ctx.source[range.as_usize()];
    let id = ctx.intern.intern(string);
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
    ctx: &mut AstBuild<'ast, '_>,
    bind_list: cst::BindList,
) -> &'ast ast::BindingList<'ast> {
    let offset = ctx.s.binds.start();
    for bind_cst in bind_list.binds(ctx.tree) {
        let bind = bind(ctx, bind_cst);
        ctx.s.binds.push(bind);
    }
    let binds = ctx.s.binds.take(offset, &mut ctx.arena);

    let bind_list = ast::BindingList { binds, range: bind_list.range() };
    ctx.arena.alloc(bind_list)
}

fn args_list<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    args_list: cst::ArgsList,
) -> &'ast ast::ArgumentList<'ast> {
    let args_list = args_list_value(ctx, args_list);
    ctx.arena.alloc(args_list)
}

fn args_list_value<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    args_list: cst::ArgsList,
) -> ast::ArgumentList<'ast> {
    let offset = ctx.s.exprs.start();
    for expr_cst in args_list.exprs(ctx.tree) {
        let expr = expr(ctx, expr_cst);
        ctx.s.exprs.push(expr);
    }
    let exprs = ctx.s.exprs.take(offset, &mut ctx.arena);
    ast::ArgumentList { exprs, range: args_list.range() }
}

fn path<'ast>(ctx: &mut AstBuild<'ast, '_>, path: cst::Path) -> &'ast ast::Path<'ast> {
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
    ctx: &mut AstBuild<'ast, '_>,
    segment: cst::PathSegment,
) -> ast::PathSegment<'ast> {
    let name = name(ctx, segment.name(ctx.tree).unwrap());
    let poly_args = segment.poly_args(ctx.tree).map(|pa| polymorph_args(ctx, pa));
    ast::PathSegment { name, poly_args }
}

fn polymorph_args<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    poly_args: cst::PolymorphArgs,
) -> &'ast ast::PolymorphArgs<'ast> {
    let offset = ctx.s.types.start();
    for ty_cst in poly_args.types(ctx.tree) {
        let ty = ty(ctx, ty_cst);
        ctx.s.types.push(ty);
    }
    let types = ctx.s.types.take(offset, &mut ctx.arena);

    let poly_args = ast::PolymorphArgs { types, range: poly_args.range() };
    ctx.arena.alloc(poly_args)
}

fn polymorph_params<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    poly_params: cst::PolymorphParams,
) -> &'ast ast::PolymorphParams<'ast> {
    let offset = ctx.s.names.start();
    for name_cst in poly_params.names(ctx.tree) {
        let name = name(ctx, name_cst);
        ctx.s.names.push(name);
    }
    let names = ctx.s.names.take(offset, &mut ctx.arena);

    let params = ast::PolymorphParams { names, range: poly_params.range() };
    ctx.arena.alloc(params)
}
