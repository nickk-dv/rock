use super::ast::*;
use crate::mem::*;

pub fn visit_with<T: MutVisit>(vis: &mut T, ast: P<Ast>) {
    visit_ast(vis, ast);
}

pub fn visit_module_with<T: MutVisit>(vis: &mut T, module: P<Module>) {
    visit_module(vis, module);
}

#[allow(unused)]
pub trait MutVisit: Sized {
    fn visit_ast(&mut self, ast: P<Ast>) {}
    fn visit_module(&mut self, module: P<Module>) {}
    fn visit_ident(&mut self, ident: &mut Ident) {}
    fn visit_module_path(&mut self, module_path: &mut ModulePath) {}
    fn visit_type(&mut self, ty: &mut Type) {}
    fn visit_custom_type(&mut self, custom_type: P<CustomType>) {}
    fn visit_array_slice(&mut self, array_slice: P<ArraySlice>) {}
    fn visit_array_static(&mut self, array_static: P<ArrayStatic>) {}
    fn visit_array_dynamic(&mut self, array_dynamic: P<ArrayDynamic>) {}

    fn visit_decl(&mut self, decl: Decl) {}
    fn visit_mod_decl(&mut self, mod_decl: P<ModDecl>) {}
    fn visit_proc_decl(&mut self, proc_decl: P<ProcDecl>) {}
    fn visit_proc_param(&mut self, param: &mut ProcParam) {}
    fn visit_enum_decl(&mut self, enum_decl: P<EnumDecl>) {}
    fn visit_enum_variant(&mut self, variant: &mut EnumVariant) {}
    fn visit_struct_decl(&mut self, struct_decl: P<StructDecl>) {}
    fn visit_struct_field(&mut self, field: &mut StructField) {}
    fn visit_global_decl(&mut self, global_decl: P<GlobalDecl>) {}
    fn visit_import_decl(&mut self, import_decl: P<ImportDecl>) {}

    fn visit_stmt(&mut self, stmt: Stmt) {}
    fn visit_if(&mut self, if_: P<If>) {}
    fn visit_else(&mut self, else_: Else) {}
    fn visit_for(&mut self, for_: P<For>) {}
    fn visit_block(&mut self, block: P<Block>) {}
    fn visit_defer(&mut self, block: P<Block>) {}
    fn visit_break(&mut self) {}
    fn visit_switch(&mut self, switch: P<Switch>) {}
    fn visit_switch_case(&mut self, case: &mut SwitchCase) {}
    fn visit_return(&mut self, return_: P<Return>) {}
    fn visit_continue(&mut self) {}
    fn visit_var_decl(&mut self, var_decl: P<VarDecl>) {}
    fn visit_var_assign(&mut self, var_assign: P<VarAssign>) {}

    fn visit_expr(&mut self, expr: Expr) {}
    fn visit_var(&mut self, var: P<Var>) {}
    fn visit_access(&mut self, access: P<Access>) {}
    fn visit_enum(&mut self, enum_: P<Enum>) {}
    fn visit_cast(&mut self, cast: P<Cast>) {}
    fn visit_sizeof(&mut self, sizeof: P<Sizeof>) {}
    fn visit_literal(&mut self, literal: P<Literal>) {}
    fn visit_proc_call(&mut self, proc_call: P<ProcCall>) {}
    fn visit_array_init(&mut self, array_init: P<ArrayInit>) {}
    fn visit_struct_init(&mut self, struct_init: P<StructInit>) {}
    fn visit_unary_expr(&mut self, unary_expr: P<UnaryExpr>) {}
    fn visit_binary_expr(&mut self, binary_expr: P<BinaryExpr>) {}
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

fn visit_ident<T: MutVisit>(vis: &mut T, ident: &mut Ident) {
    vis.visit_ident(ident);
}

fn visit_module_path<T: MutVisit>(vis: &mut T, module_path: &mut ModulePath) {
    vis.visit_module_path(module_path);
    for name in module_path.names.iter_mut() {
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
        TypeKind::ArrayDynamic(array_dynamic) => visit_array_dynamic(vis, array_dynamic),
    }
}

fn visit_custom_type<T: MutVisit>(vis: &mut T, mut custom_type: P<CustomType>) {
    vis.visit_custom_type(custom_type);
    visit_module_path(vis, &mut custom_type.module_path);
    visit_ident(vis, &mut custom_type.name);
}

fn visit_array_slice<T: MutVisit>(vis: &mut T, mut array_slice: P<ArraySlice>) {
    vis.visit_array_slice(array_slice);
    visit_type(vis, &mut array_slice.element);
}

fn visit_array_static<T: MutVisit>(vis: &mut T, mut array_static: P<ArrayStatic>) {
    vis.visit_array_static(array_static);
    visit_expr(vis, array_static.size);
    visit_type(vis, &mut array_static.element);
}

fn visit_array_dynamic<T: MutVisit>(vis: &mut T, mut array_dynamic: P<ArrayDynamic>) {
    vis.visit_array_dynamic(array_dynamic);
    visit_type(vis, &mut array_dynamic.element);
}

fn visit_decl<T: MutVisit>(vis: &mut T, decl: Decl) {
    vis.visit_decl(decl);
    match decl {
        Decl::Mod(mod_decl) => visit_mod_decl(vis, mod_decl),
        Decl::Proc(proc_decl) => visit_proc_decl(vis, proc_decl),
        Decl::Enum(enum_decl) => visit_enum_decl(vis, enum_decl),
        Decl::Struct(struct_decl) => visit_struct_decl(vis, struct_decl),
        Decl::Global(global_decl) => visit_global_decl(vis, global_decl),
        Decl::Import(import_decl) => visit_import_decl(vis, import_decl),
    }
}

fn visit_mod_decl<T: MutVisit>(vis: &mut T, mut mod_decl: P<ModDecl>) {
    vis.visit_mod_decl(mod_decl);
    visit_ident(vis, &mut mod_decl.name);
}

fn visit_proc_decl<T: MutVisit>(vis: &mut T, mut proc_decl: P<ProcDecl>) {
    vis.visit_proc_decl(proc_decl);
    visit_ident(vis, &mut proc_decl.name);
    for param in proc_decl.params.iter_mut() {
        visit_proc_param(vis, param);
    }
    if let Some(ref mut ty) = proc_decl.return_type {
        visit_type(vis, ty);
    }
    if let Some(block) = proc_decl.block {
        visit_block(vis, block);
    }
}

fn visit_proc_param<T: MutVisit>(vis: &mut T, param: &mut ProcParam) {
    vis.visit_proc_param(param);
    visit_ident(vis, &mut param.name);
    visit_type(vis, &mut param.ty);
}

fn visit_enum_decl<T: MutVisit>(vis: &mut T, mut enum_decl: P<EnumDecl>) {
    vis.visit_enum_decl(enum_decl);
    visit_ident(vis, &mut enum_decl.name);
    for variant in enum_decl.variants.iter_mut() {
        visit_enum_variant(vis, variant);
    }
}

fn visit_enum_variant<T: MutVisit>(vis: &mut T, variant: &mut EnumVariant) {
    vis.visit_enum_variant(variant);
    visit_ident(vis, &mut variant.name);
    if let Some(expr) = variant.expr {
        visit_expr(vis, expr);
    }
}

fn visit_struct_decl<T: MutVisit>(vis: &mut T, mut struct_decl: P<StructDecl>) {
    vis.visit_struct_decl(struct_decl);
    visit_ident(vis, &mut struct_decl.name);
    for field in struct_decl.fields.iter_mut() {
        visit_struct_field(vis, field);
    }
}

fn visit_struct_field<T: MutVisit>(vis: &mut T, field: &mut StructField) {
    vis.visit_struct_field(field);
    visit_ident(vis, &mut field.name);
    visit_type(vis, &mut field.ty);
    if let Some(expr) = field.default {
        visit_expr(vis, expr);
    }
}

fn visit_global_decl<T: MutVisit>(vis: &mut T, mut global_decl: P<GlobalDecl>) {
    vis.visit_global_decl(global_decl);
    visit_ident(vis, &mut global_decl.name);
    if let Some(ref mut ty) = global_decl.ty {
        visit_type(vis, ty);
    }
    visit_expr(vis, global_decl.expr);
}

fn visit_import_decl<T: MutVisit>(vis: &mut T, mut import_decl: P<ImportDecl>) {
    vis.visit_import_decl(import_decl);
    visit_module_path(vis, &mut import_decl.module_path);
    match import_decl.target {
        ImportTarget::AllSymbols => {}
        ImportTarget::Symbol(ref mut name) => visit_ident(vis, name),
        ImportTarget::SymbolList(names) => {
            for name in names.iter_mut() {
                visit_ident(vis, name);
            }
        }
    }
}

fn visit_stmt<T: MutVisit>(vis: &mut T, stmt: Stmt) {
    vis.visit_stmt(stmt);
    match stmt.kind {
        StmtKind::If(if_) => visit_if(vis, if_),
        StmtKind::For(for_) => visit_for(vis, for_),
        StmtKind::Block(block) => visit_block(vis, block),
        StmtKind::Defer(block) => visit_defer(vis, block),
        StmtKind::Break => visit_break(vis),
        StmtKind::Switch(switch_stmt) => visit_switch(vis, switch_stmt),
        StmtKind::Return(return_stmt) => visit_return(vis, return_stmt),
        StmtKind::Continue => visit_continue(vis),
        StmtKind::VarDecl(var_decl) => visit_var_decl(vis, var_decl),
        StmtKind::VarAssign(var_assign) => visit_var_assign(vis, var_assign),
        StmtKind::ProcCall(proc_call) => visit_proc_call(vis, proc_call),
    }
}

fn visit_if<T: MutVisit>(vis: &mut T, if_: P<If>) {
    vis.visit_if(if_);
    visit_expr(vis, if_.condition);
    visit_block(vis, if_.block);
    if let Some(else_) = if_.else_ {
        visit_else(vis, else_);
    }
}

fn visit_else<T: MutVisit>(vis: &mut T, else_: Else) {
    vis.visit_else(else_);
    match else_ {
        Else::If(if_) => visit_if(vis, if_),
        Else::Block(block) => visit_block(vis, block),
    }
}

fn visit_for<T: MutVisit>(vis: &mut T, for_: P<For>) {
    vis.visit_for(for_);
    if let Some(var_decl) = for_.var_decl {
        visit_var_decl(vis, var_decl);
    }
    if let Some(condition) = for_.condition {
        visit_expr(vis, condition);
    }
    if let Some(var_assign) = for_.var_assign {
        visit_var_assign(vis, var_assign);
    }
    visit_block(vis, for_.block);
}

fn visit_block<T: MutVisit>(vis: &mut T, block: P<Block>) {
    vis.visit_block(block);
    for stmt in block.stmts {
        visit_stmt(vis, stmt);
    }
}

fn visit_defer<T: MutVisit>(vis: &mut T, block: P<Block>) {
    vis.visit_defer(block);
    visit_block(vis, block);
}

fn visit_break<T: MutVisit>(vis: &mut T) {
    vis.visit_break();
}

fn visit_switch<T: MutVisit>(vis: &mut T, switch: P<Switch>) {
    vis.visit_switch(switch);
    visit_expr(vis, switch.expr);
    for case in switch.cases.iter_mut() {
        visit_switch_case(vis, case);
    }
}

fn visit_switch_case<T: MutVisit>(vis: &mut T, case: &mut SwitchCase) {
    vis.visit_switch_case(case);
    visit_expr(vis, case.expr);
    visit_block(vis, case.block);
}

fn visit_return<T: MutVisit>(vis: &mut T, return_: P<Return>) {
    vis.visit_return(return_);
    if let Some(expr) = return_.expr {
        visit_expr(vis, expr);
    }
}

fn visit_continue<T: MutVisit>(vis: &mut T) {
    vis.visit_continue();
}

fn visit_var_decl<T: MutVisit>(vis: &mut T, mut var_decl: P<VarDecl>) {
    vis.visit_var_decl(var_decl);
    visit_ident(vis, &mut var_decl.name);
    if let Some(ref mut ty) = var_decl.ty {
        visit_type(vis, ty);
    }
    if let Some(expr) = var_decl.expr {
        visit_expr(vis, expr);
    }
}

fn visit_var_assign<T: MutVisit>(vis: &mut T, var_assign: P<VarAssign>) {
    vis.visit_var_assign(var_assign);
    visit_var(vis, var_assign.var);
    visit_expr(vis, var_assign.expr);
}

fn visit_expr<T: MutVisit>(vis: &mut T, expr: Expr) {
    vis.visit_expr(expr);
    match expr.kind {
        ExprKind::Var(var) => visit_var(vis, var),
        ExprKind::Enum(enum_) => visit_enum(vis, enum_),
        ExprKind::Cast(cast) => visit_cast(vis, cast),
        ExprKind::Sizeof(sizeof) => visit_sizeof(vis, sizeof),
        ExprKind::Literal(literal) => visit_literal(vis, literal),
        ExprKind::ProcCall(proc_call) => visit_proc_call(vis, proc_call),
        ExprKind::ArrayInit(array_init) => visit_array_init(vis, array_init),
        ExprKind::StructInit(struct_init) => visit_struct_init(vis, struct_init),
        ExprKind::UnaryExpr(unary_expr) => visit_unary_expr(vis, unary_expr),
        ExprKind::BinaryExpr(binary_expr) => visit_binary_expr(vis, binary_expr),
    }
}

fn visit_var<T: MutVisit>(vis: &mut T, mut var: P<Var>) {
    vis.visit_var(var);
    visit_module_path(vis, &mut var.module_path);
    visit_ident(vis, &mut var.name);
    if let Some(access) = var.access {
        visit_access(vis, access);
    }
}

fn visit_access<T: MutVisit>(vis: &mut T, mut access: P<Access>) {
    vis.visit_access(access);
    match access.kind {
        AccessKind::Field(ref mut name) => visit_ident(vis, name),
        AccessKind::Array(expr) => visit_expr(vis, expr),
    }
    if let Some(access) = access.next {
        visit_access(vis, access);
    }
}

fn visit_enum<T: MutVisit>(vis: &mut T, mut enum_: P<Enum>) {
    vis.visit_enum(enum_);
    visit_ident(vis, &mut enum_.variant);
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

fn visit_literal<T: MutVisit>(vis: &mut T, literal: P<Literal>) {
    vis.visit_literal(literal);
}

fn visit_proc_call<T: MutVisit>(vis: &mut T, mut proc_call: P<ProcCall>) {
    vis.visit_proc_call(proc_call);
    visit_module_path(vis, &mut proc_call.module_path);
    visit_ident(vis, &mut proc_call.name);
    for expr in proc_call.input {
        visit_expr(vis, expr);
    }
    if let Some(access) = proc_call.access {
        visit_access(vis, access);
    }
}

fn visit_array_init<T: MutVisit>(vis: &mut T, mut array_init: P<ArrayInit>) {
    vis.visit_array_init(array_init);
    if let Some(ref mut ty) = array_init.ty {
        visit_type(vis, ty);
    }
    for expr in array_init.input {
        visit_expr(vis, expr);
    }
}

fn visit_struct_init<T: MutVisit>(vis: &mut T, mut struct_init: P<StructInit>) {
    vis.visit_struct_init(struct_init);
    visit_module_path(vis, &mut struct_init.module_path);
    if let Some(ref mut name) = struct_init.name {
        visit_ident(vis, name);
    }
    for expr in struct_init.input {
        visit_expr(vis, expr);
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
