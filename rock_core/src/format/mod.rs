use crate::ast::AssignOp;
use crate::error::ErrorComp;
use crate::session::ModuleID;
use crate::support::ID;
use crate::syntax::ast_layer as cst;
use crate::syntax::ast_layer::{self as ast, AstNode};
use crate::syntax::syntax_kind::SyntaxKind;
use crate::syntax::syntax_tree::{self, Node, NodeOrToken, SyntaxTree};
use crate::syntax::{self};
use crate::text::TextRange;
use crate::token::Trivia;

//@dont attach whitespace to tokens or nodes
//@whitespace \n >= 2 should limit which prepending trivias can be attached
//@attach comments to select nodes only?
// eg: field list gets inner comment
/*
struct Vector2 // inner com {
    x: f32,
    y: f32,
}
*/

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

#[must_use]
fn content_len(fmt: &mut Formatter, node: &Node) -> u32 {
    let mut len = 0;
    for nt in node.content {
        match *nt {
            NodeOrToken::Node(node_id) => {
                let node = fmt.tree.node(node_id);
                len += content_len(fmt, node);
            }
            NodeOrToken::Token(token_id) => {
                let range = fmt.tree.tokens().token_range(token_id);
                len += range.len();
            }
            NodeOrToken::Trivia(_) => {}
        }
    }
    len
}

fn trivia_lift(fmt: &mut Formatter, node: &Node, halt: SyntaxKind) {
    for nt in node.content {
        match *nt {
            NodeOrToken::Node(node_id) => {
                let node = fmt.tree.node(node_id);
                if node.kind != halt {
                    trivia_lift(fmt, node, halt);
                } else {
                    return;
                }
            }
            NodeOrToken::Trivia(trivia_id) => {
                let (trivia, range) = fmt.tree.tokens().trivia_and_range(trivia_id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment => {
                        fmt.write_range(range);
                        fmt.new_line();
                    }
                    Trivia::BlockComment => {
                        fmt.write_range(range);
                        fmt.new_line();
                    }
                }
            }
            _ => {}
        }
    }
}

fn format_exact(fmt: &mut Formatter, node: &Node) {
    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                format_exact(fmt, fmt.tree.node(node_id));
            }
            NodeOrToken::Token(token_id) => {
                let range = fmt.tree.tokens().token_range(token_id);
                fmt.write_range(range);
            }
            NodeOrToken::Trivia(trivia_id) => {
                let range = fmt.tree.tokens().trivia_range(trivia_id);
                fmt.write_range(range);
            }
        }
    }
}

const WRAP_THRESHOLD: u32 = 90;
const SUBWRAP_SYMBOL_THRESHOLD: u32 = 40;

#[must_use]
pub fn format(tree: &SyntaxTree, source: &str) -> String {
    let mut fmt = Formatter::new(&tree, source);
    source_file(&mut fmt, tree.source_file());
    fmt.finish()
}

fn source_file(fmt: &mut Formatter, source_file: cst::SourceFile) {
    for nt in source_file.0.content {
        match *nt {
            NodeOrToken::Node(node_id) => {
                let node = fmt.tree.node(node_id);
                item(fmt, cst::Item::cast(node).unwrap());
            }
            NodeOrToken::Token(_) => unreachable!(),
            NodeOrToken::Trivia(trivia_id) => {
                let (trivia, range) = fmt.tree.tokens().trivia_and_range(trivia_id);
                match trivia {
                    Trivia::Whitespace => {
                        //@leading or trailing part of the source file
                        //should have no or base endl, without extra spacing
                        let whitespace = &fmt.source[range.as_usize()];
                        if whitespace.chars().map(|c| c == '\n').count() >= 2 {
                            fmt.new_line()
                        }
                    }
                    Trivia::LineComment => {
                        fmt.write_range(range);
                        fmt.new_line();
                    }
                    Trivia::BlockComment => {
                        fmt.write_range(range);
                        fmt.new_line();
                    }
                }
            }
        }
    }
}

fn attr_list(fmt: &mut Formatter, attr_list: cst::AttrList) {
    for attr_cst in attr_list.attrs(fmt.tree) {
        attr(fmt, attr_cst);
        fmt.new_line();
    }
}

fn attr(fmt: &mut Formatter, attr: cst::Attr) {
    fmt.write_c('#');
    fmt.write_c('[');
    name(fmt, attr.name(fmt.tree).unwrap());

    let wrap = content_len(fmt, attr.0) > WRAP_THRESHOLD;
    if let Some(param_list) = attr.param_list(fmt.tree) {
        attr_param_list(fmt, param_list, wrap);
    }

    fmt.write_c(']');
}

fn attr_param_list(fmt: &mut Formatter, param_list: cst::AttrParamList, wrap: bool) {
    if param_list.params(fmt.tree).next().is_none() {
        fmt.write_c('(');
        fmt.write_c(')');
        return;
    }

    fmt.write_c('(');
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    for param in param_list.params(fmt.tree) {
        if !first {
            fmt.write_c(',');
            if wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap {
            fmt.tab();
        }
        first = false;

        attr_param(fmt, param);
    }

    if wrap {
        fmt.write_c(',');
        fmt.new_line();
    }
    fmt.write_c(')');
}

fn attr_param(fmt: &mut Formatter, param: cst::AttrParam) {
    name(fmt, param.name(fmt.tree).unwrap());
    if let Some(value) = param.value(fmt.tree) {
        fmt.write(" = ");
        fmt.write_range(value.range(fmt.tree)); //@will change value to name_id
    }
}

fn vis(fmt: &mut Formatter, vis: Option<cst::Visibility>) {
    if vis.is_some() {
        fmt.write("pub");
        fmt.space();
    }
}

fn item(fmt: &mut Formatter, item: cst::Item) {
    match item {
        cst::Item::Proc(item) => format_exact(fmt, item.0),
        cst::Item::Enum(item) => format_exact(fmt, item.0),
        cst::Item::Struct(item) => format_exact(fmt, item.0),
        cst::Item::Const(item) => format_exact(fmt, item.0),
        cst::Item::Global(item) => format_exact(fmt, item.0),
        cst::Item::Import(item) => import_item(fmt, item),
    }
}

fn struct_item(fmt: &mut Formatter, item: cst::StructItem) {
    trivia_lift(fmt, item.0, SyntaxKind::FIELD_LIST);
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }
    vis(fmt, item.visibility(fmt.tree));

    fmt.write("struct");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());
    fmt.space();
    fmt.write("{}"); //@fmt field list
    fmt.new_line();
}

//@handle possible inner & trailing trivia
//@handle field spacing (similar to source_file)
fn field_list(fmt: &mut Formatter, field_list: cst::FieldList) {
    fmt.write_c('{');
    //@todo
    fmt.write_c('}');
}

//@handle field trivia
// match trivia indent with current indent
fn field(fmt: &mut Formatter, field: cst::Field) {
    //@todo
}

fn import_item(fmt: &mut Formatter, item: ast::ImportItem) {
    trivia_lift(fmt, item.0, SyntaxKind::TOMBSTONE);
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }

    fmt.write("import");
    fmt.space();
    if let Some(name_cst) = item.package(fmt.tree) {
        name(fmt, name_cst);
        fmt.write_c(':');
    }

    import_path(fmt, item.import_path(fmt.tree).unwrap());
    if let Some(rename) = item.rename(fmt.tree) {
        import_symbol_rename(fmt, rename);
    }

    if let Some(symbol_list) = item.import_symbol_list(fmt.tree) {
        fmt.write_c('.');
        let wrap = content_len(fmt, item.0) > WRAP_THRESHOLD;
        import_symbol_list(fmt, symbol_list, wrap);
    } else {
        fmt.write_c(';');
    }
    fmt.new_line();
}

fn import_path(fmt: &mut Formatter, import_path: ast::ImportPath) {
    let mut first = true;
    for name_cst in import_path.names(fmt.tree) {
        if !first {
            fmt.write_c('/');
        }
        first = false;

        name(fmt, name_cst);
    }
}

fn import_symbol_list(fmt: &mut Formatter, import_symbol_list: ast::ImportSymbolList, wrap: bool) {
    if import_symbol_list.import_symbols(fmt.tree).next().is_none() {
        fmt.write_c('{');
        fmt.write_c('}');
        return;
    }

    fmt.write_c('{');
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    let mut total_len = 0;

    for import_symbol in import_symbol_list.import_symbols(fmt.tree) {
        let sub_wrap = total_len > SUBWRAP_SYMBOL_THRESHOLD;
        if sub_wrap {
            total_len = 0;
        }
        total_len += content_len(fmt, import_symbol.0);

        if !first {
            fmt.write_c(',');
            if wrap && sub_wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap && (first || sub_wrap) {
            fmt.tab();
        }
        first = false;

        import_symbol_fmt(fmt, import_symbol);
    }

    if wrap {
        fmt.write_c(',');
        fmt.new_line();
    }
    fmt.write_c('}');
}

fn import_symbol_fmt(fmt: &mut Formatter, import_symbol: ast::ImportSymbol) {
    name(fmt, import_symbol.name(fmt.tree).unwrap());
    if let Some(rename) = import_symbol.rename(fmt.tree) {
        import_symbol_rename(fmt, rename);
    }
}

fn import_symbol_rename(fmt: &mut Formatter, rename: ast::ImportSymbolRename) {
    fmt.space();
    fmt.write("as");
    fmt.space();
    if let Some(alias) = rename.alias(fmt.tree) {
        name(fmt, alias);
    } else {
        fmt.write_c('_');
    }
}

//@assuming no trivia attached to the name (else it can be lost)
fn name(fmt: &mut Formatter, name: cst::Name) {
    fmt.write_range(name.range(fmt.tree));
}

/*
pub fn format(tree: &SyntaxTree, source: &str) -> String {
    let mut fmt = Formatter::new(&tree, source);
    source_file(&mut fmt, tree.source_file());
    fmt.finish()
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

fn attribute_list(fmt: &mut Formatter, attr_list: ast::AttrList) {
    for attr in attr_list.attrs(fmt.tree) {
        attribute(fmt, attr);
        fmt.new_line();
    }
}

fn attribute(fmt: &mut Formatter, attr: ast::Attr) {
    fmt.write("#[");
    name_fmt(fmt, attr.name(fmt.tree).unwrap());
    fmt.write_c(']');
}

fn visibility(fmt: &mut Formatter, vis: ast::Visibility) {
    if vis.is_pub(fmt.tree) {
        fmt.write("pub");
        fmt.space();
    }
}

macro_rules! item_attr_vis_fmt {
    ($fmt:ident, $item:expr) => {
        if let Some(attr_list) = $item.attr_list($fmt.tree) {
            attribute_list($fmt, attr_list);
        }
        if let Some(vis) = $item.visibility($fmt.tree) {
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

//@type list not formatted
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
    field_list_old(fmt, item.field_list(fmt.tree).unwrap());
    fmt.new_line();
}

fn field_list_old(fmt: &mut Formatter, field_list: ast::FieldList) {
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
    param_type_list(fmt, ty.type_list(fmt.tree).unwrap());
    if let Some(ty) = ty.return_ty(fmt.tree) {
        fmt.space();
        fmt.write("->");
        fmt.space();
        type_fmt(fmt, ty);
    }
}

fn param_type_list(fmt: &mut Formatter, type_list: ast::ParamTypeList) {
    fmt.write_c('(');
    let mut first = true;
    for ty in type_list.types(fmt.tree) {
        if !first {
            fmt.write_c(',');
            fmt.space();
        }
        first = false;
        type_fmt(fmt, ty);
    }
    if type_list.is_variadic(fmt.tree) {
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
            block_fmt(fmt, defer.block(fmt.tree).unwrap());
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
    //@name_fmt(fmt, local.name(fmt.tree).unwrap());

    if let Some(ty) = local.ty(fmt.tree) {
        fmt.write_c(':');
        fmt.space();
        type_fmt(fmt, ty);
        if let Some(expr) = local.init(fmt.tree) {
            fmt.space();
            fmt.write_c('=');
            fmt.space();
            expr_fmt(fmt, expr);
        }
    } else {
        fmt.space();
        fmt.write_c('=');
        fmt.space();
        expr_fmt(fmt, local.init(fmt.tree).unwrap());
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
        /*@disabled
        ast::Expr::LitNull(_) => fmt.write("null"),
        ast::Expr::LitBool(lit) => {
            if lit.value(fmt.tree) {
                fmt.write("true");
            } else {
                fmt.write("false");
            }
        }
        ast::Expr::LitInt(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::LitFloat(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::LitChar(lit) => fmt.write_range(lit.range(fmt.tree)),
        ast::Expr::LitString(lit) => fmt.write_range(lit.range(fmt.tree)),
        */
        ast::Expr::If(if_) => expr_if(fmt, if_),
        ast::Expr::Block(block) => block_fmt(fmt, block),
        ast::Expr::Match(match_) => {} //@todo match2 fmt
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
            fmt.space();
            fmt.write(op.as_str());
            fmt.space();
            expr_fmt(fmt, lhs_rhs.next().unwrap());
        }
        _ => todo!("format is not used"),
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

fn expr_index(fmt: &mut Formatter, index: ast::ExprIndex) {
    let mut target_index_iter = index.target_index_iter(fmt.tree);
    expr_fmt(fmt, target_index_iter.next().unwrap());
    fmt.write_c('[');
    if index.is_mut(fmt.tree) {
        fmt.write("mut");
        fmt.space();
    }
    expr_fmt(fmt, target_index_iter.next().unwrap());
    fmt.write_c(']');
}

fn expr_call(fmt: &mut Formatter, call: ast::ExprCall) {
    expr_fmt(fmt, call.target(fmt.tree).unwrap());
    fmt.write_c('(');
    let mut first = true;
    let args_list = call.args_list(fmt.tree).unwrap();
    for expr in args_list.exprs(fmt.tree) {
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
*/
