use super::ast_layer as cst;
use super::syntax_tree::SyntaxTree;
use crate::arena::Arena;
use crate::ast;
use crate::error::{DiagnosticCollection, ErrorComp, ResultComp, SourceRange};
use crate::intern::InternPool;
use crate::session::{FileID, Session};
use crate::temp_buffer::TempBuffer;
use crate::timer::Timer;

//@rename some ast:: nodes to match ast_layer and syntax names (eg: ProcParam)
struct AstBuild<'ast, 'syn> {
    tree: &'syn SyntaxTree<'syn>,
    char_id: u32,
    string_id: u32,
    file_id: FileID,
    source: &'ast str, //@hack same as ast lifetime

    arena: Arena<'ast>,
    intern_name: InternPool<'ast>,   //@hack same as ast lifetime
    intern_string: InternPool<'ast>, //@hack same as ast lifetime
    string_is_cstr: Vec<bool>,
    packages: Vec<ast::Package<'ast>>,
    errors: Vec<ErrorComp>,

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

impl<'ast, 'syn> AstBuild<'ast, 'syn> {
    fn new(tree: &'syn SyntaxTree<'syn>) -> AstBuild<'ast, 'syn> {
        AstBuild {
            tree,
            char_id: 0,
            string_id: 0,
            file_id: FileID::dummy(),
            source: "",

            arena: Arena::new(),
            intern_name: InternPool::new(),
            intern_string: InternPool::new(),
            string_is_cstr: Vec::with_capacity(128),
            packages: Vec::new(),
            errors: Vec::new(),

            items: TempBuffer::new(128),
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

pub fn parse(session: &Session) -> ResultComp<ast::Ast> {
    let t_total = Timer::new();
    let (tree_dummy, _) = super::parse("", FileID::dummy());
    let mut ctx = AstBuild::new(&tree_dummy);
    let mut file_idx: usize = 0;

    for package_id in session.package_ids() {
        let package = session.package(package_id);
        let package_name_id = ctx.intern_name.intern(&package.manifest().package.name);
        let mut modules = Vec::<ast::Module>::new();

        for idx in file_idx..file_idx + package.file_count() {
            let file_id = FileID::new(idx);
            let file = session.file(file_id);
            let filename = file
                .path
                .file_stem()
                .expect("filename")
                .to_str()
                .expect("utf-8");
            let module_name_id = ctx.intern_name.intern(filename);

            let (tree, errors) = super::parse(&file.source, file_id);
            if errors.is_empty() {
                let items = source_file(&mut ctx, tree.source_file());
                let module = ast::Module {
                    file_id,
                    name_id: module_name_id,
                    items,
                };
                modules.push(module);
            } else {
                ctx.errors.extend(errors);
            }
        }

        file_idx += package.file_count();
        ctx.packages.push(ast::Package {
            name_id: package_name_id,
            modules,
        });
    }

    t_total.stop("ast parse (new) total");
    if ctx.errors.is_empty() {
        let ast = ast::Ast {
            arena: ctx.arena,
            intern_name: ctx.intern_name,
            intern_string: ctx.intern_string,
            string_is_cstr: ctx.string_is_cstr,
            packages: ctx.packages,
        };
        ResultComp::Ok((ast, vec![]))
    } else {
        ResultComp::Err(DiagnosticCollection::new().join_errors(ctx.errors))
    }
}

fn source_file<'ast>(
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

fn stmt_defer<'ast>(ctx: &mut AstBuild<'ast, '_>, defer: cst::StmtDefer) -> &'ast ast::Block<'ast> {
    if let Some(short_block) = defer.short_block(ctx.tree) {
        let stmt_cst = short_block.stmt(ctx.tree).unwrap();
        let stmt = stmt(ctx, stmt_cst);

        let offset = ctx.stmts.start();
        ctx.stmts.add(stmt);
        let stmts = ctx.stmts.take(offset, &mut ctx.arena);

        let block = ast::Block {
            stmts,
            range: stmt.range,
        };
        ctx.arena.alloc(block)
    } else {
        let block_cst = defer.block(ctx.tree).unwrap();
        let block = block(ctx, block_cst);
        ctx.arena.alloc(block)
    }
}

fn stmt_loop<'ast>(ctx: &mut AstBuild<'ast, '_>, loop_: cst::StmtLoop) -> &'ast ast::Loop<'ast> {
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

fn stmt_local<'ast>(ctx: &mut AstBuild<'ast, '_>, local: cst::StmtLocal) -> &'ast ast::Local<'ast> {
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
    ctx.arena.alloc(local)
}

fn stmt_assign<'ast>(
    ctx: &mut AstBuild<'ast, '_>,
    assign: cst::StmtAssign,
) -> &'ast ast::Assign<'ast> {
    let op = assign.assign_op(ctx.tree);
    //@api for getting range of ops etc

    let mut lhs_rhs_iter: cst::AstNodeIterator<cst::Expr> = assign.lhs_rhs_iter(ctx.tree);
    let lhs = expr(ctx, lhs_rhs_iter.next().unwrap());
    let rhs = expr(ctx, lhs_rhs_iter.next().unwrap());
    todo!()
}

//@rename expr_cst back to expr when each arm has a function
fn expr<'ast>(ctx: &mut AstBuild<'ast, '_>, expr_cst: cst::Expr) -> &'ast ast::Expr<'ast> {
    let range = expr_cst.range(ctx.tree);

    let kind = match expr_cst {
        cst::Expr::Paren(paren) => {
            //@problem with range and inner expr
            todo!();
        }
        cst::Expr::LitNull(_) => ast::ExprKind::LitNull,
        cst::Expr::LitBool(lit) => {
            //@add get on actual token type
            let val = false;

            ast::ExprKind::LitBool { val }
        }
        cst::Expr::LitInt(lit) => {
            //@assuming that range of Node == range of Token
            let range = lit.range(ctx.tree);
            let string = &ctx.source[range.as_usize()];

            let val = match string.parse::<u64>() {
                Ok(value) => value,
                Err(error) => {
                    ctx.errors.push(ErrorComp::new(
                        format!("parse integer error: {}", error),
                        SourceRange::new(range, ctx.file_id),
                        None,
                    ));
                    0
                }
            };

            ast::ExprKind::LitInt { val }
        }
        cst::Expr::LitFloat(lit) => {
            //@assuming that range of Node == range of Token
            let range = lit.range(ctx.tree);
            let string = &ctx.source[range.as_usize()];

            let val = match string.parse::<f64>() {
                Ok(value) => value,
                Err(error) => {
                    ctx.errors.push(ErrorComp::new(
                        format!("parse float error: {}", error),
                        SourceRange::new(range, ctx.file_id),
                        None,
                    ));
                    0.0
                }
            };

            ast::ExprKind::LitFloat { val }
        }
        cst::Expr::LitChar(_) => {
            let val = ctx.tree.tokens().get_char(ctx.char_id as usize);
            ctx.char_id += 1;

            ast::ExprKind::LitChar { val }
        }
        cst::Expr::LitString(_) => {
            let (string, c_string) = ctx.tree.tokens().get_string(ctx.string_id as usize);
            let id = ctx.intern_string.intern(string);
            ctx.string_id += 1;

            if id.index() >= ctx.string_is_cstr.len() {
                ctx.string_is_cstr.push(c_string);
            } else if c_string {
                ctx.string_is_cstr[id.index()] = true;
            }

            ast::ExprKind::LitString { id, c_string }
        }
        cst::Expr::If(if_) => todo!(),
        cst::Expr::Block(expr_block) => {
            let block = block(ctx, expr_block.into_block());

            let block = ctx.arena.alloc(block);
            ast::ExprKind::Block { block }
        }
        cst::Expr::Match(match_) => todo!(),
        cst::Expr::Field(field) => {
            let target = expr(ctx, field.target(ctx.tree).unwrap());
            let name = name(ctx, field.name(ctx.tree).unwrap());

            ast::ExprKind::Field { target, name }
        }
        cst::Expr::Index(index) => todo!(), //@syntax / api imcomplete
        cst::Expr::Call(call) => {
            let target = expr(ctx, call.target(ctx.tree).unwrap());

            let offset = ctx.exprs.start();
            let argument_list = call.call_argument_list(ctx.tree).unwrap();
            for input in argument_list.inputs(ctx.tree) {
                let expr = expr(ctx, input);
                ctx.exprs.add(expr);
            }
            let input = ctx.exprs.take(offset, &mut ctx.arena);

            let input = ctx.arena.alloc(input);
            ast::ExprKind::Call { target, input }
        }
        cst::Expr::Cast(cast) => {
            let target = expr(ctx, cast.target(ctx.tree).unwrap());
            let into = ty(ctx, cast.into_ty(ctx.tree).unwrap());

            let into = ctx.arena.alloc(into);
            ast::ExprKind::Cast { target, into }
        }
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
        cst::Expr::StructInit(struct_init) => {
            //@optional in grammar, unsupported in ast
            let path = path(ctx, struct_init.path(ctx.tree).unwrap());

            let offset = ctx.field_inits.start();
            let field_init_list = struct_init.field_init_list(ctx.tree).unwrap();
            for field_init in field_init_list.field_inits(ctx.tree) {
                //@change design, expr is stored always instead of name always and opt expr
            }
            let input = ctx.field_inits.take(offset, &mut ctx.arena);

            let struct_init = ast::StructInit { path, input };
            let struct_init = ctx.arena.alloc(struct_init);
            ast::ExprKind::StructInit { struct_init }
        }
        cst::Expr::ArrayInit(array_init) => {
            let offset = ctx.exprs.start();
            for input in array_init.inputs(ctx.tree) {
                let expr = expr(ctx, input);
                ctx.exprs.add(expr);
            }
            let input = ctx.exprs.take(offset, &mut ctx.arena);

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
