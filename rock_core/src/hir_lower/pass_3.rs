use super::hir_builder as hb;
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

pub fn run(hb: &mut hb::HirBuilder) {
    for id in hb.proc_ids() {
        process_proc_data(hb, id)
    }
    for id in hb.enum_ids() {
        process_enum_data(hb, id)
    }
    for id in hb.union_ids() {
        process_union_data(hb, id)
    }
    for id in hb.struct_ids() {
        process_struct_data(hb, id)
    }
    for id in hb.const_ids() {
        process_const_data(hb, id)
    }
    for id in hb.global_ids() {
        process_global_data(hb, id)
    }
}

pub fn resolve_decl_type<'ast, 'hir>(
    hb: &mut hb::HirBuilder<'_, 'ast, 'hir>,
    from_id: hir::ScopeID,
    ast_ty: ast::Type<'ast>,
    resolve_const: bool,
) -> hir::Type<'hir> {
    match ast_ty {
        ast::Type::Basic(basic) => hir::Type::Basic(basic),
        ast::Type::Custom(path) => super::pass_5::path_resolve_as_type(hb, from_id, path),
        ast::Type::Reference(ref_ty, mutt) => {
            let ref_ty = resolve_decl_type(hb, from_id, *ref_ty, resolve_const);
            let ty = hb.arena().alloc(ref_ty);
            hir::Type::Reference(ty, mutt)
        }
        ast::Type::ArraySlice(slice) => {
            let elem_ty = resolve_decl_type(hb, from_id, slice.ty, resolve_const);
            let hir_slice = hb.arena().alloc(hir::ArraySlice {
                mutt: slice.mutt,
                ty: elem_ty,
            });
            hir::Type::ArraySlice(hir_slice)
        }
        ast::Type::ArrayStatic(array) => {
            let const_id = hb.add_const_expr(from_id, array.size);
            let elem_ty = resolve_decl_type(hb, from_id, array.ty, resolve_const);
            if resolve_const {
                super::pass_4::const_resolve_const_expr(hb, from_id, const_id);
            }
            let hir_array = hb.arena().alloc(hir::ArrayStaticDecl {
                size: const_id,
                ty: elem_ty,
            });
            hir::Type::ArrayStaticDecl(hir_array)
        }
    }
}

fn process_proc_data(hb: &mut hb::HirBuilder, id: hir::ProcID) {
    let decl = hb.proc_ast(id);
    let from_id = hb.proc_data(id).from_id;
    let mut unique = Vec::<hir::ProcParam>::new();

    for param in decl.params.iter() {
        if let Some(existing) = unique
            .iter()
            .find_map(|it| (it.name.id == param.name.id).then_some(it))
        {
            error_duplicate_proc_param(hb, from_id, param, existing);
        } else {
            unique.push(hir::ProcParam {
                mutt: param.mutt,
                name: param.name,
                ty: resolve_decl_type(hb, from_id, param.ty, false),
            });
        }
    }
    hb.proc_data_mut(id).params = hb.arena().alloc_slice(&unique);
    hb.proc_data_mut(id).return_ty = if let Some(ret_ty) = decl.return_ty {
        resolve_decl_type(hb, from_id, ret_ty, false)
    } else {
        hir::Type::Basic(ast::BasicType::Unit)
    }
}

fn process_enum_data(hb: &mut hb::HirBuilder, id: hir::EnumID) {
    let decl = hb.enum_ast(id);
    let from_id = hb.enum_data(id).from_id;
    let mut unique = Vec::<hir::EnumVariant>::new();

    for variant in decl.variants.iter() {
        if let Some(existing) = unique
            .iter()
            .find_map(|it| (it.name.id == variant.name.id).then_some(it))
        {
            error_duplicate_enum_variant(hb, from_id, variant, existing);
        } else {
            unique.push(hir::EnumVariant {
                name: variant.name,
                value: variant.value.map(|value| hb.add_const_expr(from_id, value)),
            });
        }
    }
    hb.enum_data_mut(id).variants = hb.arena().alloc_slice(&unique);
}

fn process_union_data(hb: &mut hb::HirBuilder, id: hir::UnionID) {
    let decl = hb.union_ast(id);
    let from_id = hb.union_data(id).from_id;
    let mut unique = Vec::<hir::UnionMember>::new();

    for member in decl.members.iter() {
        if let Some(existing) = unique
            .iter()
            .find_map(|it| (it.name.id == member.name.id).then_some(it))
        {
            error_duplicate_union_member(hb, from_id, member, existing);
        } else {
            unique.push(hir::UnionMember {
                name: member.name,
                ty: resolve_decl_type(hb, from_id, member.ty, false),
            });
        }
    }
    hb.union_data_mut(id).members = hb.arena().alloc_slice(&unique);
}

fn process_struct_data(hb: &mut hb::HirBuilder, id: hir::StructID) {
    let decl = hb.struct_ast(id);
    let from_id = hb.struct_data(id).from_id;
    let mut unique = Vec::<hir::StructField>::new();

    for field in decl.fields.iter() {
        if let Some(existing) = unique
            .iter()
            .find_map(|it| (it.name.id == field.name.id).then_some(it))
        {
            error_duplicate_struct_field(hb, from_id, field, existing);
        } else {
            unique.push(hir::StructField {
                vis: field.vis,
                name: field.name,
                ty: resolve_decl_type(hb, from_id, field.ty, false),
            });
        }
    }
    hb.struct_data_mut(id).fields = hb.arena().alloc_slice(&unique);
}

fn process_const_data(hb: &mut hb::HirBuilder, id: hir::ConstID) {
    let decl = hb.const_ast(id);
    let from_id = hb.const_data(id).from_id;

    let ty = resolve_decl_type(hb, from_id, decl.ty, false);
    let const_id = hb.add_const_expr(from_id, decl.value);

    let data = hb.const_data_mut(id);
    data.ty = ty;
    data.value = const_id;
}

fn process_global_data(hb: &mut hb::HirBuilder, id: hir::GlobalID) {
    let decl = hb.global_ast(id);
    let from_id = hb.global_data(id).from_id;

    let ty = resolve_decl_type(hb, from_id, decl.ty, false);
    let const_id = hb.add_const_expr(from_id, decl.value);

    let data = hb.global_data_mut(id);
    data.ty = ty;
    data.value = const_id;
}

fn error_duplicate_proc_param<'ast>(
    hb: &mut hb::HirBuilder,
    from_id: hir::ScopeID,
    param: &'ast ast::ProcParam<'ast>,
    existing: &hir::ProcParam,
) {
    hb.error(
        ErrorComp::error(format!(
            "parameter `{}` is defined multiple times",
            hb.name_str(param.name.id)
        ))
        .context(hb.src(from_id, param.name.range))
        .context_info("existing parameter", hb.src(from_id, existing.name.range)),
    );
}

fn error_duplicate_enum_variant<'ast>(
    hb: &mut hb::HirBuilder,
    from_id: hir::ScopeID,
    variant: &'ast ast::EnumVariant<'ast>,
    existing: &hir::EnumVariant,
) {
    hb.error(
        ErrorComp::error(format!(
            "variant `{}` is defined multiple times",
            hb.name_str(variant.name.id)
        ))
        .context(hb.src(from_id, variant.name.range))
        .context_info("existing variant", hb.src(from_id, existing.name.range)),
    );
}

fn error_duplicate_union_member<'ast>(
    hb: &mut hb::HirBuilder,
    from_id: hir::ScopeID,
    member: &'ast ast::UnionMember<'ast>,
    existing: &hir::UnionMember,
) {
    hb.error(
        ErrorComp::error(format!(
            "member `{}` is defined multiple times",
            hb.name_str(member.name.id)
        ))
        .context(hb.src(from_id, member.name.range))
        .context_info("existing member", hb.src(from_id, existing.name.range)),
    );
}

fn error_duplicate_struct_field<'ast>(
    hb: &mut hb::HirBuilder,
    from_id: hir::ScopeID,
    field: &'ast ast::StructField<'ast>,
    existing: &hir::StructField,
) {
    hb.error(
        ErrorComp::error(format!(
            "field `{}` is defined multiple times",
            hb.name_str(field.name.id)
        ))
        .context(hb.src(from_id, field.name.range))
        .context_info("existing field", hb.src(from_id, existing.name.range)),
    );
}
