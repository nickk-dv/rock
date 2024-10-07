use crate::ast;
use crate::support::AsStr;
use crate::syntax::ast_layer::{self as cst, AstNode};
use crate::syntax::syntax_kind::{SyntaxKind, SyntaxSet};
use crate::syntax::syntax_tree::{Node, NodeOrToken, SyntaxTree};
use crate::text::TextRange;
use crate::token::Trivia;

//@choose .next().is_none vs content_empty() for empty format lists (related to trivia)
//@interleaved_node_list() support same-line line comments
//@aligned padded line comments (eg: after struct fields)
//@unify repeated wrapped lists formatting (generic fn)
//@trim line comment trailing whitespace

#[must_use]
pub fn format(tree: &SyntaxTree, source: &str) -> String {
    let mut fmt = Formatter {
        tree,
        source,
        tab_depth: 0,
        line_offset: 0,
        buffer: String::with_capacity(source.len()),
    };
    source_file(&mut fmt, tree.source_file());
    fmt.buffer
}

struct Formatter<'syn> {
    tree: &'syn SyntaxTree<'syn>,
    source: &'syn str,
    tab_depth: u32,
    line_offset: u32,
    buffer: String,
}

impl<'syn> Formatter<'syn> {
    const TAB_STR: &'static str = "    ";
    const WRAP_THRESHOLD: u32 = 90;
    const SUBWRAP_IMPORT_SYMBOL: u32 = 60;

    fn space(&mut self) {
        self.line_offset += 1;
        self.buffer.push(' ');
    }
    fn new_line(&mut self) {
        self.line_offset = 0;
        self.buffer.push('\n');
    }

    fn write(&mut self, c: char) {
        self.line_offset += 1;
        self.buffer.push(c);
    }
    fn write_str(&mut self, string: &str) {
        self.line_offset += string.len() as u32;
        self.buffer.push_str(string);
    }
    fn write_range(&mut self, range: TextRange) {
        let string = &self.source[range.as_usize()];
        self.write_str(string);
    }

    fn tab_inc(&mut self) {
        self.tab_depth += 1;
    }
    fn tab_dec(&mut self) {
        assert_ne!(self.tab_depth, 0);
        self.tab_depth -= 1;
    }
    fn tab_single(&mut self) {
        self.write_str(Formatter::TAB_STR);
    }
    fn tab_depth(&mut self) {
        for _ in 0..self.tab_depth {
            self.tab_single();
        }
    }

    fn wrap(&self, node: &Node) -> bool {
        let offset = self.line_offset + content_len(self, node);
        offset > Formatter::WRAP_THRESHOLD
    }
}

#[must_use]
fn content_empty(fmt: &mut Formatter, node: &Node) -> bool {
    for not in node.content {
        match *not {
            NodeOrToken::Node(_) => return false,
            NodeOrToken::Token(_) => {}
            NodeOrToken::Trivia(trivia_id) => {
                let trivia = fmt.tree.tokens().trivia(trivia_id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment => return false,
                }
            }
        }
    }
    true
}

#[must_use]
fn content_len(fmt: &Formatter, node: &Node) -> u32 {
    let mut len = 0;
    for not in node.content {
        match *not {
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

fn trivia_lift(fmt: &mut Formatter, node: &Node, halt: SyntaxSet) {
    for not in node.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                let node = fmt.tree.node(node_id);
                if halt.contains(node.kind) {
                    continue;
                }
                trivia_lift(fmt, node, halt);
            }
            NodeOrToken::Trivia(trivia_id) => {
                let (trivia, range) = fmt.tree.tokens().trivia_and_range(trivia_id);
                match trivia {
                    Trivia::Whitespace => {}
                    Trivia::LineComment => {
                        fmt.tab_depth();
                        fmt.write_range(range);
                        fmt.new_line();
                    }
                }
            }
            _ => {}
        }
    }
}

fn interleaved_node_list<'syn, I: AstNode<'syn>>(
    fmt: &mut Formatter<'syn>,
    node_list: &Node<'syn>,
    format_fn: fn(&mut Formatter<'syn>, I),
) {
    let mut first = true;
    let mut new_line = false;

    for not in node_list.content {
        match *not {
            NodeOrToken::Node(node_id) => {
                if new_line {
                    new_line = false;
                    if !first {
                        fmt.new_line();
                    }
                }
                first = false;

                let node = fmt.tree.node(node_id);
                fmt.tab_depth();
                format_fn(fmt, I::cast(node).unwrap());
                fmt.new_line();
            }
            NodeOrToken::Token(_) => {}
            NodeOrToken::Trivia(trivia_id) => {
                let (trivia, range) = fmt.tree.tokens().trivia_and_range(trivia_id);
                match trivia {
                    Trivia::Whitespace => {
                        let whitespace = &fmt.source[range.as_usize()];
                        if whitespace.chars().filter(|&c| c == '\n').count() >= 2 {
                            new_line = true;
                        }
                    }
                    Trivia::LineComment => {
                        if new_line {
                            new_line = false;
                            if !first {
                                fmt.new_line();
                            }
                        }
                        first = false;

                        fmt.tab_depth();
                        let comment = &fmt.source[range.as_usize()];
                        let comment = comment.trim_end();
                        fmt.write_str(comment);
                        fmt.new_line();
                    }
                }
            }
        }
    }
}

//==================== SOURCE FILE ====================

fn source_file<'syn>(fmt: &mut Formatter<'syn>, source_file: cst::SourceFile<'syn>) {
    interleaved_node_list(fmt, source_file.0, item);
}

fn attr_list(fmt: &mut Formatter, attr_list: cst::AttrList) {
    for attr_cst in attr_list.attrs(fmt.tree) {
        attr(fmt, attr_cst);
        fmt.new_line();
    }
}

fn attr(fmt: &mut Formatter, attr: cst::Attr) {
    fmt.write('#');
    fmt.write('[');
    name(fmt, attr.name(fmt.tree).unwrap());
    if let Some(param_list) = attr.param_list(fmt.tree) {
        attr_param_list(fmt, param_list);
    }
    fmt.write(']');
}

fn attr_param_list(fmt: &mut Formatter, param_list: cst::AttrParamList) {
    if param_list.params(fmt.tree).next().is_none() {
        fmt.write('(');
        fmt.write(')');
        return;
    }

    fmt.write('(');
    let wrap = fmt.wrap(param_list.0);
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    for param in param_list.params(fmt.tree) {
        if !first {
            fmt.write(',');
            if wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap {
            fmt.tab_single();
        }
        first = false;

        attr_param(fmt, param);
    }

    if wrap {
        fmt.write(',');
        fmt.new_line();
    }
    fmt.write(')');
}

fn attr_param(fmt: &mut Formatter, param: cst::AttrParam) {
    name(fmt, param.name(fmt.tree).unwrap());
    if let Some(value) = param.value(fmt.tree) {
        fmt.write_str(" = ");
        fmt.write_range(value.range(fmt.tree));
    }
}

fn vis(fmt: &mut Formatter, vis: Option<cst::Visibility>) {
    if vis.is_some() {
        fmt.write_str("pub");
        fmt.space();
    }
}

fn item<'syn>(fmt: &mut Formatter<'syn>, item: cst::Item<'syn>) {
    match item {
        cst::Item::Proc(item) => proc_item(fmt, item),
        cst::Item::Enum(item) => enum_item(fmt, item),
        cst::Item::Struct(item) => struct_item(fmt, item),
        cst::Item::Const(item) => const_item(fmt, item),
        cst::Item::Global(item) => global_item(fmt, item),
        cst::Item::Import(item) => import_item(fmt, item),
    }
}

fn proc_item<'syn>(fmt: &mut Formatter<'syn>, item: cst::ProcItem<'syn>) {
    const HALT: SyntaxSet = SyntaxSet::new(&[SyntaxKind::BLOCK]);
    trivia_lift(fmt, item.0, HALT);
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }
    vis(fmt, item.visibility(fmt.tree));

    fmt.write_str("proc");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());
    param_list(fmt, item.param_list(fmt.tree).unwrap());

    fmt.space();
    fmt.write_str("->");
    fmt.space();
    ty(fmt, item.return_ty(fmt.tree).unwrap());

    if let Some(block_cst) = item.block(fmt.tree) {
        fmt.space();
        block(fmt, block_cst, false);
    } else {
        fmt.write(';');
    }
}

fn param_list<'syn>(fmt: &mut Formatter<'syn>, param_list: cst::ParamList<'syn>) {
    if param_list.params(fmt.tree).next().is_none() {
        fmt.write('(');
        if param_list.is_variadic(fmt.tree) {
            fmt.write_str("..");
        }
        fmt.write(')');
        return;
    }
    fmt.write('(');
    let wrap = fmt.wrap(param_list.0);
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    for param_cst in param_list.params(fmt.tree) {
        if !first {
            fmt.write(',');
            if wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap {
            fmt.tab_single();
        }
        first = false;
        param(fmt, param_cst);
    }

    let is_variadic = param_list.is_variadic(fmt.tree);
    if is_variadic {
        fmt.write(',');
        fmt.space();
        fmt.write_str("..");
    }

    if wrap {
        if !is_variadic {
            fmt.write(',');
        }
        fmt.new_line();
    }
    fmt.write(')');
}

fn param<'syn>(fmt: &mut Formatter<'syn>, param: cst::Param<'syn>) {
    if param.is_mut(fmt.tree) {
        fmt.write_str("mut");
        fmt.space();
    }
    name(fmt, param.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, param.ty(fmt.tree).unwrap());
}

fn enum_item<'syn>(fmt: &mut Formatter<'syn>, item: cst::EnumItem<'syn>) {
    const HALT: SyntaxSet = SyntaxSet::new(&[SyntaxKind::VARIANT_LIST]);
    trivia_lift(fmt, item.0, HALT);
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }
    vis(fmt, item.visibility(fmt.tree));

    fmt.write_str("enum");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());
    fmt.space();
    variant_list(fmt, item.variant_list(fmt.tree).unwrap());
}

fn variant_list<'syn>(fmt: &mut Formatter<'syn>, variant_list: cst::VariantList<'syn>) {
    if content_empty(fmt, variant_list.0) {
        fmt.write('{');
        fmt.write('}');
        return;
    }

    fmt.write('{');
    fmt.new_line();

    fmt.tab_inc();
    interleaved_node_list(fmt, variant_list.0, variant);
    fmt.tab_dec();

    fmt.write('}');
}

fn variant<'syn>(fmt: &mut Formatter<'syn>, variant: cst::Variant<'syn>) {
    name(fmt, variant.name(fmt.tree).unwrap());
    if let Some(field_list) = variant.field_list(fmt.tree) {
        variant_field_list(fmt, field_list);
    }
    fmt.write(',');
}

fn variant_field_list<'syn>(fmt: &mut Formatter<'syn>, field_list: cst::VariantFieldList<'syn>) {
    fmt.write('(');
    let mut first = true;
    for field_ty in field_list.fields(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        ty(fmt, field_ty);
    }
    fmt.write(')');
}

fn struct_item<'syn>(fmt: &mut Formatter<'syn>, item: cst::StructItem<'syn>) {
    const HALT: SyntaxSet = SyntaxSet::new(&[SyntaxKind::FIELD_LIST]);
    trivia_lift(fmt, item.0, HALT);
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }
    vis(fmt, item.visibility(fmt.tree));

    fmt.write_str("struct");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());
    fmt.space();
    field_list(fmt, item.field_list(fmt.tree).unwrap());
}

fn field_list<'syn>(fmt: &mut Formatter<'syn>, field_list: cst::FieldList<'syn>) {
    if content_empty(fmt, field_list.0) {
        fmt.write('{');
        fmt.write('}');
        return;
    }

    fmt.write('{');
    fmt.new_line();

    fmt.tab_inc();
    interleaved_node_list(fmt, field_list.0, field);
    fmt.tab_dec();

    fmt.write('}');
}

fn field<'syn>(fmt: &mut Formatter<'syn>, field: cst::Field<'syn>) {
    name(fmt, field.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, field.ty(fmt.tree).unwrap());
    fmt.write(',');
}

fn const_item<'syn>(fmt: &mut Formatter<'syn>, item: cst::ConstItem<'syn>) {
    trivia_lift(fmt, item.0, SyntaxSet::empty());
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }
    vis(fmt, item.visibility(fmt.tree));

    fmt.write_str("const");
    fmt.space();
    name(fmt, item.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, item.ty(fmt.tree).unwrap());
    fmt.space();
    fmt.write('=');
    fmt.space();
    expr(fmt, item.value(fmt.tree).unwrap());
    fmt.write(';');
}

fn global_item<'syn>(fmt: &mut Formatter<'syn>, item: cst::GlobalItem<'syn>) {
    trivia_lift(fmt, item.0, SyntaxSet::empty());
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }
    vis(fmt, item.visibility(fmt.tree));

    fmt.write_str("global");
    fmt.space();
    if item.is_mut(fmt.tree) {
        fmt.write_str("mut");
        fmt.space();
    }
    name(fmt, item.name(fmt.tree).unwrap());
    fmt.write(':');
    fmt.space();
    ty(fmt, item.ty(fmt.tree).unwrap());
    fmt.space();
    fmt.write('=');
    fmt.space();
    expr(fmt, item.value(fmt.tree).unwrap());
    fmt.write(';');
}

fn import_item(fmt: &mut Formatter, item: cst::ImportItem) {
    trivia_lift(fmt, item.0, SyntaxSet::empty());
    if let Some(attr_list_cst) = item.attr_list(fmt.tree) {
        attr_list(fmt, attr_list_cst);
    }

    fmt.write_str("import");
    fmt.space();
    if let Some(name_cst) = item.package(fmt.tree) {
        name(fmt, name_cst);
        fmt.write(':');
    }

    import_path(fmt, item.import_path(fmt.tree).unwrap());
    if let Some(rename) = item.rename(fmt.tree) {
        import_symbol_rename(fmt, rename);
    }

    if let Some(symbol_list) = item.import_symbol_list(fmt.tree) {
        fmt.write('.');
        import_symbol_list(fmt, symbol_list);
    } else {
        fmt.write(';');
    }
}

fn import_path(fmt: &mut Formatter, import_path: cst::ImportPath) {
    let mut first = true;
    for name_cst in import_path.names(fmt.tree) {
        if !first {
            fmt.write('/');
        }
        first = false;
        name(fmt, name_cst);
    }
}

fn import_symbol_list(fmt: &mut Formatter, import_symbol_list: cst::ImportSymbolList) {
    if import_symbol_list.import_symbols(fmt.tree).next().is_none() {
        fmt.write('{');
        fmt.write('}');
        return;
    }

    fmt.write('{');
    let wrap = fmt.wrap(import_symbol_list.0);
    if wrap {
        fmt.new_line();
    }

    let mut first = true;
    let mut total_len = 0;

    for import_symbol_cst in import_symbol_list.import_symbols(fmt.tree) {
        let sub_wrap = total_len > Formatter::SUBWRAP_IMPORT_SYMBOL;
        if sub_wrap {
            total_len = 0;
        }
        if wrap {
            total_len += content_len(fmt, import_symbol_cst.0);
        }

        if !first {
            fmt.write(',');
            if wrap && sub_wrap {
                fmt.new_line();
            } else {
                fmt.space();
            }
        }
        if wrap && (first || sub_wrap) {
            fmt.tab_single();
        }
        first = false;
        import_symbol(fmt, import_symbol_cst);
    }

    if wrap {
        fmt.write(',');
        fmt.new_line();
    }
    fmt.write('}');
}

fn import_symbol(fmt: &mut Formatter, import_symbol: cst::ImportSymbol) {
    name(fmt, import_symbol.name(fmt.tree).unwrap());
    if let Some(rename) = import_symbol.rename(fmt.tree) {
        import_symbol_rename(fmt, rename);
    }
}

fn import_symbol_rename(fmt: &mut Formatter, rename: cst::ImportSymbolRename) {
    fmt.space();
    fmt.write_str("as");
    fmt.space();
    if let Some(alias) = rename.alias(fmt.tree) {
        name(fmt, alias);
    } else {
        fmt.write('_');
    }
}

//==================== TYPE ====================

fn ty<'syn>(fmt: &mut Formatter<'syn>, ty_cst: cst::Type<'syn>) {
    match ty_cst {
        cst::Type::Basic(ty_cst) => {
            let basic = ty_cst.basic(fmt.tree);
            fmt.write_str(basic.as_str());
        }
        cst::Type::Custom(ty_cst) => {
            path(fmt, ty_cst.path(fmt.tree).unwrap());
        }
        cst::Type::Reference(ty_cst) => {
            fmt.write('&');
            if ty_cst.is_mut(fmt.tree) {
                fmt.write_str("mut");
            }
            fmt.space();
            ty(fmt, ty_cst.ref_ty(fmt.tree).unwrap());
        }
        cst::Type::Procedure(ty_cst) => {
            proc_ty(fmt, ty_cst);
        }
        cst::Type::ArraySlice(ty_cst) => {
            fmt.write('[');
            fmt.write('&');
            if ty_cst.is_mut(fmt.tree) {
                fmt.write_str("mut");
            }
            fmt.write(']');
            ty(fmt, ty_cst.elem_ty(fmt.tree).unwrap());
        }
        cst::Type::ArrayStatic(ty_cst) => {
            fmt.write('[');
            expr(fmt, ty_cst.len(fmt.tree).unwrap());
            fmt.write(']');
            ty(fmt, ty_cst.elem_ty(fmt.tree).unwrap());
        }
    }
}

fn proc_ty<'syn>(fmt: &mut Formatter<'syn>, proc_ty: cst::TypeProcedure<'syn>) {
    fmt.write_str("proc");
    fmt.write('(');

    let mut first = true;
    let param_type_list = proc_ty.type_list(fmt.tree).unwrap();

    for param_ty in param_type_list.types(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        ty(fmt, param_ty);
    }

    if param_type_list.is_variadic(fmt.tree) {
        if param_type_list.types(fmt.tree).next().is_some() {
            fmt.write(',');
            fmt.space();
        }
        fmt.write_str("..");
    }

    fmt.write(')');
    fmt.space();
    fmt.write_str("->");
    fmt.space();
    ty(fmt, proc_ty.return_ty(fmt.tree).unwrap());
}

//==================== STMT ====================

fn block<'syn>(fmt: &mut Formatter<'syn>, block: cst::Block<'syn>, carry: bool) {
    if content_empty(fmt, block.0) {
        fmt.write('{');
        if carry {
            fmt.new_line();
            fmt.tab_depth();
        }
        fmt.write('}');
        return;
    }

    fmt.write('{');
    fmt.new_line();

    fmt.tab_inc();
    interleaved_node_list(fmt, block.0, stmt);
    fmt.tab_dec();

    fmt.tab_depth();
    fmt.write('}');
}

fn stmt<'syn>(fmt: &mut Formatter<'syn>, stmt: cst::Stmt<'syn>) {
    match stmt {
        cst::Stmt::Break(_) => stmt_break(fmt),
        cst::Stmt::Continue(_) => stmt_continue(fmt),
        cst::Stmt::Return(stmt) => stmt_return(fmt, stmt),
        cst::Stmt::Defer(stmt) => stmt_defer(fmt, stmt),
        cst::Stmt::Loop(stmt) => stmt_loop(fmt, stmt),
        cst::Stmt::Local(stmt) => stmt_local(fmt, stmt),
        cst::Stmt::Assign(stmt) => stmt_assign(fmt, stmt, true),
        cst::Stmt::ExprSemi(stmt) => stmt_expr_semi(fmt, stmt),
        cst::Stmt::ExprTail(stmt) => stmt_expr_tail(fmt, stmt),
    }
}

fn stmt_break<'syn>(fmt: &mut Formatter<'syn>) {
    fmt.write_str("break");
    fmt.write(';');
}

fn stmt_continue<'syn>(fmt: &mut Formatter<'syn>) {
    fmt.write_str("continue");
    fmt.write(';');
}

fn stmt_return<'syn>(fmt: &mut Formatter<'syn>, stmt: cst::StmtReturn<'syn>) {
    fmt.write_str("return");
    if let Some(expr_cst) = stmt.expr(fmt.tree) {
        fmt.space();
        expr(fmt, expr_cst);
    }
    fmt.write(';');
}

fn stmt_defer<'syn>(fmt: &mut Formatter<'syn>, defer: cst::StmtDefer<'syn>) {
    fmt.write_str("defer");
    fmt.space();
    if let Some(block_cst) = defer.block(fmt.tree) {
        block(fmt, block_cst, false);
    } else {
        stmt(fmt, defer.stmt(fmt.tree).unwrap());
    }
}

fn stmt_loop<'syn>(fmt: &mut Formatter<'syn>, stmt: cst::StmtLoop<'syn>) {
    fmt.write_str("for");

    if let Some(header) = stmt.while_header(fmt.tree) {
        fmt.space();
        expr(fmt, header.cond(fmt.tree).unwrap());
        fmt.space();
    } else if let Some(header) = stmt.clike_header(fmt.tree) {
        fmt.space();
        stmt_local(fmt, header.local(fmt.tree).unwrap());
        fmt.space();
        expr(fmt, header.cond(fmt.tree).unwrap());
        fmt.write(';');
        fmt.space();
        stmt_assign(fmt, header.assign(fmt.tree).unwrap(), false);
        fmt.space();
    } else {
        fmt.space();
    }

    block(fmt, stmt.block(fmt.tree).unwrap(), false);
}

fn stmt_local<'syn>(fmt: &mut Formatter<'syn>, stmt: cst::StmtLocal<'syn>) {
    fmt.write_str("let");
    fmt.space();
    bind(fmt, stmt.bind(fmt.tree).unwrap());

    if let Some(ty_cst) = stmt.ty(fmt.tree) {
        fmt.write(':');
        fmt.space();
        ty(fmt, ty_cst);
    }

    fmt.space();
    fmt.write('=');
    fmt.space();
    expr(fmt, stmt.init(fmt.tree).unwrap());
    fmt.write(';');
}

fn stmt_assign<'syn>(fmt: &mut Formatter<'syn>, stmt: cst::StmtAssign<'syn>, semi: bool) {
    let mut lhs_rhs = stmt.lhs_rhs_iter(fmt.tree);
    expr(fmt, lhs_rhs.next().unwrap());
    fmt.space();

    let assign_op = stmt.assign_op(fmt.tree).unwrap();
    match assign_op {
        ast::AssignOp::Assign => fmt.write('='),
        ast::AssignOp::Bin(bin_op) => {
            fmt.write_str(bin_op.as_str());
            fmt.write('=');
        }
    }

    fmt.space();
    expr(fmt, lhs_rhs.next().unwrap());
    if semi {
        fmt.write(';');
    }
}

fn stmt_expr_semi<'syn>(fmt: &mut Formatter<'syn>, stmt: cst::StmtExprSemi<'syn>) {
    let expr_cst = stmt.expr(fmt.tree).unwrap();
    expr(fmt, expr_cst);
    match expr_cst {
        cst::Expr::If(_) => {}
        cst::Expr::Block(_) => {}
        cst::Expr::Match(_) => {}
        _ => fmt.write(';'),
    }
}

fn stmt_expr_tail<'syn>(fmt: &mut Formatter<'syn>, stmt: cst::StmtExprTail<'syn>) {
    fmt.write_str("->");
    fmt.space();
    expr(fmt, stmt.expr(fmt.tree).unwrap());
    fmt.write(';');
}

//==================== EXPR ====================

fn expr<'syn>(fmt: &mut Formatter<'syn>, expr: cst::Expr<'syn>) {
    match expr {
        cst::Expr::Paren(expr) => expr_paren(fmt, expr),
        cst::Expr::Lit(lit_cst) => lit(fmt, lit_cst),
        cst::Expr::If(expr) => expr_if(fmt, expr),
        cst::Expr::Block(block_cst) => block(fmt, block_cst, false),
        cst::Expr::Match(expr) => expr_match(fmt, expr),
        cst::Expr::Field(expr) => expr_field(fmt, expr),
        cst::Expr::Index(expr) => expr_index(fmt, expr),
        cst::Expr::Slice(expr) => expr_slice(fmt, expr),
        cst::Expr::Call(expr) => expr_call(fmt, expr),
        cst::Expr::Cast(expr) => expr_cast(fmt, expr),
        cst::Expr::Sizeof(expr) => expr_sizeof(fmt, expr),
        cst::Expr::Item(expr) => expr_item(fmt, expr),
        cst::Expr::Variant(expr) => expr_variant(fmt, expr),
        cst::Expr::StructInit(expr) => expr_struct_init(fmt, expr),
        cst::Expr::ArrayInit(expr) => expr_array_init(fmt, expr),
        cst::Expr::ArrayRepeat(expr) => expr_array_repeat(fmt, expr),
        cst::Expr::Deref(expr) => expr_deref(fmt, expr),
        cst::Expr::Address(expr) => expr_address(fmt, expr),
        cst::Expr::Range(range_cst) => range(fmt, range_cst),
        cst::Expr::Unary(expr) => expr_unary(fmt, expr),
        cst::Expr::Binary(expr) => expr_binary(fmt, expr),
    }
}

fn expr_paren<'syn>(fmt: &mut Formatter<'syn>, paren: cst::ExprParen<'syn>) {
    fmt.write('(');
    expr(fmt, paren.expr(fmt.tree).unwrap());
    fmt.write(')');
}

fn expr_if<'syn>(fmt: &mut Formatter<'syn>, if_: cst::ExprIf<'syn>) {
    let entry = if_.entry_branch(fmt.tree).unwrap();
    fmt.write_str("if");
    fmt.space();
    expr(fmt, entry.cond(fmt.tree).unwrap());
    fmt.space();
    block(fmt, entry.block(fmt.tree).unwrap(), true);

    for branch in if_.else_if_branches(fmt.tree) {
        fmt.space();
        fmt.write_str("else");
        fmt.space();
        fmt.write_str("if");
        fmt.space();
        expr(fmt, branch.cond(fmt.tree).unwrap());
        fmt.space();
        block(fmt, branch.block(fmt.tree).unwrap(), true);
    }

    if let Some(else_block) = if_.else_block(fmt.tree) {
        fmt.space();
        fmt.write_str("else");
        fmt.space();
        block(fmt, else_block, true);
    }
}

//@no empty mode
fn expr_match<'syn>(fmt: &mut Formatter<'syn>, match_: cst::ExprMatch<'syn>) {
    fmt.write_str("match");
    fmt.space();
    expr(fmt, match_.on_expr(fmt.tree).unwrap());
    fmt.space();

    fmt.write('{');
    fmt.new_line();

    fmt.tab_inc();
    let match_arm_list = match_.match_arm_list(fmt.tree).unwrap();
    interleaved_node_list(fmt, match_arm_list.0, match_arm);
    fmt.tab_dec();

    fmt.tab_depth();
    fmt.write('}');
}

fn match_arm<'syn>(fmt: &mut Formatter<'syn>, match_arm: cst::MatchArm<'syn>) {
    pat(fmt, match_arm.pat(fmt.tree).unwrap());
    fmt.space();
    fmt.write_str("->");
    fmt.space();
    expr(fmt, match_arm.expr(fmt.tree).unwrap());
    fmt.write(',');
}

fn expr_field<'syn>(fmt: &mut Formatter<'syn>, field: cst::ExprField<'syn>) {
    expr(fmt, field.target(fmt.tree).unwrap());
    fmt.write('.');
    name(fmt, field.name(fmt.tree).unwrap());
}

fn expr_index<'syn>(fmt: &mut Formatter<'syn>, index: cst::ExprIndex<'syn>) {
    let mut target_index = index.target_index_iter(fmt.tree);
    expr(fmt, target_index.next().unwrap());
    fmt.write('[');
    expr(fmt, target_index.next().unwrap());
    fmt.write(']');
}

fn expr_slice<'syn>(fmt: &mut Formatter<'syn>, slice: cst::ExprSlice<'syn>) {
    let mut target_range = slice.target_range_iter(fmt.tree);
    expr(fmt, target_range.next().unwrap());
    fmt.write('[');
    fmt.write('&');
    if slice.is_mut(fmt.tree) {
        fmt.write_str("mut");
        fmt.space();
    }
    expr(fmt, target_range.next().unwrap());
    fmt.write(']');
}

fn expr_call<'syn>(fmt: &mut Formatter<'syn>, call: cst::ExprCall<'syn>) {
    expr(fmt, call.target(fmt.tree).unwrap());
    args_list(fmt, call.args_list(fmt.tree).unwrap());
}

fn expr_cast<'syn>(fmt: &mut Formatter<'syn>, cast: cst::ExprCast<'syn>) {
    expr(fmt, cast.target(fmt.tree).unwrap());
    fmt.space();
    fmt.write_str("as");
    fmt.space();
    ty(fmt, cast.into_ty(fmt.tree).unwrap());
}

fn expr_sizeof<'syn>(fmt: &mut Formatter<'syn>, sizeof: cst::ExprSizeof<'syn>) {
    fmt.write_str("sizeof");
    fmt.write('(');
    ty(fmt, sizeof.ty(fmt.tree).unwrap());
    fmt.write(')');
}

fn expr_item<'syn>(fmt: &mut Formatter<'syn>, expr: cst::ExprItem<'syn>) {
    path(fmt, expr.path(fmt.tree).unwrap());
    if let Some(args_list_cst) = expr.args_list(fmt.tree) {
        args_list(fmt, args_list_cst);
    }
}

fn expr_variant<'syn>(fmt: &mut Formatter<'syn>, variant: cst::ExprVariant<'syn>) {
    fmt.write('.');
    name(fmt, variant.name(fmt.tree).unwrap());
    if let Some(args_list_cst) = variant.args_list(fmt.tree) {
        args_list(fmt, args_list_cst);
    }
}

fn expr_struct_init<'syn>(fmt: &mut Formatter<'syn>, struct_init: cst::ExprStructInit<'syn>) {
    if let Some(path_cst) = struct_init.path(fmt.tree) {
        path(fmt, path_cst);
    }
    fmt.write('.');
    field_init_list(fmt, struct_init.field_init_list(fmt.tree).unwrap());
}

fn field_init_list<'syn>(fmt: &mut Formatter<'syn>, field_init_list: cst::FieldInitList<'syn>) {
    if field_init_list.field_inits(fmt.tree).next().is_none() {
        fmt.write('{');
        fmt.write('}');
        return;
    }

    fmt.write('{');
    fmt.space();

    let mut first = true;
    for field_init_cst in field_init_list.field_inits(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        field_init(fmt, field_init_cst);
    }

    fmt.space();
    fmt.write('}');
}

fn field_init<'syn>(fmt: &mut Formatter<'syn>, field_init: cst::FieldInit<'syn>) {
    name(fmt, field_init.name(fmt.tree).unwrap());
    if let Some(expr_cst) = field_init.expr(fmt.tree) {
        fmt.write(':');
        fmt.space();
        expr(fmt, expr_cst);
    }
}

fn expr_array_init<'syn>(fmt: &mut Formatter<'syn>, array_init: cst::ExprArrayInit<'syn>) {
    fmt.write('[');

    let mut first = true;
    for expr_cst in array_init.inputs(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        expr(fmt, expr_cst);
    }

    fmt.write(']');
}

fn expr_array_repeat<'syn>(fmt: &mut Formatter<'syn>, array_repeat: cst::ExprArrayRepeat<'syn>) {
    fmt.write('[');

    let mut expr_len = array_repeat.expr_len_iter(fmt.tree);
    expr(fmt, expr_len.next().unwrap());
    fmt.write(';');
    fmt.space();
    expr(fmt, expr_len.next().unwrap());

    fmt.write(']');
}

fn expr_deref<'syn>(fmt: &mut Formatter<'syn>, deref: cst::ExprDeref<'syn>) {
    fmt.write('*');
    expr(fmt, deref.expr(fmt.tree).unwrap());
}

fn expr_address<'syn>(fmt: &mut Formatter<'syn>, address: cst::ExprAddress<'syn>) {
    fmt.write('&');
    if address.is_mut(fmt.tree) {
        fmt.write_str("mut");
        fmt.space();
    }
    expr(fmt, address.expr(fmt.tree).unwrap());
}

fn expr_unary<'syn>(fmt: &mut Formatter<'syn>, unary: cst::ExprUnary<'syn>) {
    let un_op = unary.un_op(fmt.tree);
    fmt.write_str(un_op.as_str());
    expr(fmt, unary.rhs(fmt.tree).unwrap());
}

fn expr_binary<'syn>(fmt: &mut Formatter<'syn>, binary: cst::ExprBinary<'syn>) {
    let mut lhs_rhs = binary.lhs_rhs_iter(fmt.tree);
    expr(fmt, lhs_rhs.next().unwrap());
    fmt.space();
    let bin_op = binary.bin_op(fmt.tree);
    fmt.write_str(bin_op.as_str());
    fmt.space();
    expr(fmt, lhs_rhs.next().unwrap());
}

//==================== PAT ====================

fn pat(fmt: &mut Formatter, pat: cst::Pat) {
    match pat {
        cst::Pat::Wild(_) => fmt.write('_'),
        cst::Pat::Lit(pat) => lit(fmt, pat.lit(fmt.tree).unwrap()),
        cst::Pat::Item(pat) => pat_item(fmt, pat),
        cst::Pat::Variant(pat) => pat_variant(fmt, pat),
        cst::Pat::Or(pat) => pat_or(fmt, pat),
    }
}

fn pat_item(fmt: &mut Formatter, pat: cst::PatItem) {
    path(fmt, pat.path(fmt.tree).unwrap());
    if let Some(bind_list_cst) = pat.bind_list(fmt.tree) {
        bind_list(fmt, bind_list_cst);
    }
}

fn pat_variant(fmt: &mut Formatter, pat: cst::PatVariant) {
    fmt.write('.');
    name(fmt, pat.name(fmt.tree).unwrap());
    if let Some(bind_list_cst) = pat.bind_list(fmt.tree) {
        bind_list(fmt, bind_list_cst);
    }
}

fn pat_or(fmt: &mut Formatter, pat_or: cst::PatOr) {
    let mut first = true;
    for pat_cst in pat_or.patterns(fmt.tree) {
        if !first {
            fmt.space();
            fmt.write('|');
            fmt.space();
        }
        first = false;
        pat(fmt, pat_cst);
    }
}

fn lit(fmt: &mut Formatter, lit: cst::Lit) {
    fmt.write_range(lit.range(fmt.tree));
}

fn range<'syn>(fmt: &mut Formatter<'syn>, range: cst::Range<'syn>) {
    match range {
        cst::Range::Full(_) => fmt.write_str(".."),
        cst::Range::ToExclusive(range) => {
            fmt.write_str("..<");
            expr(fmt, range.end(fmt.tree).unwrap());
        }
        cst::Range::ToInclusive(range) => {
            fmt.write_str("..=");
            expr(fmt, range.end(fmt.tree).unwrap());
        }
        cst::Range::From(range) => {
            expr(fmt, range.start(fmt.tree).unwrap());
            fmt.write_str("..");
        }
        cst::Range::Exclusive(range) => {
            let mut start_end = range.start_end_iter(fmt.tree);
            expr(fmt, start_end.next().unwrap());
            fmt.write_str("..<");
            expr(fmt, start_end.next().unwrap());
        }
        cst::Range::Inclusive(range) => {
            let mut start_end = range.start_end_iter(fmt.tree);
            expr(fmt, start_end.next().unwrap());
            fmt.write_str("..=");
            expr(fmt, start_end.next().unwrap());
        }
    }
}

//==================== COMMON ====================

//@get the ident token + range instead?
fn name(fmt: &mut Formatter, name: cst::Name) {
    fmt.write_range(name.range(fmt.tree));
}

fn path(fmt: &mut Formatter, path: cst::Path) {
    let mut first = true;
    for name_cst in path.names(fmt.tree) {
        if !first {
            fmt.write('.');
        }
        first = false;
        name(fmt, name_cst);
    }
}

fn bind(fmt: &mut Formatter, bind: cst::Bind) {
    if let Some(name_cst) = bind.name(fmt.tree) {
        if bind.is_mut(fmt.tree) {
            fmt.write_str("mut");
            fmt.space();
        }
        name(fmt, name_cst);
    } else {
        fmt.write('_');
    }
}

fn bind_list(fmt: &mut Formatter, bind_list: cst::BindList) {
    fmt.write('(');
    let mut first = true;
    for bind_cst in bind_list.binds(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        bind(fmt, bind_cst);
    }
    fmt.write(')');
}

fn args_list<'syn>(fmt: &mut Formatter<'syn>, args_list: cst::ArgsList<'syn>) {
    fmt.write('(');
    let mut first = true;
    for expr_cst in args_list.exprs(fmt.tree) {
        if !first {
            fmt.write(',');
            fmt.space();
        }
        first = false;
        expr(fmt, expr_cst);
    }
    fmt.write(')');
}
