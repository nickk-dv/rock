use super::hir_builder as hb;
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;
use crate::text::TextRange;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        const_resolve_proc_data(hb, id)
    }
    for id in hb.enum_ids() {
        const_resolve_enum_data(hb, id)
    }
    for id in hb.union_ids() {
        const_resolve_union_data(hb, id)
    }
    for id in hb.struct_ids() {
        const_resolve_struct_data(hb, id)
    }
    for id in hb.const_ids() {
        const_resolve_const_data(hb, id)
    }
    for id in hb.global_ids() {
        const_resolve_global_data(hb, id)
    }
}

fn const_resolve_proc_data(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    let data = hb.proc_data(id);
    let from_id = data.from_id;
    let ret_ty = data.return_ty;

    for param in data.params.iter() {
        const_resolve_type(hb, from_id, param.ty);
    }
    const_resolve_type(hb, from_id, ret_ty);
}

fn const_resolve_enum_data(hb: &mut hb::HirBuilder, id: hir::EnumID) {
    let data = hb.enum_data(id);
    let from_id = data.from_id;

    for variant in data.variants.iter() {
        if let Some(const_id) = variant.value {
            const_resolve_const_expr(hb, from_id, const_id);
        }
    }
}

fn const_resolve_union_data(hb: &mut hb::HirBuilder, id: hir::UnionID) {
    let data = hb.union_data(id);
    let from_id = data.from_id;

    for member in data.members.iter() {
        const_resolve_type(hb, from_id, member.ty);
    }
}

fn const_resolve_struct_data(hb: &mut hb::HirBuilder, id: hir::StructID) {
    let data = hb.struct_data(id);
    let from_id = data.from_id;

    for field in data.fields.iter() {
        const_resolve_type(hb, from_id, field.ty);
    }
}

fn const_resolve_const_data(hb: &mut hb::HirBuilder, id: hir::ConstID) {
    let data = hb.const_data(id);
    let from_id = data.from_id;
    let value = data.value;

    const_resolve_type(hb, from_id, data.ty);
    const_resolve_const_expr(hb, from_id, value);
}

fn const_resolve_global_data(hb: &mut hb::HirBuilder, id: hir::GlobalID) {
    let data = hb.global_data(id);
    let from_id = data.from_id;
    let value = data.value;

    const_resolve_type(hb, from_id, data.ty);
    const_resolve_const_expr(hb, from_id, value);
}

fn const_resolve_type(hb: &mut hb::HirBuilder, from_id: hir::ScopeID, ty: hir::Type) {
    match ty {
        hir::Type::Reference(ref_ty, _) => const_resolve_type(hb, from_id, *ref_ty),
        hir::Type::ArraySlice(slice) => const_resolve_type(hb, from_id, slice.ty),
        hir::Type::ArrayStatic(array) => const_resolve_type(hb, from_id, array.ty),
        hir::Type::ArrayStaticDecl(array) => {
            const_resolve_const_expr(hb, from_id, array.size);
            const_resolve_type(hb, from_id, array.ty);
        }
        _ => {}
    }
}

fn const_resolve_const_expr(hb: &mut hb::HirBuilder, from_id: hir::ScopeID, id: hir::ConstExprID) {
    let ast_expr = hb.const_expr_ast(id);

    let hir_expr = match ast_expr.kind {
        ast::ExprKind::LitInt { val, ty } => hir::Expr::LitInt {
            val,
            ty: ast::BasicType::U64,
        },
        _ => {
            error_const_expr_unsupported(hb, from_id, ast_expr.range);
            hir::Expr::Error
        }
    };
    let data = hb.const_expr_data_mut(id);
    data.value = Some(hir_expr)
}

fn error_const_expr_unsupported(hb: &mut hb::HirBuilder, from_id: hir::ScopeID, range: TextRange) {
    let source = hb.src(from_id, range);
    hb.error(ErrorComp::error("only integer constant expressions are supported").context(source));
}
