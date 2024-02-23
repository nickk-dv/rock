use super::ast::*;
use crate::mem::*;

pub fn visit_module_with<T: MutVisit>(vis: &mut T, module: P<Module>) {
    visit_module(vis, module);
}

#[allow(unused_variables)]
pub trait MutVisit: Sized {
    fn visit_module(&mut self, module: P<Module>) {}

    fn visit_ident(&mut self, ident: &mut Ident) {}
    fn visit_path(&mut self, path: P<Path>) {}
    fn visit_type(&mut self, ty: &mut Type) {}

    fn visit_decl(&mut self, decl: Decl) {}
    fn visit_mod_decl(&mut self, mod_decl: P<ModDecl>) {}
    fn visit_use_decl(&mut self, use_decl: P<UseDecl>) {}
    fn visit_proc_decl(&mut self, proc_decl: P<ProcDecl>) {}
    fn visit_enum_decl(&mut self, enum_decl: P<EnumDecl>) {}
    fn visit_union_decl(&mut self, union_decl: P<UnionDecl>) {}
    fn visit_struct_decl(&mut self, struct_decl: P<StructDecl>) {}
    fn visit_const_decl(&mut self, const_decl: P<ConstDecl>) {}
    fn visit_global_decl(&mut self, global_decl: P<GlobalDecl>) {}

    fn visit_stmt(&mut self, stmt: Stmt) {}
    fn visit_for(&mut self, for_: P<For>) {}
    fn visit_var_decl(&mut self, var_decl: P<VarDecl>) {}
    fn visit_var_assign(&mut self, var_assign: P<VarAssign>) {}

    fn visit_expr(&mut self, expr: P<Expr>) {}
    fn visit_const_expr(&mut self, expr: ConstExpr) {}
    fn visit_if(&mut self, if_: P<If>) {}
}

fn visit_module<T: MutVisit>(vis: &mut T, module: P<Module>) {
    vis.visit_module(module);
    for decl in module.decls {
        visit_decl(vis, decl);
    }
}

fn visit_ident<T: MutVisit>(vis: &mut T, name: &mut Ident) {
    vis.visit_ident(name);
}

fn visit_path<T: MutVisit>(vis: &mut T, path: P<Path>) {
    vis.visit_path(path);
    for name in path.names.iter_mut() {
        visit_ident(vis, name);
    }
}

fn visit_type<T: MutVisit>(vis: &mut T, ty: &mut Type) {
    vis.visit_type(ty);
    match ty.kind {
        TypeKind::Basic(..) => {}
        TypeKind::Custom(path) => vis.visit_path(path),
        TypeKind::ArraySlice(mut array_slice) => {
            visit_type(vis, &mut array_slice.ty);
        }
        TypeKind::ArrayStatic(mut array_static) => {
            visit_const_expr(vis, array_static.size);
            visit_type(vis, &mut array_static.ty);
        }
        TypeKind::Enum(..) => {}
        TypeKind::Union(..) => {}
        TypeKind::Struct(..) => {}
        TypeKind::Poison => {}
    }
}

fn visit_decl<T: MutVisit>(vis: &mut T, decl: Decl) {
    vis.visit_decl(decl);
    match decl {
        Decl::Mod(mod_decl) => visit_mod_decl(vis, mod_decl),
        Decl::Use(use_decl) => visit_use_decl(vis, use_decl),
        Decl::Proc(proc_decl) => visit_proc_decl(vis, proc_decl),
        Decl::Enum(enum_decl) => visit_enum_decl(vis, enum_decl),
        Decl::Union(union_decl) => visit_union_decl(vis, union_decl),
        Decl::Struct(struct_decl) => visit_struct_decl(vis, struct_decl),
        Decl::Const(const_decl) => visit_const_decl(vis, const_decl),
        Decl::Global(global_decl) => visit_global_decl(vis, global_decl),
    }
}

fn visit_mod_decl<T: MutVisit>(vis: &mut T, mut mod_decl: P<ModDecl>) {
    vis.visit_mod_decl(mod_decl);
    visit_ident(vis, &mut mod_decl.name);
}

fn visit_use_decl<T: MutVisit>(vis: &mut T, use_decl: P<UseDecl>) {
    vis.visit_use_decl(use_decl);
    visit_path(vis, use_decl.path);
    for symbol in use_decl.symbols.iter_mut() {
        visit_ident(vis, &mut symbol.name);
        if let Some(ref mut alias) = symbol.alias {
            visit_ident(vis, alias);
        }
    }
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
        visit_expr(vis, block);
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

fn visit_const_decl<T: MutVisit>(vis: &mut T, mut const_decl: P<ConstDecl>) {
    vis.visit_const_decl(const_decl);
    visit_ident(vis, &mut const_decl.name);
    if let Some(ref mut ty) = const_decl.ty {
        visit_type(vis, ty);
    }
    visit_const_expr(vis, const_decl.value);
}

fn visit_global_decl<T: MutVisit>(vis: &mut T, mut global_decl: P<GlobalDecl>) {
    vis.visit_global_decl(global_decl);
    visit_ident(vis, &mut global_decl.name);
    if let Some(ref mut ty) = global_decl.ty {
        visit_type(vis, ty);
    }
    visit_const_expr(vis, global_decl.value);
}

fn visit_stmt<T: MutVisit>(vis: &mut T, stmt: Stmt) {
    vis.visit_stmt(stmt);
    match stmt.kind {
        StmtKind::Break => {}
        StmtKind::Continue => {}
        StmtKind::Return(ret_expr) => {
            if let Some(expr) = ret_expr {
                visit_expr(vis, expr);
            }
        }
        StmtKind::Defer(block) => visit_expr(vis, block),
        StmtKind::ForLoop(for_) => visit_for(vis, for_),
        StmtKind::VarDecl(var_decl) => visit_var_decl(vis, var_decl),
        StmtKind::VarAssign(var_assign) => visit_var_assign(vis, var_assign),
        StmtKind::ExprSemi(expr) => visit_expr(vis, expr),
        StmtKind::ExprTail(expr) => visit_expr(vis, expr),
    }
}

fn visit_for<T: MutVisit>(vis: &mut T, for_: P<For>) {
    vis.visit_for(for_);
    match for_.kind {
        ForKind::Loop => {}
        ForKind::While { cond } => {
            visit_expr(vis, cond);
        }
        ForKind::ForLoop {
            var_decl,
            cond,
            var_assign,
        } => {
            visit_var_decl(vis, var_decl);
            visit_expr(vis, cond);
            visit_var_assign(vis, var_assign);
        }
    }
    visit_expr(vis, for_.block);
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

fn visit_expr<T: MutVisit>(vis: &mut T, mut expr: P<Expr>) {
    vis.visit_expr(expr);
    match expr.kind {
        ExprKind::Unit => {}
        ExprKind::Discard => {}
        ExprKind::LitNull => {}
        ExprKind::LitBool { .. } => {}
        ExprKind::LitInt { .. } => {}
        ExprKind::LitFloat { .. } => {}
        ExprKind::LitChar { .. } => {}
        ExprKind::LitString { .. } => {}
        ExprKind::If { if_ } => visit_if(vis, if_),
        ExprKind::Block { stmts } => {
            for stmt in stmts {
                visit_stmt(vis, stmt);
            }
        }
        ExprKind::Match { on_expr, arms } => {
            visit_expr(vis, on_expr);
            for arm in arms {
                visit_expr(vis, arm.pat);
                visit_expr(vis, arm.expr);
            }
        }
        ExprKind::Field {
            target,
            ref mut name,
        } => {
            visit_expr(vis, target);
            visit_ident(vis, name);
        }
        ExprKind::Index { target, index } => {
            visit_expr(vis, target);
            visit_expr(vis, index);
        }
        ExprKind::Cast { target, mut ty } => {
            visit_expr(vis, target);
            visit_type(vis, &mut ty);
        }
        ExprKind::Sizeof { mut ty } => visit_type(vis, &mut ty),
        ExprKind::Item { path } => visit_path(vis, path),
        ExprKind::ProcCall { path, input } => {
            visit_path(vis, path);
            for expr in input {
                visit_expr(vis, expr);
            }
        }
        ExprKind::StructInit { path, input } => {
            visit_path(vis, path);
            for field in input.iter_mut() {
                visit_ident(vis, &mut field.name);
                if let Some(expr) = field.expr {
                    visit_expr(vis, expr);
                }
            }
        }
        ExprKind::ArrayInit { input } => {
            for expr in input {
                visit_expr(vis, expr);
            }
        }
        ExprKind::ArrayRepeat { expr, size } => {
            visit_expr(vis, expr);
            visit_const_expr(vis, size);
        }
        ExprKind::UnaryExpr { rhs, .. } => {
            visit_expr(vis, rhs);
        }
        ExprKind::BinaryExpr { lhs, rhs, .. } => {
            visit_expr(vis, lhs);
            visit_expr(vis, rhs);
        }
    }
}

fn visit_const_expr<T: MutVisit>(vis: &mut T, expr: ConstExpr) {
    vis.visit_const_expr(expr);
    visit_expr(vis, expr.0);
}

fn visit_if<T: MutVisit>(vis: &mut T, if_: P<If>) {
    vis.visit_if(if_);
    visit_expr(vis, if_.cond);
    visit_expr(vis, if_.block);
    match if_.else_ {
        Some(Else::If { else_if }) => visit_if(vis, else_if),
        Some(Else::Block { block }) => visit_expr(vis, block),
        None => {}
    }
}
