use crate::error::ErrorComp;
use crate::session::FileID;
use crate::syntax;
use crate::syntax::ast_layer as ast;
use crate::syntax::syntax_tree::SyntaxTree;
use crate::text::TextRange;

//@use session?
pub fn format(source: &str, file_id: FileID) -> Result<String, Vec<ErrorComp>> {
    let (tree, errors) = syntax::parse(source, file_id);
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut fmt = Formatter::new(&tree, source);
    source_file(&mut fmt, tree.source_file());
    Ok(fmt.finish())
}

#[test]
fn format_test() {
    let source = r#"
    #[inline]    pub 
    proc 
    main (
        x : s32 , y : s32  ) ->  u32 ;

    proc p_empty( );
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
    "#;

    if let Ok(formatted) = format(source, FileID::dummy()) {
        println!("ORIGINAL SOURCE:\n-----\n{}-----", source);
        println!("FORMATTED SOURCE:\n-----\n{}-----", formatted);
    } else {
        panic!("incorrect source syntax");
    }
}

struct Formatter<'syn> {
    tree: &'syn SyntaxTree<'syn>,
    source: &'syn str,
    buffer: String,
}

impl<'syn> Formatter<'syn> {
    fn new(tree: &'syn SyntaxTree<'syn>, source: &'syn str) -> Formatter<'syn> {
        Formatter {
            tree,
            source,
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

    fn space(&mut self) {
        self.buffer.push(' ');
    }

    fn tab(&mut self) {
        self.write("    ");
    }

    fn new_line(&mut self) {
        self.buffer.push('\n');
    }

    fn get_str(&self, range: TextRange) -> &'syn str {
        &self.source[range.as_usize()]
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
        fmt.write("{ @todo }");
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
        param_fmt(fmt, param);
        first = false;
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
    let mut first = true;
    for variant in variant_list.variants(fmt.tree) {
        if first {
            fmt.new_line();
            first = false;
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
    let mut first = true;
    for field in field_list.fields(fmt.tree) {
        if first {
            fmt.new_line();
            first = false;
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
    item_attr_vis_fmt!(fmt, item);
    fmt.write("import");
    fmt.space();
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
        name_fmt(fmt, name);
        first = false;
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
        import_symbol_fmt(fmt, import_symbol);
        first = false;
    }
    fmt.write_c('}');
}

fn import_symbol_fmt(fmt: &mut Formatter, import_symbol: ast::ImportSymbol) {
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
    let range = name.token_range(fmt.tree);
    let string = fmt.get_str(range);
    fmt.write(string);
}

fn path(fmt: &mut Formatter, path: ast::Path) {
    let mut first = true;
    for name in path.names(fmt.tree) {
        if !first {
            fmt.write_c('.');
        }
        name_fmt(fmt, name);
        first = false;
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
    let basic = ty.basic_ty(fmt.tree);
    fmt.write(basic.as_str());
}

fn type_custom(fmt: &mut Formatter, ty: ast::TypeCustom) {
    path(fmt, ty.path(fmt.tree).unwrap());
}

fn type_reference(fmt: &mut Formatter, ty: ast::TypeReference) {
    //
}

fn type_procedure(fmt: &mut Formatter, ty: ast::TypeProcedure) {
    //
}

fn type_array_slice(fmt: &mut Formatter, ty: ast::TypeArraySlice) {
    //
}

fn type_array_static(fmt: &mut Formatter, ty: ast::TypeArrayStatic) {
    //
}

fn expr_fmt(fmt: &mut Formatter, expr: ast::Expr) {
    fmt.write("<@expr>")
}
