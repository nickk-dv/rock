use super::ast::*;
use crate::mem::*;

#[allow(unused_variables)]
pub trait MutVisit: Sized {
    fn visit_ast(&mut self, ast: P<Ast>) {}
    fn visit_module(&mut self, module: P<Module>) {}
    fn visit_ident(&mut self, ident: &mut Ident) {}
    fn visit_path(&mut self, path: &mut Path) {}
    fn visit_type(&mut self, ty: &mut Type) {}
    fn visit_custom_type(&mut self, custom_type: P<CustomType>) {}
    fn visit_array_slice(&mut self, array_slice: P<ArraySlice>) {}
    fn visit_array_static(&mut self, array_static: P<ArrayStatic>) {}

    fn visit_decl(&mut self, decl: Decl) {}
    fn visit_module_decl(&mut self, module_decl: P<ModuleDecl>) {}
    fn visit_global_decl(&mut self, global_decl: P<GlobalDecl>) {}
    fn visit_import_decl(&mut self, import_decl: P<ImportDecl>) {}
    fn visit_proc_decl(&mut self, proc_decl: P<ProcDecl>) {}
    fn visit_enum_decl(&mut self, enum_decl: P<EnumDecl>) {}
    fn visit_union_decl(&mut self, union_decl: P<UnionDecl>) {}
    fn visit_struct_decl(&mut self, struct_decl: P<StructDecl>) {}

    fn visit_stmt(&mut self, stmt: Stmt) {}
    fn visit_break(&mut self) {}
    fn visit_continue(&mut self) {}
    fn visit_for(&mut self, for_: P<For>) {}
    fn visit_defer(&mut self, block: P<Block>) {}
    fn visit_return(&mut self, return_: P<Return>) {}
    fn visit_var_decl(&mut self, var_decl: P<VarDecl>) {}
    fn visit_var_assign(&mut self, var_assign: P<VarAssign>) {}
    fn visit_expr_stmt(&mut self, expr_stmt: P<ExprStmt>) {}

    fn visit_expr(&mut self, expr: Expr) {}
    fn visit_const_expr(&mut self, expr: ConstExpr) {}
    fn visit_lit(&mut self, lit: &mut Lit) {}
    fn visit_if(&mut self, if_: P<If>) {}
    fn visit_block(&mut self, block: P<Block>) {}
    fn visit_match(&mut self, switch: P<Match>) {}
    fn visit_index(&mut self, index: P<Index>) {}
    fn visit_dot_name(&mut self, name: &mut Ident) {}
    fn visit_cast(&mut self, cast: P<Cast>) {}
    fn visit_sizeof(&mut self, sizeof: P<Sizeof>) {}
    fn visit_item(&mut self, item: P<Item>) {}
    fn visit_proc_call(&mut self, proc_call: P<ProcCall>) {}
    fn visit_array_init(&mut self, array_init: P<ArrayInit>) {}
    fn visit_struct_init(&mut self, struct_init: P<StructInit>) {}
    fn visit_unary_expr(&mut self, unary_expr: P<UnaryExpr>) {}
    fn visit_binary_expr(&mut self, binary_expr: P<BinaryExpr>) {}
}

pub fn visit_with<T: MutVisit>(vis: &mut T, ast: P<Ast>) {
    visit_ast(vis, ast);
}

pub fn visit_module_with<T: MutVisit>(vis: &mut T, module: P<Module>) {
    visit_module(vis, module);
}

fn visit_ast<T: MutVisit>(vis: &mut T, ast: P<Ast>) {
    vis.visit_ast(ast.copy());
    for module in ast.modules.iter() {
        visit_module(vis, module.copy());
    }
}

fn visit_module<T: MutVisit>(vis: &mut T, module: P<Module>) {
    vis.visit_module(module.copy());
    for decl in module.decls {
        visit_decl(vis, decl);
    }
}

fn visit_ident<T: MutVisit>(vis: &mut T, name: &mut Ident) {
    vis.visit_ident(name);
}

fn visit_path<T: MutVisit>(vis: &mut T, path: &mut Path) {
    vis.visit_path(path);
    for name in path.names.iter_mut() {
        visit_ident(vis, name);
    }
}

fn visit_type<T: MutVisit>(vis: &mut T, ty: &mut Type) {
    vis.visit_type(ty);
    match ty.kind {
        TypeKind::Basic(..) => {}
        TypeKind::Custom(custom_type) => visit_custom_type(vis, custom_type),
        TypeKind::ArraySlice(array_slice) => visit_array_slice(vis, array_slice),
        TypeKind::ArrayStatic(array_static) => visit_array_static(vis, array_static),
        TypeKind::Enum(..) => {}
        TypeKind::Union(..) => {}
        TypeKind::Struct(..) => {}
        TypeKind::Poison => {}
    }
}

fn visit_custom_type<T: MutVisit>(vis: &mut T, mut custom_type: P<CustomType>) {
    vis.visit_custom_type(custom_type);
    visit_path(vis, &mut custom_type.path);
    visit_ident(vis, &mut custom_type.name);
}

fn visit_array_slice<T: MutVisit>(vis: &mut T, mut array_slice: P<ArraySlice>) {
    vis.visit_array_slice(array_slice);
    visit_type(vis, &mut array_slice.ty);
}

fn visit_array_static<T: MutVisit>(vis: &mut T, mut array_static: P<ArrayStatic>) {
    vis.visit_array_static(array_static);
    visit_const_expr(vis, array_static.size);
    visit_type(vis, &mut array_static.ty);
}

fn visit_decl<T: MutVisit>(vis: &mut T, decl: Decl) {
    vis.visit_decl(decl);
    match decl {
        Decl::Module(module_decl) => visit_module_decl(vis, module_decl),
        Decl::Import(import_decl) => visit_import_decl(vis, import_decl),
        Decl::Global(global_decl) => visit_global_decl(vis, global_decl),
        Decl::Proc(proc_decl) => visit_proc_decl(vis, proc_decl),
        Decl::Enum(enum_decl) => visit_enum_decl(vis, enum_decl),
        Decl::Union(union_decl) => visit_union_decl(vis, union_decl),
        Decl::Struct(struct_decl) => visit_struct_decl(vis, struct_decl),
    }
}

fn visit_module_decl<T: MutVisit>(vis: &mut T, mut mod_decl: P<ModuleDecl>) {
    vis.visit_module_decl(mod_decl);
    visit_ident(vis, &mut mod_decl.name);
}

fn visit_import_decl<T: MutVisit>(vis: &mut T, mut import_decl: P<ImportDecl>) {
    vis.visit_import_decl(import_decl);
    visit_path(vis, &mut import_decl.path);
    match import_decl.target {
        ImportTarget::GlobAll => {}
        ImportTarget::Symbol(ref mut name) => visit_ident(vis, name),
        ImportTarget::SymbolList(names) => {
            for name in names.iter_mut() {
                visit_ident(vis, name);
            }
        }
    }
}

fn visit_global_decl<T: MutVisit>(vis: &mut T, mut global_decl: P<GlobalDecl>) {
    vis.visit_global_decl(global_decl);
    visit_ident(vis, &mut global_decl.name);
    if let Some(ref mut ty) = global_decl.ty {
        visit_type(vis, ty);
    }
    visit_const_expr(vis, global_decl.value);
}

fn visit_proc_decl<T: MutVisit>(vis: &mut T, mut proc_decl: P<ProcDecl>) {
    vis.visit_proc_decl(proc_decl);
    visit_ident(vis, &mut proc_decl.name);
    for param in proc_decl.params.iter_mut() {
        visit_ident(vis, &mut param.name);
        visit_type(vis, &mut param.ty);
    }
    if let Some(ref mut ty) = proc_decl.return_ty {
        visit_type(vis, ty);
    }
    if let Some(block) = proc_decl.block {
        visit_block(vis, block);
    }
}

fn visit_enum_decl<T: MutVisit>(vis: &mut T, mut enum_decl: P<EnumDecl>) {
    vis.visit_enum_decl(enum_decl);
    visit_ident(vis, &mut enum_decl.name);
    for variant in enum_decl.variants.iter_mut() {
        visit_ident(vis, &mut variant.name);
        if let Some(value) = variant.value {
            visit_const_expr(vis, value);
        }
    }
}

fn visit_union_decl<T: MutVisit>(vis: &mut T, mut union_decl: P<UnionDecl>) {
    vis.visit_union_decl(union_decl);
    visit_ident(vis, &mut union_decl.name);
    for member in union_decl.members.iter_mut() {
        visit_ident(vis, &mut member.name);
        visit_type(vis, &mut member.ty);
    }
}

fn visit_struct_decl<T: MutVisit>(vis: &mut T, mut struct_decl: P<StructDecl>) {
    vis.visit_struct_decl(struct_decl);
    visit_ident(vis, &mut struct_decl.name);
    for field in struct_decl.fields.iter_mut() {
        visit_ident(vis, &mut field.name);
        visit_type(vis, &mut field.ty);
    }
}

fn visit_stmt<T: MutVisit>(vis: &mut T, stmt: Stmt) {
    vis.visit_stmt(stmt);
    match stmt.kind {
        StmtKind::Break => visit_break(vis),
        StmtKind::Continue => visit_continue(vis),
        StmtKind::For(for_) => visit_for(vis, for_),
        StmtKind::Defer(block) => visit_defer(vis, block),
        StmtKind::Return(return_stmt) => visit_return(vis, return_stmt),
        StmtKind::VarDecl(var_decl) => visit_var_decl(vis, var_decl),
        StmtKind::VarAssign(var_assign) => visit_var_assign(vis, var_assign),
        StmtKind::ExprStmt(expr_stmt) => visit_expr_stmt(vis, expr_stmt),
    }
}

fn visit_break<T: MutVisit>(vis: &mut T) {
    vis.visit_break();
}

fn visit_continue<T: MutVisit>(vis: &mut T) {
    vis.visit_continue();
}

fn visit_for<T: MutVisit>(vis: &mut T, for_: P<For>) {
    vis.visit_for(for_);
    if let Some(var_decl) = for_.var_decl {
        visit_var_decl(vis, var_decl);
    }
    if let Some(cond) = for_.cond {
        visit_expr(vis, cond);
    }
    if let Some(var_assign) = for_.var_assign {
        visit_var_assign(vis, var_assign);
    }
    visit_block(vis, for_.block);
}

fn visit_defer<T: MutVisit>(vis: &mut T, block: P<Block>) {
    vis.visit_defer(block);
    visit_block(vis, block);
}

fn visit_return<T: MutVisit>(vis: &mut T, return_: P<Return>) {
    vis.visit_return(return_);
    if let Some(expr) = return_.expr {
        visit_expr(vis, expr);
    }
}

fn visit_var_decl<T: MutVisit>(vis: &mut T, mut var_decl: P<VarDecl>) {
    vis.visit_var_decl(var_decl);
    if let Some(ref mut name) = var_decl.name {
        visit_ident(vis, name);
    }
    if let Some(ref mut ty) = var_decl.ty {
        visit_type(vis, ty);
    }
    if let Some(expr) = var_decl.expr {
        visit_expr(vis, expr);
    }
}

fn visit_var_assign<T: MutVisit>(vis: &mut T, var_assign: P<VarAssign>) {
    vis.visit_var_assign(var_assign);
    visit_expr(vis, var_assign.lhs);
    visit_expr(vis, var_assign.rhs);
}

fn visit_expr_stmt<T: MutVisit>(vis: &mut T, expr_stmt: P<ExprStmt>) {
    vis.visit_expr_stmt(expr_stmt);
    visit_expr(vis, expr_stmt.expr);
}

fn visit_expr<T: MutVisit>(vis: &mut T, mut expr: Expr) {
    vis.visit_expr(expr);
    match expr.kind {
        ExprKind::Discard => {}
        ExprKind::Lit(ref mut lit) => visit_lit(vis, lit),
        ExprKind::If(if_) => visit_if(vis, if_),
        ExprKind::Block(block) => visit_block(vis, block),
        ExprKind::Match(match_) => visit_match(vis, match_),
        ExprKind::Index(index) => visit_index(vis, index),
        ExprKind::DotName(ref mut name) => visit_dot_name(vis, name),
        ExprKind::Cast(cast) => visit_cast(vis, cast),
        ExprKind::Sizeof(sizeof) => visit_sizeof(vis, sizeof),
        ExprKind::Item(item) => visit_item(vis, item),
        ExprKind::ProcCall(proc_call) => visit_proc_call(vis, proc_call),
        ExprKind::ArrayInit(array_init) => visit_array_init(vis, array_init),
        ExprKind::StructInit(struct_init) => visit_struct_init(vis, struct_init),
        ExprKind::UnaryExpr(unary_expr) => visit_unary_expr(vis, unary_expr),
        ExprKind::BinaryExpr(binary_expr) => visit_binary_expr(vis, binary_expr),
    }
}

fn visit_const_expr<T: MutVisit>(vis: &mut T, expr: ConstExpr) {
    vis.visit_const_expr(expr);
    visit_expr(vis, expr.0);
}

fn visit_lit<T: MutVisit>(vis: &mut T, lit: &mut Lit) {
    vis.visit_lit(lit);
}

fn visit_if<T: MutVisit>(vis: &mut T, if_: P<If>) {
    vis.visit_if(if_);
    visit_expr(vis, if_.cond);
    visit_block(vis, if_.block);
    match if_.else_ {
        Some(Else::If(if_else)) => visit_if(vis, if_else),
        Some(Else::Block(block)) => visit_block(vis, block),
        None => {}
    }
}

fn visit_block<T: MutVisit>(vis: &mut T, block: P<Block>) {
    vis.visit_block(block);
    for stmt in block.stmts {
        visit_stmt(vis, stmt);
    }
}

fn visit_match<T: MutVisit>(vis: &mut T, match_: P<Match>) {
    vis.visit_match(match_);
    visit_expr(vis, match_.expr);
    for arm in match_.arms.iter_mut() {
        visit_expr(vis, arm.pat);
        visit_expr(vis, arm.expr);
    }
}

fn visit_index<T: MutVisit>(vis: &mut T, index: P<Index>) {
    vis.visit_index(index);
    visit_expr(vis, index.expr);
}

fn visit_dot_name<T: MutVisit>(vis: &mut T, name: &mut Ident) {
    vis.visit_dot_name(name);
    visit_ident(vis, name);
}

fn visit_cast<T: MutVisit>(vis: &mut T, mut cast: P<Cast>) {
    vis.visit_cast(cast);
    visit_type(vis, &mut cast.ty);
    visit_expr(vis, cast.expr);
}

fn visit_sizeof<T: MutVisit>(vis: &mut T, mut sizeof: P<Sizeof>) {
    vis.visit_sizeof(sizeof);
    visit_type(vis, &mut sizeof.ty);
}

fn visit_item<T: MutVisit>(vis: &mut T, mut item: P<Item>) {
    vis.visit_item(item);
    visit_path(vis, &mut item.path);
    visit_ident(vis, &mut item.name);
}

fn visit_proc_call<T: MutVisit>(vis: &mut T, mut proc_call: P<ProcCall>) {
    vis.visit_proc_call(proc_call);
    visit_path(vis, &mut proc_call.path);
    visit_ident(vis, &mut proc_call.name);
    for expr in proc_call.input {
        visit_expr(vis, expr);
    }
}

fn visit_array_init<T: MutVisit>(vis: &mut T, array_init: P<ArrayInit>) {
    vis.visit_array_init(array_init);
    for expr in array_init.input {
        visit_expr(vis, expr);
    }
}

fn visit_struct_init<T: MutVisit>(vis: &mut T, mut struct_init: P<StructInit>) {
    vis.visit_struct_init(struct_init);
    visit_path(vis, &mut struct_init.path);
    visit_ident(vis, &mut struct_init.name);
    for field in struct_init.input.iter_mut() {
        visit_ident(vis, &mut field.name);
        if let Some(expr) = field.expr {
            visit_expr(vis, expr);
        }
    }
}

fn visit_unary_expr<T: MutVisit>(vis: &mut T, unary_expr: P<UnaryExpr>) {
    vis.visit_unary_expr(unary_expr);
    visit_expr(vis, unary_expr.rhs);
}

fn visit_binary_expr<T: MutVisit>(vis: &mut T, binary_expr: P<BinaryExpr>) {
    vis.visit_binary_expr(binary_expr);
    visit_expr(vis, binary_expr.lhs);
    visit_expr(vis, binary_expr.rhs);
}
