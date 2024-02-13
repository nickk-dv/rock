use super::ast::*;
use crate::mem::*;

pub fn visit_module_with<T: MutVisit>(vis: &mut T, module: P<Module>) {
    visit_module(vis, module);
}

#[allow(unused_variables)]
pub trait MutVisit: Sized {
    fn visit_module(&mut self, module: P<Module>) {}

    fn visit_ident(&mut self, ident: &mut Ident) {}
    fn visit_path(&mut self, path: &mut Path) {}
    fn visit_type(&mut self, ty: &mut Type) {}

    fn visit_decl(&mut self, decl: Decl) {}
    fn visit_module_decl(&mut self, module_decl: P<ModuleDecl>) {}
    fn visit_import_decl(&mut self, import_decl: P<ImportDecl>) {}
    fn visit_global_decl(&mut self, global_decl: P<GlobalDecl>) {}
    fn visit_proc_decl(&mut self, proc_decl: P<ProcDecl>) {}
    fn visit_enum_decl(&mut self, enum_decl: P<EnumDecl>) {}
    fn visit_union_decl(&mut self, union_decl: P<UnionDecl>) {}
    fn visit_struct_decl(&mut self, struct_decl: P<StructDecl>) {}

    fn visit_stmt(&mut self, stmt: Stmt) {}
    fn visit_for(&mut self, for_: P<For>) {}
    fn visit_var_decl(&mut self, var_decl: P<VarDecl>) {}
    fn visit_var_assign(&mut self, var_assign: P<VarAssign>) {}

    fn visit_expr(&mut self, expr: P<Expr>) {}
    fn visit_const_expr(&mut self, expr: ConstExpr) {}
    fn visit_if(&mut self, if_: P<If>) {}
    fn visit_block(&mut self, block: P<Block>) {}
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
        TypeKind::Custom(mut custom_type) => {
            visit_path(vis, &mut custom_type.path);
            visit_ident(vis, &mut custom_type.name);
        }
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
        ImportTarget::Symbol { ref mut name } => {
            visit_ident(vis, name);
        }
        ImportTarget::SymbolList { names } => {
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
        StmtKind::Break => {}
        StmtKind::Continue => {}
        StmtKind::Return(ret_expr) => {
            if let Some(expr) = ret_expr {
                visit_expr(vis, expr);
            }
        }
        StmtKind::Defer(block) => visit_block(vis, block),
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
    visit_block(vis, for_.block);
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
    match expr.kind {
        ExprKind::Unit => {}
        ExprKind::Discard => {}
        ExprKind::LitNull => {}
        ExprKind::LitBool { .. } => {}
        ExprKind::LitUint { .. } => {}
        ExprKind::LitFloat { .. } => {}
        ExprKind::LitChar { .. } => {}
        ExprKind::LitString { .. } => {}
        ExprKind::If { if_ } => visit_if(vis, if_),
        ExprKind::Block { block } => visit_block(vis, block),
        ExprKind::Match { expr, arms } => {
            visit_expr(vis, expr);
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
        ExprKind::Cast { target, ref mut ty } => {
            visit_expr(vis, target);
            visit_type(vis, ty);
        }
        ExprKind::Sizeof { ref mut ty } => visit_type(vis, ty),
        ExprKind::Item { mut item } => {
            visit_ident(vis, &mut item.name);
            visit_path(vis, &mut item.path);
        }
        ExprKind::ProcCall { mut item, input } => {
            visit_ident(vis, &mut item.name);
            visit_path(vis, &mut item.path);
            for expr in input {
                visit_expr(vis, expr);
            }
        }
        ExprKind::StructInit { mut item, input } => {
            visit_ident(vis, &mut item.name);
            visit_path(vis, &mut item.path);
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
