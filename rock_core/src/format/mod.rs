use crate::ast::{AssignOp, BinOp};
use crate::error::ErrorComp;
use crate::session::ModuleID;
use crate::syntax;
use crate::syntax::ast_layer as ast;
use crate::syntax::syntax_tree::SyntaxTree;
use crate::text::TextRange;

//@conform to ast_build style:
// _cst postfix in cst nodes with name conflits,
// remove _fmt postfix from functions
//@use session?
pub fn format(source: &str, module_id: ModuleID) -> Result<String, Vec<ErrorComp>> {
    let (tree, errors) = syntax::parse(source, module_id, true);
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut fmt = Formatter::new(&tree, source);
    source_file(&mut fmt, tree.source_file());
    Ok(fmt.finish())
}

//@remove, test on real code
#[test]
fn format_test() {
    let source = r#"
    #[inline]    pub 
    proc 
    main (
        x : s32 , y : s32  ) ->  u32 ;

    proc p_empty( ) { break; return 5; }
    proc p_var (..);
    proc p_var_a (x: s32, ..);
    proc p_arg (x: s32,);
    proc p_args (x: s32, y: s32,);

    struct Empty {}
    struct Vec1 {x: f32}
    struct Vec2 {x: f32, y: f32}

    enum Empty {}
    enum KindSingle { Variant = 5 }
    pub enum KindDouble { Variant, Variant2 = 10 }

    const V: s32 = 5;
    global mut ValG: s32 = 10;
    global ValG2: s.SomeType = 20;

    import s;
    import s/g;
    import s/g as smth;
    import s.{ };
    import s/g as smth.{mem, mem2 as other,};

    struct TypeTest {
        f: u32,
        f: Custom.Type,
        f: & u32,
        f: &mut u32,
        f: proc(u32, s32, ..) -> bool,
        f: []G,
        f: [mut]G,
        f: [10][50]Custom.Type,
    }

    const STRUCT: Vec2 = (&mut G.name   (1, 3, 4,));

    proc stuff() {
        let x: s32;
        let x: s32 = 2 +    3;
        let x=(  
        
        15  
        - 8 
        ) ..< (55);
        x.field = y - g * 5;
        defer {
            let x = 10;
        }
    }
    "#;

    if let Ok(formatted) = format(source, ModuleID::dummy()) {
        println!("ORIGINAL SOURCE:\n-----\n{}-----", source);
        println!("FORMATTED SOURCE:\n-----\n{}-----", formatted);
    } else {
        panic!("incorrect source syntax");
    }
}

struct Formatter<'syn> {
    tree: &'syn SyntaxTree<'syn>,
    source: &'syn str,
    tab_depth: u32,
    buffer: String,
}

impl<'syn> Formatter<'syn> {
    fn new(tree: &'syn SyntaxTree<'syn>, source: &'syn str) -> Formatter<'syn> {
        Formatter {
            tree,
            source,
            tab_depth: 0,
            buffer: String::with_capacity(source.len()),
        }
    }

    fn finish(self) -> String {
        self.buffer
    }

    fn write(&mut self, string: &str) {
        self.buffer.push_str(string);
    }

    fn write_c(&mut self, c: char) {
        self.buffer.push(c);
    }

    fn write_range(&mut self, range: TextRange) {
        let string = &self.source[range.as_usize()];
        self.write(string);
    }

    fn space(&mut self) {
        self.buffer.push(' ');
    }

    fn new_line(&mut self) {
        self.buffer.push('\n');
    }

    fn tab(&mut self) {
        self.write("    ");
    }

    fn tab_depth(&mut self) {
        for _ in 0..self.tab_depth {
            self.tab();
        }
    }

    fn depth_increment(&mut self) {
        self.tab_depth += 1;
    }

    fn depth_decrement(&mut self) {
        self.tab_depth -= 1;
    }
}

fn source_file(fmt: &mut Formatter, source_file: ast::SourceFile) {
    for item in source_file.items(fmt.tree) {
        item_fmt(fmt, item);
    }
}

fn item_fmt(fmt: &mut Formatter, item: ast::Item) {
    fmt.new_line();
    match item {
        ast::Item::Proc(item) => proc_item(fmt, item),
        ast::Item::Enum(item) => enum_item(fmt, item),
        ast::Item::Struct(item) => struct_item(fmt, item),
        ast::Item::Const(item) => const_item(fmt, item),
        ast::Item::Global(item) => global_item(fmt, item),
        ast::Item::Import(item) => import_item(fmt, item),
    }
}

fn attribute(fmt: &mut Formatter, attr: ast::Attribute) {
    fmt.write("#[");
    name_fmt(fmt, attr.name(fmt.tree).unwrap());
    fmt.write_c(']');
    fmt.new_line();
}

fn visibility(fmt: &mut Formatter, vis: ast::Visibility) {
    if vis.is_pub(fmt.tree) {
        fmt.write("pub");
        fmt.space();
    }
}

macro_rules! item_attr_vis_fmt {
    ($fmt:ident, $item:expr) => {
        if let Some(attr) = $item.attribute($fmt.tree) {
            attribute($fmt, attr);
        }
        if let Some(vis) = $item.visiblity($fmt.tree) {
            visibility($fmt, vis);
        }
    };
}

fn proc_item(fmt: &mut Formatter, item: ast::ProcItem) {
    item_attr_vis_fmt!(fmt, item);
    fmt.write("proc");
    fmt.space();
    name_fmt(fmt, item.name(fmt.tree).unwrap());
    param_list(fmt, item.param_list(fmt.tree).unwrap());
    if let Some(ty) = item.return_ty(fmt.tree) {
        fmt.space();
        fmt.write("->");
        fmt.space();
        type_fmt(fmt, ty);
    }
    if let Some(block) = item.block(fmt.tree) {
        fmt.space();
        block_fmt(fmt, block);
    } else {
        fmt.write_c(';');
    }
    fmt.new_line();
}

fn param_list(fmt: &mut Formatter, param_list: ast::ParamList) {
    fmt.write_c('(');
    let mut first = true;
    for param in param_list.params(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        first = false;
        param_fmt(fmt, param);
    }
    if param_list.is_variadic(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        fmt.write("..");
    }
    fmt.write_c(')');
}

fn param_fmt(fmt: &mut Formatter, param: ast::Param) {
    if param.is_mut(fmt.tree) {
        fmt.write("mut");
        fmt.space();
    }
    name_fmt(fmt, param.name(fmt.tree).unwrap());
    fmt.write_c(':');
    fmt.space();
    type_fmt(fmt, param.ty(fmt.tree).unwrap());
}

fn enum_item(fmt: &mut Formatter, item: ast::EnumItem) {
    item_attr_vis_fmt!(fmt, item);
    fmt.write("enum");
    fmt.space();
    name_fmt(fmt, item.name(fmt.tree).unwrap());
    fmt.space();
    if let Some(ty) = item.type_basic(fmt.tree) {
        type_basic(fmt, ty);
        fmt.space();
    }
    variant_list(fmt, item.variant_list(fmt.tree).unwrap());
    fmt.new_line();
}

fn variant_list(fmt: &mut Formatter, variant_list: ast::VariantList) {
    fmt.write_c('{');
    let mut empty = true;
    for variant in variant_list.variants(fmt.tree) {
        if empty {
            fmt.new_line();
            empty = false;
        }
        fmt.tab();
        variant_fmt(fmt, variant);
        fmt.write_c(',');
        fmt.new_line();
    }
    fmt.write_c('}');
}

fn variant_fmt(fmt: &mut Formatter, variant: ast::Variant) {
    name_fmt(fmt, variant.name(fmt.tree).unwrap());
    if let Some(expr) = variant.value(fmt.tree) {
        fmt.space();
        fmt.write_c('=');
        fmt.space();
        expr_fmt(fmt, expr);
    }
}

fn struct_item(fmt: &mut Formatter, item: ast::StructItem) {
    item_attr_vis_fmt!(fmt, item);
    fmt.write("struct");
    fmt.space();
    name_fmt(fmt, item.name(fmt.tree).unwrap());
    fmt.space();
    field_list(fmt, item.field_list(fmt.tree).unwrap());
    fmt.new_line();
}

fn field_list(fmt: &mut Formatter, field_list: ast::FieldList) {
    fmt.write_c('{');
    let mut empty = true;
    for field in field_list.fields(fmt.tree) {
        if empty {
            fmt.new_line();
            empty = false;
        }
        fmt.tab();
        field_fmt(fmt, field);
        fmt.write_c(',');
        fmt.new_line();
    }
    fmt.write_c('}');
}

fn field_fmt(fmt: &mut Formatter, field: ast::Field) {
    name_fmt(fmt, field.name(fmt.tree).unwrap());
    fmt.write_c(':');
    fmt.space();
    type_fmt(fmt, field.ty(fmt.tree).unwrap());
}

fn const_item(fmt: &mut Formatter, item: ast::ConstItem) {
    item_attr_vis_fmt!(fmt, item);
    fmt.write("const");
    fmt.space();
    name_fmt(fmt, item.name(fmt.tree).unwrap());
    fmt.write_c(':');
    fmt.space();
    type_fmt(fmt, item.ty(fmt.tree).unwrap());
    fmt.space();
    fmt.write_c('=');
    fmt.space();
    expr_fmt(fmt, item.value(fmt.tree).unwrap());
    fmt.write_c(';');
    fmt.new_line();
}

fn global_item(fmt: &mut Formatter, item: ast::GlobalItem) {
    item_attr_vis_fmt!(fmt, item);
    fmt.write("global");
    fmt.space();
    if item.is_mut(fmt.tree) {
        fmt.write("mut");
        fmt.space();
    }
    name_fmt(fmt, item.name(fmt.tree).unwrap());
    fmt.write_c(':');
    fmt.space();
    type_fmt(fmt, item.ty(fmt.tree).unwrap());
    fmt.space();
    fmt.write_c('=');
    fmt.space();
    expr_fmt(fmt, item.value(fmt.tree).unwrap());
    fmt.write_c(';');
    fmt.new_line();
}

fn import_item(fmt: &mut Formatter, item: ast::ImportItem) {
    //@vis will still be added here
    item_attr_vis_fmt!(fmt, item);
    fmt.write("import");
    fmt.space();
    if let Some(name) = item.package(fmt.tree) {
        name_fmt(fmt, name);
        fmt.write_c(':');
    }
    import_path(fmt, item.import_path(fmt.tree).unwrap());
    if let Some(name_alias) = item.name_alias(fmt.tree) {
        name_alias_fmt(fmt, name_alias);
    }
    if let Some(symbol_list) = item.import_symbol_list(fmt.tree) {
        fmt.write_c('.');
        import_symbol_list(fmt, symbol_list);
    } else {
        fmt.write_c(';');
    }
    fmt.new_line();
}

fn import_path(fmt: &mut Formatter, import_path: ast::ImportPath) {
    let mut first = true;
    for name in import_path.names(fmt.tree) {
        if !first {
            fmt.write_c('/');
        }
        first = false;
        name_fmt(fmt, name);
    }
}

fn import_symbol_list(fmt: &mut Formatter, import_symbol_list: ast::ImportSymbolList) {
    fmt.write_c('{');
    let mut first = true;
    for import_symbol in import_symbol_list.import_symbols(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        first = false;
        import_symbol_fmt(fmt, import_symbol);
    }
    fmt.write_c('}');
}

fn import_symbol_fmt(fmt: &mut Formatter, import_symbol: ast::ImportSymbol) {
    if let Some(vis) = import_symbol.visiblity(fmt.tree) {
        visibility(fmt, vis);
    }
    name_fmt(fmt, import_symbol.name(fmt.tree).unwrap());
    if let Some(name_alias) = import_symbol.name_alias(fmt.tree) {
        name_alias_fmt(fmt, name_alias);
    }
}

fn name_alias_fmt(fmt: &mut Formatter, name_alias: ast::NameAlias) {
    fmt.space();
    fmt.write("as");
    fmt.space();
    name_fmt(fmt, name_alias.name(fmt.tree).unwrap());
}

fn name_fmt(fmt: &mut Formatter, name: ast::Name) {
    fmt.write_range(name.range(fmt.tree));
}

fn path_fmt(fmt: &mut Formatter, path: ast::Path) {
    let mut first = true;
    for name in path.names(fmt.tree) {
        if !first {
            fmt.write_c('.');
        }
        first = false;
        name_fmt(fmt, name);
    }
}

fn type_fmt(fmt: &mut Formatter, ty: ast::Type) {
    match ty {
        ast::Type::Basic(ty) => type_basic(fmt, ty),
        ast::Type::Custom(ty) => type_custom(fmt, ty),
        ast::Type::Reference(ty) => type_reference(fmt, ty),
        ast::Type::Procedure(ty) => type_procedure(fmt, ty),
        ast::Type::ArraySlice(ty) => type_array_slice(fmt, ty),
        ast::Type::ArrayStatic(ty) => type_array_static(fmt, ty),
    }
}

fn type_basic(fmt: &mut Formatter, ty: ast::TypeBasic) {
    let basic = ty.basic(fmt.tree);
    fmt.write(basic.as_str());
}

fn type_custom(fmt: &mut Formatter, ty: ast::TypeCustom) {
    path_fmt(fmt, ty.path(fmt.tree).unwrap());
}

fn type_reference(fmt: &mut Formatter, ty: ast::TypeReference) {
    fmt.write_c('&');
    if ty.is_mut(fmt.tree) {
        fmt.write("mut");
        fmt.space();
    }
    type_fmt(fmt, ty.ref_ty(fmt.tree).unwrap());
}

fn type_procedure(fmt: &mut Formatter, ty: ast::TypeProcedure) {
    fmt.write("proc");
    param_type_list(fmt, ty.param_type_list(fmt.tree).unwrap());
    if let Some(ty) = ty.return_ty(fmt.tree) {
        fmt.space();
        fmt.write("->");
        fmt.space();
        type_fmt(fmt, ty);
    }
}

fn param_type_list(fmt: &mut Formatter, param_type_list: ast::ParamTypeList) {
    fmt.write_c('(');
    let mut first = true;
    for ty in param_type_list.param_types(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        first = false;
        type_fmt(fmt, ty);
    }
    if param_type_list.is_variadic(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        fmt.write("..");
    }
    fmt.write_c(')');
}

fn type_array_slice(fmt: &mut Formatter, ty: ast::TypeArraySlice) {
    fmt.write_c('[');
    if ty.is_mut(fmt.tree) {
        fmt.write("mut");
    }
    fmt.write_c(']');
    type_fmt(fmt, ty.elem_ty(fmt.tree).unwrap());
}

fn type_array_static(fmt: &mut Formatter, ty: ast::TypeArrayStatic) {
    fmt.write_c('[');
    expr_fmt(fmt, ty.len(fmt.tree).unwrap());
    fmt.write_c(']');
    type_fmt(fmt, ty.elem_ty(fmt.tree).unwrap());
}

fn stmt_fmt(fmt: &mut Formatter, stmt: ast::Stmt) {
    match stmt {
        ast::Stmt::Break(_) => {
            fmt.write("break");
            fmt.write_c(';');
        }
        ast::Stmt::Continue(_) => {
            fmt.write("continue");
            fmt.write_c(';');
        }
        ast::Stmt::Return(stmt_return) => {
            fmt.write("return");
            if let Some(expr) = stmt_return.expr(fmt.tree) {
                fmt.space();
                expr_fmt(fmt, expr);
            }
            fmt.write_c(';');
        }
        ast::Stmt::Defer(defer) => {
            fmt.write("defer");
            fmt.space();
            if let Some(short_block) = defer.short_block(fmt.tree) {
                stmt_fmt(fmt, short_block.stmt(fmt.tree).unwrap());
            } else {
                block_fmt(fmt, defer.block(fmt.tree).unwrap());
            }
        }
        ast::Stmt::Loop(loop_) => stmt_loop(fmt, loop_),
        ast::Stmt::Local(local) => stmt_local(fmt, local),
        ast::Stmt::Assign(assign) => stmt_assign(fmt, assign, true),
        ast::Stmt::ExprSemi(expr_semi) => {
            let expr = expr_semi.expr(fmt.tree).unwrap();
            let semi = !matches!(
                expr,
                ast::Expr::If(_) | ast::Expr::Block(_) | ast::Expr::Match(_)
            );
            expr_fmt(fmt, expr);
            if semi {
                fmt.write_c(';');
            }
        }
        ast::Stmt::ExprTail(expr_tail) => {
            fmt.write("->");
            fmt.space();
            expr_fmt(fmt, expr_tail.expr(fmt.tree).unwrap());
            fmt.write_c(';');
        }
    }
}

fn stmt_loop(fmt: &mut Formatter, loop_: ast::StmtLoop) {
    fmt.write("for");
    fmt.space();

    if let Some(while_header) = loop_.while_header(fmt.tree) {
        expr_fmt(fmt, while_header.cond(fmt.tree).unwrap());
        fmt.space();
    } else if let Some(clike_header) = loop_.clike_header(fmt.tree) {
        stmt_local(fmt, clike_header.local(fmt.tree).unwrap());
        fmt.space();
        expr_fmt(fmt, clike_header.cond(fmt.tree).unwrap());
        fmt.write_c(';');
        fmt.space();
        stmt_assign(fmt, clike_header.assign(fmt.tree).unwrap(), false);
        fmt.space();
    }

    block_fmt(fmt, loop_.block(fmt.tree).unwrap());
}

fn stmt_local(fmt: &mut Formatter, local: ast::StmtLocal) {
    if local.is_mut(fmt.tree) {
        fmt.write("mut");
    } else {
        fmt.write("let");
    }
    fmt.space();
    name_fmt(fmt, local.name(fmt.tree).unwrap());

    if let Some(ty) = local.ty(fmt.tree) {
        fmt.write_c(':');
        fmt.space();
        type_fmt(fmt, ty);
        if let Some(expr) = local.expr(fmt.tree) {
            fmt.space();
            fmt.write_c('=');
            fmt.space();
            expr_fmt(fmt, expr);
        }
    } else {
        fmt.space();
        fmt.write_c('=');
        fmt.space();
        expr_fmt(fmt, local.expr(fmt.tree).unwrap());
    }
    fmt.write_c(';');
}

fn stmt_assign(fmt: &mut Formatter, assign: ast::StmtAssign, semi: bool) {
    let mut lhs_rhs = assign.lhs_rhs_iter(fmt.tree);
    expr_fmt(fmt, lhs_rhs.next().unwrap());

    fmt.space();
    let assign_op = assign.assign_op(fmt.tree).unwrap();
    match assign_op {
        AssignOp::Assign => {
            fmt.write_c('=');
        }
        AssignOp::Bin(op) => {
            fmt.write(op.as_str());
            fmt.write_c('=');
        }
    }
    fmt.space();

    expr_fmt(fmt, lhs_rhs.next().unwrap());
    if semi {
        fmt.write_c(';');
    }
}

fn expr_fmt(fmt: &mut Formatter, expr: ast::Expr) {
    match expr {
        ast::Expr::Paren(expr_paren) => {
            fmt.write_c('(');
            expr_fmt(fmt, expr_paren.expr(fmt.tree).unwrap());
            fmt.write_c(')');
        }
        ast::Expr::LitNull(_) => fmt.write("null"),
        //@assuming that lit.rage is single lit token
        ast::Expr::LitBool(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::LitInt(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::LitFloat(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::LitChar(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::LitString(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::If(if_) => expr_if(fmt, if_),
        ast::Expr::Block(block) => block_fmt(fmt, block.into_block()),
        ast::Expr::Match(match_) => expr_match(fmt, match_),
        ast::Expr::Field(field) => {
            expr_fmt(fmt, field.target(fmt.tree).unwrap());
            fmt.write_c('.');
            name_fmt(fmt, field.name(fmt.tree).unwrap());
        }
        ast::Expr::Index(index) => expr_index(fmt, index),
        ast::Expr::Call(call) => expr_call(fmt, call),
        ast::Expr::Cast(cast) => {
            expr_fmt(fmt, cast.target(fmt.tree).unwrap());
            fmt.space();
            fmt.write("as");
            fmt.space();
            type_fmt(fmt, cast.into_ty(fmt.tree).unwrap());
        }
        ast::Expr::Sizeof(sizeof) => {
            fmt.write("sizeof");
            fmt.write_c('(');
            type_fmt(fmt, sizeof.ty(fmt.tree).unwrap());
            fmt.write_c(')');
        }
        ast::Expr::Item(item) => {
            path_fmt(fmt, item.path(fmt.tree).unwrap());
        }
        ast::Expr::Variant(variant) => {
            fmt.write_c('.');
            name_fmt(fmt, variant.name(fmt.tree).unwrap());
        }
        ast::Expr::StructInit(struct_init) => expr_struct_init(fmt, struct_init),
        ast::Expr::ArrayInit(array_init) => expr_array_init(fmt, array_init),
        ast::Expr::ArrayRepeat(array_repeat) => expr_array_repeat(fmt, array_repeat),
        ast::Expr::Deref(deref) => {
            fmt.write_c('*');
            expr_fmt(fmt, deref.expr(fmt.tree).unwrap());
        }
        ast::Expr::Address(address) => {
            fmt.write_c('&');
            if address.is_mut(fmt.tree) {
                fmt.write("mut");
                fmt.space();
            }
            expr_fmt(fmt, address.expr(fmt.tree).unwrap());
        }
        ast::Expr::Unary(unary) => {
            let op = unary.un_op(fmt.tree);
            fmt.write(op.as_str());
            expr_fmt(fmt, unary.rhs(fmt.tree).unwrap());
        }
        ast::Expr::Binary(binary) => {
            let mut lhs_rhs = binary.lhs_rhs_iter(fmt.tree);
            expr_fmt(fmt, lhs_rhs.next().unwrap());

            let op = binary.bin_op(fmt.tree);
            if matches!(op, BinOp::Range | BinOp::RangeInc) {
                fmt.write(op.as_str());
            } else {
                fmt.space();
                fmt.write(op.as_str());
                fmt.space();
            }

            expr_fmt(fmt, lhs_rhs.next().unwrap());
        }
    }
}

fn expr_if(fmt: &mut Formatter, if_: ast::ExprIf) {
    let entry = if_.entry_branch(fmt.tree).unwrap();
    fmt.write("if");
    fmt.space();
    expr_fmt(fmt, entry.cond(fmt.tree).unwrap());
    fmt.space();
    block_fmt(fmt, entry.block(fmt.tree).unwrap());

    for branch in if_.else_if_branches(fmt.tree) {
        fmt.space();
        fmt.write("else");
        fmt.space();
        fmt.write("if");
        fmt.space();
        expr_fmt(fmt, branch.cond(fmt.tree).unwrap());
        fmt.space();
        block_fmt(fmt, branch.block(fmt.tree).unwrap());
    }

    if let Some(block) = if_.else_block(fmt.tree) {
        fmt.space();
        fmt.write("else");
        fmt.space();
        block_fmt(fmt, block);
    }
}

fn block_fmt(fmt: &mut Formatter, block: ast::Block) {
    fmt.write_c('{');
    fmt.depth_increment();

    let mut empty = true;
    for stmt in block.stmts(fmt.tree) {
        if empty {
            fmt.new_line();
            empty = false;
        }
        fmt.tab_depth();
        stmt_fmt(fmt, stmt);
        fmt.new_line();
    }

    fmt.depth_decrement();
    if !empty {
        fmt.tab_depth();
    }
    fmt.write_c('}');
}

fn expr_match(fmt: &mut Formatter, match_: ast::ExprMatch) {
    fmt.write("match");
    fmt.space();
    expr_fmt(fmt, match_.on_expr(fmt.tree).unwrap());
    fmt.space();
    fmt.write_c('{');
    fmt.depth_increment();

    let mut empty = true;
    let match_arm_list = match_.match_arm_list(fmt.tree).unwrap();
    for match_arm in match_arm_list.match_arms(fmt.tree) {
        if empty {
            fmt.new_line();
            empty = false;
        }
        let mut pat_expr_iter = match_arm.pat_expr_iter(fmt.tree);
        fmt.tab_depth();
        expr_fmt(fmt, pat_expr_iter.next().unwrap());
        fmt.space();
        fmt.write("->");
        fmt.space();
        expr_fmt(fmt, pat_expr_iter.next().unwrap());
        fmt.write_c(',');
        fmt.new_line();
    }

    if let Some(fallback) = match_arm_list.fallback(fmt.tree) {
        if empty {
            fmt.new_line();
            empty = false;
        }
        fmt.tab_depth();
        fmt.write_c('_');
        fmt.space();
        fmt.write("->");
        fmt.space();
        expr_fmt(fmt, fallback.expr(fmt.tree).unwrap());
        fmt.write_c(',');
        fmt.new_line();
    }

    fmt.depth_decrement();
    if !empty {
        fmt.tab_depth();
    }
    fmt.write_c('}');
}

fn expr_index(fmt: &mut Formatter, index: ast::ExprIndex) {
    expr_fmt(fmt, index.target(fmt.tree).unwrap());
    fmt.write_c('[');
    if index.is_mut(fmt.tree) {
        fmt.write("mut");
        fmt.space();
    }
    //@todo no slicing / index expr api
    fmt.write("<@index_or_slice>");
    fmt.write_c(']');
}

fn expr_call(fmt: &mut Formatter, call: ast::ExprCall) {
    expr_fmt(fmt, call.target(fmt.tree).unwrap());
    fmt.write_c('(');
    let mut first = true;
    let call_argument_list = call.call_argument_list(fmt.tree).unwrap();
    for expr in call_argument_list.inputs(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        first = false;
        expr_fmt(fmt, expr);
    }
    fmt.write_c(')');
}

fn expr_struct_init(fmt: &mut Formatter, struct_init: ast::ExprStructInit) {
    if let Some(path) = struct_init.path(fmt.tree) {
        path_fmt(fmt, path);
    }
    fmt.write_c('.');
    fmt.write_c('{');

    let mut first = true;
    let field_init_list = struct_init.field_init_list(fmt.tree).unwrap();
    for field_init in field_init_list.field_inits(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        first = false;
        if let Some(name) = field_init.name(fmt.tree) {
            name_fmt(fmt, name);
            fmt.write_c(':');
            fmt.space();
        }
        expr_fmt(fmt, field_init.expr(fmt.tree).unwrap());
    }
    fmt.write_c('}');
}

fn expr_array_init(fmt: &mut Formatter, array_init: ast::ExprArrayInit) {
    fmt.write_c('[');
    let mut first = true;
    for expr in array_init.inputs(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        first = false;
        expr_fmt(fmt, expr);
    }
    fmt.write_c(']');
}

fn expr_array_repeat(fmt: &mut Formatter, array_repeat: ast::ExprArrayRepeat) {
    let mut expr_len = array_repeat.expr_len_iter(fmt.tree);
    fmt.write_c('[');
    expr_fmt(fmt, expr_len.next().unwrap());
    fmt.write_c(';');
    fmt.space();
    expr_fmt(fmt, expr_len.next().unwrap());
    fmt.write_c(']');
}
