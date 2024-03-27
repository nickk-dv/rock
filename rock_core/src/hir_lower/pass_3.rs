use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

pub fn run(hir: &mut HirData, emit: &mut HirEmit) {
    for id in hir.proc_ids() {
        process_proc_data(hir, id)
    }
    for id in hir.enum_ids() {
        process_enum_data(hir, id)
    }
    for id in hir.union_ids() {
        process_union_data(hir, id)
    }
    for id in hir.struct_ids() {
        process_struct_data(hir, id)
    }
    for id in hir.const_ids() {
        process_const_data(hir, id)
    }
    for id in hir.global_ids() {
        process_global_data(hir, id)
    }
}

pub fn resolve_type<'hir, 'ast>(
    hir: &mut HirData,
    emit: &mut HirEmit,
    origin_id: hir::ScopeID,
    ast_ty: ast::Type<'ast>,
    resolve_const: bool,
) -> hir::Type<'hir> {
    match ast_ty {
        ast::Type::Basic(basic) => hir::Type::Basic(basic),
        ast::Type::Custom(path) => super::pass_5::path_resolve_as_type(hir, emit, origin_id, path),
        ast::Type::Reference(ref_ty, mutt) => {
            let ref_ty = resolve_type(hir, emit, origin_id, *ref_ty, resolve_const);
            let ty = emit.arena.alloc(ref_ty);
            hir::Type::Reference(ty, mutt)
        }
        ast::Type::ArraySlice(slice) => {
            let elem_ty = resolve_type(hir, emit, origin_id, slice.ty, resolve_const);
            let hir_slice = emit.arena.alloc(hir::ArraySlice {
                mutt: slice.mutt,
                ty: elem_ty,
            });
            hir::Type::ArraySlice(hir_slice)
        }
        ast::Type::ArrayStatic(array) => {
            let const_id = hir.add_const_expr(origin_id, array.size);
            let elem_ty = resolve_type(hir, emit, origin_id, array.ty, resolve_const);
            if resolve_const {
                super::pass_4::const_resolve_const_expr(hir, emit, origin_id, const_id);
            }
            let hir_array = emit.arena.alloc(hir::ArrayStaticDecl {
                size: const_id,
                ty: elem_ty,
            });
            hir::Type::ArrayStaticDecl(hir_array)
        }
    }
}

fn process_proc_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::ProcID) {
    let item = hir.proc_ast(id);
    let origin_id = hir.proc_data(id).origin_id;
    let mut unique = Vec::<hir::ProcParam>::new();

    for param in item.params.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == param.name.id) {
            error_duplicate_proc_param(hir, origin_id, param, existing);
        } else {
            unique.push(hir::ProcParam {
                mutt: param.mutt,
                name: param.name,
                ty: resolve_type(hir, emit, origin_id, param.ty, false),
            });
        }
    }
    hir.proc_data_mut(id).params = emit.arena.alloc_slice(&unique);
    hir.proc_data_mut(id).return_ty = if let Some(ret_ty) = item.return_ty {
        resolve_type(hir, emit, origin_id, ret_ty, false)
    } else {
        hir::Type::Basic(ast::BasicType::Unit)
    }
}

fn process_enum_data(hir: &mut HirData, emit: &mut HirEmit, id: hir::EnumID) {
    let item = hir.enum_ast(id);
    let origin_id = hir.enum_data(id).origin_id;
    let mut unique = Vec::<hir::EnumVariant>::new();

    for variant in item.variants.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == variant.name.id) {
            error_duplicate_enum_variant(hir, origin_id, variant, existing);
        } else {
            unique.push(hir::EnumVariant {
                name: variant.name,
                value: variant
                    .value
                    .map(|value| hir.add_const_expr(origin_id, value)),
            });
        }
    }
    hir.enum_data_mut(id).variants = emit.arena.alloc_slice(&unique);
}

fn process_union_data(hb: &mut hb::HirBuilder, id: hir::UnionID) {
    let item = hb.union_ast(id);
    let origin_id = hb.union_data(id).origin_id;
    let mut unique = Vec::<hir::UnionMember>::new();

    for member in item.members.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == member.name.id) {
            error_duplicate_union_member(hb, origin_id, member, existing);
        } else {
            unique.push(hir::UnionMember {
                name: member.name,
                ty: resolve_type(hb, origin_id, member.ty, false),
            });
        }
    }
    hb.union_data_mut(id).members = hb.arena().alloc_slice(&unique);
}

fn process_struct_data(hb: &mut hb::HirBuilder, id: hir::StructID) {
    let item = hb.struct_ast(id);
    let origin_id = hb.struct_data(id).origin_id;
    let mut unique = Vec::<hir::StructField>::new();

    for field in item.fields.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == field.name.id) {
            error_duplicate_struct_field(hb, origin_id, field, existing);
        } else {
            unique.push(hir::StructField {
                vis: field.vis,
                name: field.name,
                ty: resolve_type(hb, origin_id, field.ty, false),
            });
        }
    }
    hb.struct_data_mut(id).fields = hb.arena().alloc_slice(&unique);
}

fn process_const_data(hb: &mut hb::HirBuilder, id: hir::ConstID) {
    let item = hb.const_ast(id);
    let origin_id = hb.const_data(id).origin_id;

    let ty = resolve_type(hb, origin_id, item.ty, false);
    let const_id = hb.add_const_expr(origin_id, item.value);

    let data = hb.const_data_mut(id);
    data.ty = ty;
    data.value = const_id;
}

fn process_global_data(hb: &mut hb::HirBuilder, id: hir::GlobalID) {
    let item = hb.global_ast(id);
    let origin_id = hb.global_data(id).origin_id;

    let ty = resolve_type(hb, origin_id, item.ty, false);
    let const_id = hb.add_const_expr(origin_id, item.value);

    let data = hb.global_data_mut(id);
    data.ty = ty;
    data.value = const_id;
}

fn error_duplicate_proc_param<'ast>(
    hb: &mut hb::HirBuilder,
    origin_id: hir::ScopeID,
    param: &'ast ast::ProcParam<'ast>,
    existing: &hir::ProcParam,
) {
    hb.error(
        ErrorComp::error(format!(
            "parameter `{}` is defined multiple times",
            hb.name_str(param.name.id)
        ))
        .context(hb.src(origin_id, param.name.range))
        .context_info("existing parameter", hb.src(origin_id, existing.name.range)),
    );
}

fn error_duplicate_enum_variant<'ast>(
    hb: &mut hb::HirBuilder,
    origin_id: hir::ScopeID,
    variant: &'ast ast::EnumVariant<'ast>,
    existing: &hir::EnumVariant,
) {
    hb.error(
        ErrorComp::error(format!(
            "variant `{}` is defined multiple times",
            hb.name_str(variant.name.id)
        ))
        .context(hb.src(origin_id, variant.name.range))
        .context_info("existing variant", hb.src(origin_id, existing.name.range)),
    );
}

fn error_duplicate_union_member<'ast>(
    hb: &mut hb::HirBuilder,
    origin_id: hir::ScopeID,
    member: &'ast ast::UnionMember<'ast>,
    existing: &hir::UnionMember,
) {
    hb.error(
        ErrorComp::error(format!(
            "member `{}` is defined multiple times",
            hb.name_str(member.name.id)
        ))
        .context(hb.src(origin_id, member.name.range))
        .context_info("existing member", hb.src(origin_id, existing.name.range)),
    );
}

fn error_duplicate_struct_field<'ast>(
    hb: &mut hb::HirBuilder,
    origin_id: hir::ScopeID,
    field: &'ast ast::StructField<'ast>,
    existing: &hir::StructField,
) {
    hb.error(
        ErrorComp::error(format!(
            "field `{}` is defined multiple times",
            hb.name_str(field.name.id)
        ))
        .context(hb.src(origin_id, field.name.range))
        .context_info("existing field", hb.src(origin_id, existing.name.range)),
    );
}
