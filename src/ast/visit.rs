use super::ast::*;

//@change some inputs to by value
// remove && in matches

pub fn visit_module_with<'ast, T: Visit<'ast>>(vis: &mut T, module: &'ast Module) {
    visit_module(vis, module);
}

#[allow(unused_variables)]
pub trait Visit<'ast>: Sized {
    fn visit_module(&mut self, module: &'ast Module) {}

    fn visit_ident(&mut self, ident: &'ast Ident) {}
    fn visit_path(&mut self, path: &'ast Path) {}
    fn visit_type(&mut self, ty: &'ast Type) {}

    fn visit_decl(&mut self, decl: &'ast Decl) {}
    fn visit_mod_decl(&mut self, mod_decl: &'ast ModDecl) {}
    fn visit_use_decl(&mut self, use_decl: &'ast UseDecl) {}
    fn visit_proc_decl(&mut self, proc_decl: &'ast ProcDecl) {}
    fn visit_enum_decl(&mut self, enum_decl: &'ast EnumDecl) {}
    fn visit_union_decl(&mut self, union_decl: &'ast UnionDecl) {}
    fn visit_struct_decl(&mut self, struct_decl: &'ast StructDecl) {}
    fn visit_const_decl(&mut self, const_decl: &'ast ConstDecl) {}
    fn visit_global_decl(&mut self, global_decl: &'ast GlobalDecl) {}

    fn visit_stmt(&mut self, stmt: &'ast Stmt) {}
    fn visit_for(&mut self, for_: &'ast For) {}
    fn visit_var_decl(&mut self, var_decl: &'ast VarDecl) {}
    fn visit_var_assign(&mut self, var_assign: &'ast VarAssign) {}

    fn visit_expr(&mut self, expr: &'ast Expr) {}
    fn visit_const_expr(&mut self, expr: &'ast ConstExpr) {}
    fn visit_if(&mut self, if_: &'ast If) {}
}

fn visit_module<'ast, T: Visit<'ast>>(vis: &mut T, module: &'ast Module) {
    vis.visit_module(module);
    for decl in module.decls.iter() {
        visit_decl(vis, decl);
    }
}

fn visit_ident<'ast, T: Visit<'ast>>(vis: &mut T, name: &'ast Ident) {
    vis.visit_ident(name);
}

fn visit_path<'ast, T: Visit<'ast>>(vis: &mut T, path: &'ast Path) {
    vis.visit_path(path);
    for name in path.names.iter_mut() {
        visit_ident(vis, name);
    }
}

fn visit_type<'ast, T: Visit<'ast>>(vis: &mut T, ty: &'ast Type) {
    vis.visit_type(ty);
    match ty {
        Type::Basic(..) => {}
        Type::Custom(path) => vis.visit_path(path),
        Type::Reference(ref_ty, ..) => {
            visit_type(vis, ref_ty);
        }
        Type::ArraySlice(array_slice) => {
            visit_type(vis, &array_slice.ty);
        }
        Type::ArrayStatic(array_static) => {
            visit_const_expr(vis, &array_static.size);
            visit_type(vis, &array_static.ty);
        }
    }
}

fn visit_decl<'ast, T: Visit<'ast>>(vis: &mut T, decl: &'ast Decl) {
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

fn visit_mod_decl<'ast, T: Visit<'ast>>(vis: &mut T, mod_decl: &'ast ModDecl) {
    vis.visit_mod_decl(mod_decl);
    visit_ident(vis, &mod_decl.name);
}

fn visit_use_decl<'ast, T: Visit<'ast>>(vis: &mut T, use_decl: &'ast UseDecl) {
    vis.visit_use_decl(use_decl);
    visit_path(vis, use_decl.path);
    for symbol in use_decl.symbols.iter_mut() {
        visit_ident(vis, &symbol.name);
        if let Some(ref alias) = symbol.alias {
            visit_ident(vis, alias);
        }
    }
}

fn visit_proc_decl<'ast, T: Visit<'ast>>(vis: &mut T, proc_decl: &'ast ProcDecl) {
    vis.visit_proc_decl(proc_decl);
    visit_ident(vis, &proc_decl.name);
    for param in proc_decl.params.iter_mut() {
        visit_ident(vis, &param.name);
        visit_type(vis, &param.ty);
    }
    if let Some(ref ty) = proc_decl.return_ty {
        visit_type(vis, ty);
    }
    if let Some(block) = proc_decl.block {
        visit_expr(vis, block);
    }
}

fn visit_enum_decl<'ast, T: Visit<'ast>>(vis: &mut T, enum_decl: &'ast EnumDecl) {
    vis.visit_enum_decl(enum_decl);
    visit_ident(vis, &enum_decl.name);
    for variant in enum_decl.variants.iter_mut() {
        visit_ident(vis, &variant.name);
        if let Some(ref value) = variant.value {
            visit_const_expr(vis, value);
        }
    }
}

fn visit_union_decl<'ast, T: Visit<'ast>>(vis: &mut T, union_decl: &'ast UnionDecl) {
    vis.visit_union_decl(union_decl);
    visit_ident(vis, &union_decl.name);
    for member in union_decl.members.iter_mut() {
        visit_ident(vis, &member.name);
        visit_type(vis, &member.ty);
    }
}

fn visit_struct_decl<'ast, T: Visit<'ast>>(vis: &mut T, struct_decl: &'ast StructDecl) {
    vis.visit_struct_decl(struct_decl);
    visit_ident(vis, &struct_decl.name);
    for field in struct_decl.fields.iter_mut() {
        visit_ident(vis, &field.name);
        visit_type(vis, &field.ty);
    }
}

fn visit_const_decl<'ast, T: Visit<'ast>>(vis: &mut T, const_decl: &'ast ConstDecl) {
    vis.visit_const_decl(const_decl);
    visit_ident(vis, &const_decl.name);
    if let Some(ref ty) = const_decl.ty {
        visit_type(vis, ty);
    }
    visit_const_expr(vis, &const_decl.value);
}

fn visit_global_decl<'ast, T: Visit<'ast>>(vis: &mut T, global_decl: &'ast GlobalDecl) {
    vis.visit_global_decl(global_decl);
    visit_ident(vis, &global_decl.name);
    if let Some(ref ty) = global_decl.ty {
        visit_type(vis, ty);
    }
    visit_const_expr(vis, &global_decl.value);
}

fn visit_stmt<'ast, T: Visit<'ast>>(vis: &mut T, stmt: &'ast Stmt) {
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

fn visit_for<'ast, T: Visit<'ast>>(vis: &mut T, for_: &'ast For) {
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

fn visit_var_decl<'ast, T: Visit<'ast>>(vis: &mut T, var_decl: &'ast VarDecl) {
    vis.visit_var_decl(var_decl);
    visit_ident(vis, &var_decl.name);
    if let Some(ref ty) = var_decl.ty {
        visit_type(vis, ty);
    }
    if let Some(expr) = var_decl.expr {
        visit_expr(vis, expr);
    }
}

fn visit_var_assign<'ast, T: Visit<'ast>>(vis: &mut T, var_assign: &'ast VarAssign) {
    vis.visit_var_assign(var_assign);
    visit_expr(vis, var_assign.lhs);
    visit_expr(vis, var_assign.rhs);
}

fn visit_expr<'ast, T: Visit<'ast>>(vis: &mut T, expr: &'ast Expr) {
    vis.visit_expr(expr);
    match expr.kind {
        ExprKind::Unit => {}
        ExprKind::LitNull => {}
        ExprKind::LitBool { .. } => {}
        ExprKind::LitInt { .. } => {}
        ExprKind::LitFloat { .. } => {}
        ExprKind::LitChar { .. } => {}
        ExprKind::LitString { .. } => {}
        ExprKind::If { if_ } => visit_if(vis, if_),
        ExprKind::Block { ref stmts } => {
            for stmt in stmts.iter() {
                visit_stmt(vis, stmt);
            }
        }
        ExprKind::Match { on_expr, ref arms } => {
            visit_expr(vis, on_expr);
            for arm in arms.iter() {
                visit_expr(vis, arm.pat);
                visit_expr(vis, arm.expr);
            }
        }
        ExprKind::Field { target, ref name } => {
            visit_expr(vis, target);
            visit_ident(vis, name);
        }
        ExprKind::Index { target, index } => {
            visit_expr(vis, target);
            visit_expr(vis, index);
        }
        ExprKind::Cast { target, ty } => {
            visit_expr(vis, target);
            visit_type(vis, &ty);
        }
        ExprKind::Sizeof { ref ty } => visit_type(vis, ty),
        ExprKind::Item { path } => visit_path(vis, path),
        ExprKind::ProcCall { path, ref input } => {
            visit_path(vis, path);
            for expr in input.iter() {
                visit_expr(vis, expr);
            }
        }
        ExprKind::StructInit { path, ref input } => {
            visit_path(vis, path);
            for field in input.iter() {
                visit_ident(vis, &field.name);
                if let Some(expr) = field.expr {
                    visit_expr(vis, expr);
                }
            }
        }
        ExprKind::ArrayInit { ref input } => {
            for expr in input.iter() {
                visit_expr(vis, expr);
            }
        }
        ExprKind::ArrayRepeat { expr, ref size } => {
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

fn visit_const_expr<'ast, T: Visit<'ast>>(vis: &mut T, expr: &'ast ConstExpr) {
    vis.visit_const_expr(expr);
    visit_expr(vis, expr.0);
}

fn visit_if<'ast, T: Visit<'ast>>(vis: &mut T, if_: &'ast If) {
    vis.visit_if(if_);
    visit_expr(vis, if_.cond);
    visit_expr(vis, if_.block);
    match if_.else_ {
        Some(Else::If { else_if }) => visit_if(vis, else_if),
        Some(Else::Block { block }) => visit_expr(vis, block),
        None => {}
    }
}
