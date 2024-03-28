use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

pub fn run(hir: &mut HirData, emit: &mut HirEmit) {
    for id in hir.proc_ids() {
        const_resolve_proc_data(hir, emit, id)
    }
    for id in hir.enum_ids() {
        const_resolve_enum_data(hir, emit, id)
    }
    for id in hir.union_ids() {
        const_resolve_union_data(hir, emit, id)
    }
    for id in hir.struct_ids() {
        const_resolve_struct_data(hir, emit, id)
    }
    for id in hir.const_ids() {
        const_resolve_const_data(hir, emit, id)
    }
    for id in hir.global_ids() {
        const_resolve_global_data(hir, emit, id)
    }
}

fn const_resolve_proc_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::ProcID) {
    let data = hir.proc_data(id);
    let origin_id = data.origin_id;
    let ret_ty = data.return_ty;

    for param in data.params.iter() {
        const_resolve_type(hir, emit, origin_id, param.ty);
    }
    const_resolve_type(hir, emit, origin_id, ret_ty);
}

fn const_resolve_enum_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::EnumID) {
    let data = hir.enum_data(id);
    let origin_id = data.origin_id;

    for variant in data.variants.iter() {
        if let Some(const_id) = variant.value {
            const_resolve_const_expr(hir, emit, origin_id, const_id);
        }
    }
}

fn const_resolve_union_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::UnionID) {
    let data = hir.union_data(id);
    let origin_id = data.origin_id;

    for member in data.members.iter() {
        const_resolve_type(hir, emit, origin_id, member.ty);
    }
}

fn const_resolve_struct_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::StructID) {
    let data = hir.struct_data(id);
    let origin_id = data.origin_id;

    for field in data.fields.iter() {
        const_resolve_type(hir, emit, origin_id, field.ty);
    }
}

fn const_resolve_const_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::ConstID) {
    let data = hir.const_data(id);
    let origin_id = data.origin_id;
    let value = data.value;

    const_resolve_type(hir, emit, origin_id, data.ty);
    const_resolve_const_expr(hir, emit, origin_id, value);
}

fn const_resolve_global_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::GlobalID) {
    let data = hir.global_data(id);
    let origin_id = data.origin_id;
    let value = data.value;

    const_resolve_type(hir, emit, origin_id, data.ty);
    const_resolve_const_expr(hir, emit, origin_id, value);
}

fn const_resolve_type(
    hir: &mut HirData,
    emit: &mut HirEmit,
    origin_id: hir::ScopeID,
    ty: hir::Type,
) {
    match ty {
        hir::Type::Reference(ref_ty, _) => const_resolve_type(hir, emit, origin_id, *ref_ty),
        hir::Type::ArraySlice(slice) => const_resolve_type(hir, emit, origin_id, slice.ty),
        hir::Type::ArrayStatic(array) => const_resolve_type(hir, emit, origin_id, array.ty),
        hir::Type::ArrayStaticDecl(array) => {
            const_resolve_const_expr(hir, emit, origin_id, array.size);
            const_resolve_type(hir, emit, origin_id, array.ty);
        }
        _ => {}
    }
}

pub fn const_resolve_const_expr(
    hir: &mut HirData,
    emit: &mut HirEmit,
    origin_id: hir::ScopeID,
    id: hir::ConstExprID,
) {
    let ast_expr = hir.const_expr_ast(id);

    let hir_expr = match ast_expr.kind {
        ast::ExprKind::LitInt { val } => hir::Expr::LitInt {
            val,
            ty: ast::BasicType::U64,
        },
        _ => {
            emit.error(
                ErrorComp::error("only integer constant expressions are supported")
                    .context(hir.src(origin_id, ast_expr.range)),
            );
            hir::Expr::Error
        }
    };
    hir.const_expr_data_mut(id).value = Some(hir_expr);
}

pub fn const_resolve_const_expr_instant<'hir, 'ast>(
    hir: &HirData<'hir, 'ast>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ScopeID,
    expr: &'ast ast::Expr<'ast>,
) -> &'hir hir::Expr<'hir> {
    let hir_expr = match expr.kind {
        ast::ExprKind::LitInt { val } => hir::Expr::LitInt {
            val,
            ty: ast::BasicType::U64,
        },
        _ => {
            emit.error(
                ErrorComp::error("only integer constant expressions are supported")
                    .context(hir.src(origin_id, expr.range)),
            );
            hir::Expr::Error
        }
    };
    emit.arena.alloc(hir_expr)
}
