use super::hir_builder as hb;
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;
use crate::text::TextRange;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        typecheck_proc(hb, id)
    }
}

fn typecheck_proc(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    let decl = hb.proc_ast(id);
    let data = hb.proc_data(id);

    match decl.block {
        Some(block) => {
            let ty = typecheck_expr(hb, data.from_id, block);
        }
        None => {
            //@for now having a tail directive assumes that it must be a #[c_call]
            // and this directory is only present with no block expression
            let directive_tail = decl
                .directive_tail
                .expect("directive expected with no proc block");

            let directive_name = hb.name_str(directive_tail.name.id);
            if directive_name != "c_call" {
                hb.error(
                    ErrorComp::error(format!(
                        "expected a `c_call` directive, got `{}`",
                        directive_name
                    ))
                    .context(hb.get_scope(data.from_id).source(directive_tail.name.range)),
                )
            }
        }
    }
}

fn type_format(hb: &mut hb::HirBuilder, ty: hir::Type) -> String {
    match ty {
        hir::Type::Error => "error".into(),
        hir::Type::Basic(basic) => match basic {
            ast::BasicType::Unit => "()".into(),
            ast::BasicType::Bool => "bool".into(),
            ast::BasicType::S8 => "s8".into(),
            ast::BasicType::S16 => "s16".into(),
            ast::BasicType::S32 => "s32".into(),
            ast::BasicType::S64 => "s64".into(),
            ast::BasicType::Ssize => "ssize".into(),
            ast::BasicType::U8 => "u8".into(),
            ast::BasicType::U16 => "u16".into(),
            ast::BasicType::U32 => "u32".into(),
            ast::BasicType::U64 => "u64".into(),
            ast::BasicType::Usize => "usize".into(),
            ast::BasicType::F32 => "f32".into(),
            ast::BasicType::F64 => "f64".into(),
            ast::BasicType::Char => "char".into(),
            ast::BasicType::Rawptr => "rawptr".into(),
        },
        hir::Type::Enum(id) => hb.name_str(hb.enum_data(id).name.id).into(),
        hir::Type::Union(id) => hb.name_str(hb.union_data(id).name.id).into(),
        hir::Type::Struct(id) => hb.name_str(hb.struct_data(id).name.id).into(),
        hir::Type::Reference(ref_ty, mutt) => {
            let mut_str = match mutt {
                ast::Mut::Mutable => "mut ",
                ast::Mut::Immutable => "",
            };
            format!("&{}{}", mut_str, type_format(hb, *ref_ty))
        }
        hir::Type::ArraySlice(slice) => {
            let mut_str = match slice.mutt {
                ast::Mut::Mutable => "mut",
                ast::Mut::Immutable => "",
            };
            format!("[{}]{}", mut_str, type_format(hb, slice.ty))
        }
        hir::Type::ArrayStatic(array) => format!("[<SIZE>]{}", type_format(hb, array.ty)),
        hir::Type::ArrayStaticDecl(array) => format!("[<SIZE>]{}", type_format(hb, array.ty)),
    }
}

//@better idea would be to return type repr that is not allocated via arena
// and will be fast to construct and compare
#[must_use]
fn typecheck_expr<'ast, 'hir>(
    hb: &mut hb::HirBuilder<'_, 'ast, 'hir>,
    from_id: hir::ScopeID,
    checked_expr: &ast::Expr<'ast>,
) -> hir::Type<'hir> {
    match checked_expr.kind {
        ast::ExprKind::Unit => hir::Type::Basic(ast::BasicType::Unit),
        ast::ExprKind::LitNull => hir::Type::Basic(ast::BasicType::Rawptr),
        ast::ExprKind::LitBool { val } => hir::Type::Basic(ast::BasicType::Bool),
        ast::ExprKind::LitInt { val, ty } => hir::Type::Basic(ty.unwrap_or(ast::BasicType::S32)),
        ast::ExprKind::LitFloat { val, ty } => hir::Type::Basic(ty.unwrap_or(ast::BasicType::F64)),
        ast::ExprKind::LitChar { val } => hir::Type::Basic(ast::BasicType::Char),
        ast::ExprKind::LitString { id } => {
            let slice = hb.arena().alloc(hir::ArraySlice {
                mutt: ast::Mut::Immutable,
                ty: hir::Type::Basic(ast::BasicType::U8),
            });
            hir::Type::ArraySlice(slice)
        }
        ast::ExprKind::If { if_ } => typecheck_todo(hb, from_id, checked_expr),
        ast::ExprKind::Block { stmts } => {
            for (idx, stmt) in stmts.iter().enumerate() {
                let last = idx + 1 == stmts.len();
                let stmt_ty = match stmt.kind {
                    ast::StmtKind::Break => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::Continue => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::Return(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::Defer(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::ForLoop(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::VarDecl(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::VarAssign(_) => hir::Type::Basic(ast::BasicType::Unit),
                    ast::StmtKind::ExprSemi(expr) => typecheck_expr(hb, from_id, expr),
                    ast::StmtKind::ExprTail(expr) => typecheck_expr(hb, from_id, expr),
                };
                if last {
                    return stmt_ty;
                }
            }
            hir::Type::Basic(ast::BasicType::Unit)
        }
        ast::ExprKind::Match { match_ } => typecheck_todo(hb, from_id, checked_expr),
        ast::ExprKind::Field { target, name } => {
            //@allowing only single reference access of the field
            // no automatic derefencing is done automatically
            // might be a preferred design choice
            let ty = typecheck_expr(hb, from_id, target);
            match ty {
                hir::Type::Reference(ref_ty, mutt) => check_field_ty(hb, from_id, *ref_ty, name),
                _ => check_field_ty(hb, from_id, ty, name),
            }
        }
        ast::ExprKind::Index { target, index } => {
            let ty = typecheck_expr(hb, from_id, target);
            let _ = typecheck_expr(hb, from_id, index); //@expect usize
            match ty {
                hir::Type::Reference(ref_ty, mutt) => {
                    check_index_ty(hb, from_id, *ref_ty, index.range)
                }
                _ => check_index_ty(hb, from_id, ty, index.range),
            }
        }
        ast::ExprKind::Cast { target, ty } => typecheck_todo(hb, from_id, checked_expr),
        ast::ExprKind::Sizeof { ty } => {
            //@check ast type
            hir::Type::Basic(ast::BasicType::Usize)
        }
        ast::ExprKind::Item { path } => typecheck_todo(hb, from_id, checked_expr),
        ast::ExprKind::ProcCall { proc_call } => {
            //@nameresolve proc_call.path

            for &expr in proc_call.input {
                let _ = typecheck_expr(hb, from_id, expr);
            }
            typecheck_todo(hb, from_id, checked_expr)
        }
        ast::ExprKind::StructInit { struct_init } => {
            //@nameresolve struct_init.path

            // @first find if field name exists, only then handle
            // name and expr or just a name
            for init in struct_init.input {
                match init.expr {
                    Some(expr) => {
                        //@field name and the expression
                        let _ = typecheck_expr(hb, from_id, expr);
                    }
                    None => {
                        //@field name that must match some named value in scope
                    }
                }
            }
            typecheck_todo(hb, from_id, checked_expr)
        }
        ast::ExprKind::ArrayInit { input } => typecheck_todo(hb, from_id, checked_expr),
        ast::ExprKind::ArrayRepeat { expr, size } => typecheck_todo(hb, from_id, checked_expr),
        ast::ExprKind::UnaryExpr { op, rhs } => {
            let rhs = typecheck_expr(hb, from_id, rhs);
            typecheck_todo(hb, from_id, checked_expr)
        }
        ast::ExprKind::BinaryExpr { op, lhs, rhs } => {
            let lhs = typecheck_expr(hb, from_id, lhs);
            let rhs = typecheck_expr(hb, from_id, rhs);
            typecheck_todo(hb, from_id, checked_expr)
        }
    }
}

fn typecheck_todo<'hir>(
    hb: &mut hb::HirBuilder,
    from_id: hir::ScopeID,
    checked_expr: &ast::Expr,
) -> hir::Type<'hir> {
    hb.error(
        ErrorComp::warning("this expression is not yet typechecked")
            .context(hb.get_scope(from_id).source(checked_expr.range)),
    );
    hir::Type::Error
}

fn check_field_ty<'hir>(
    hb: &mut hb::HirBuilder<'_, '_, 'hir>,
    from_id: hir::ScopeID,
    ty: hir::Type<'hir>,
    name: ast::Ident,
) -> hir::Type<'hir> {
    match ty {
        hir::Type::Error => hir::Type::Error,
        hir::Type::Union(id) => {
            let data = hb.union_data(id);
            let find = data
                .members
                .iter()
                .enumerate()
                .find_map(|(id, member)| (member.name.id == name.id).then(|| member));
            match find {
                Some(member) => member.ty,
                _ => {
                    hb.error(
                        ErrorComp::error(format!(
                            "no field `{}` exists on union type `{}`",
                            hb.name_str(name.id),
                            hb.name_str(data.name.id),
                        ))
                        .context(hb.get_scope(from_id).source(name.range)),
                    );
                    hir::Type::Error
                }
            }
        }
        hir::Type::Struct(id) => {
            let data = hb.struct_data(id);
            let find = data
                .fields
                .iter()
                .enumerate()
                .find_map(|(id, field)| (field.name.id == name.id).then(|| field));
            match find {
                Some(field) => field.ty,
                _ => {
                    hb.error(
                        ErrorComp::error(format!(
                            "no field `{}` exists on struct type `{}`",
                            hb.name_str(name.id),
                            hb.name_str(data.name.id),
                        ))
                        .context(hb.get_scope(from_id).source(name.range)),
                    );
                    hir::Type::Error
                }
            }
        }
        _ => {
            let ty_format = type_format(hb, ty);
            hb.error(
                ErrorComp::error(format!(
                    "no field `{}` exists on value of type {}",
                    hb.name_str(name.id),
                    ty_format,
                ))
                .context(hb.get_scope(from_id).source(name.range)),
            );
            hir::Type::Error
        }
    }
}

fn check_index_ty<'hir>(
    hb: &mut hb::HirBuilder<'_, '_, 'hir>,
    from_id: hir::ScopeID,
    ty: hir::Type<'hir>,
    index_range: TextRange,
) -> hir::Type<'hir> {
    match ty {
        hir::Type::Error => hir::Type::Error,
        hir::Type::ArraySlice(slice) => slice.ty,
        hir::Type::ArrayStatic(array) => array.ty,
        hir::Type::ArrayStaticDecl(array) => array.ty,
        _ => {
            let ty_format = type_format(hb, ty);
            hb.error(
                ErrorComp::error(format!("cannot index value of type {}", ty_format,))
                    .context(hb.get_scope(from_id).source(index_range)),
            );
            hir::Type::Error
        }
    }
}
