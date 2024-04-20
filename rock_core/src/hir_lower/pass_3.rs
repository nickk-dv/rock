use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::error::ErrorComp;
use crate::hir;

pub fn run<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.registry().proc_ids() {
        process_proc_data(hir, emit, id)
    }
    for id in hir.registry().enum_ids() {
        process_enum_data(hir, emit, id)
    }
    for id in hir.registry().union_ids() {
        process_union_data(hir, emit, id)
    }
    for id in hir.registry().struct_ids() {
        process_struct_data(hir, emit, id)
    }
    for id in hir.registry().const_ids() {
        process_const_data(hir, emit, id)
    }
    for id in hir.registry().global_ids() {
        process_global_data(hir, emit, id)
    }
}

pub fn type_resolve<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: hir::ModuleID,
    ast_ty: ast::Type,
) -> hir::Type<'hir> {
    match ast_ty {
        ast::Type::Basic(basic) => hir::Type::Basic(basic),
        ast::Type::Custom(path) => {
            super::pass_5::path_resolve_type(hir, emit, None, origin_id, path)
        }
        ast::Type::Reference(ref_ty, mutt) => {
            let ref_ty = type_resolve(hir, emit, origin_id, *ref_ty);
            let ty = emit.arena.alloc(ref_ty);
            hir::Type::Reference(ty, mutt)
        }
        ast::Type::ArraySlice(slice) => {
            let elem_ty = type_resolve(hir, emit, origin_id, slice.ty);
            let hir_slice = emit.arena.alloc(hir::ArraySlice {
                mutt: slice.mutt,
                ty: elem_ty,
            });
            hir::Type::ArraySlice(hir_slice)
        }
        ast::Type::ArrayStatic(array) => {
            let size = super::pass_4::const_expr_resolve(hir, emit, origin_id, array.size);
            let elem_ty = type_resolve(hir, emit, origin_id, array.ty);
            let hir_array = emit.arena.alloc(hir::ArrayStatic { size, ty: elem_ty });
            hir::Type::ArrayStatic(hir_array)
        }
    }
}

fn process_proc_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::ProcID,
) {
    let item = hir.registry().proc_item(id);
    let origin_id = hir.registry().proc_data(id).origin_id;
    let mut unique = Vec::<hir::ProcParam>::new();

    for param in item.params.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == param.name.id) {
            emit.error(ErrorComp::error(
                format!(
                    "parameter `{}` is defined multiple times",
                    hir.name_str(param.name.id)
                ),
                hir.src(origin_id, param.name.range),
                ErrorComp::info(
                    "existing parameter",
                    hir.src(origin_id, existing.name.range),
                ),
            ));
        } else {
            unique.push(hir::ProcParam {
                mutt: param.mutt,
                name: param.name,
                ty: type_resolve(hir, emit, origin_id, param.ty),
            });
        }
    }

    hir.registry_mut().proc_data_mut(id).params = emit.arena.alloc_slice(&unique);
    hir.registry_mut().proc_data_mut(id).return_ty = if let Some(ret_ty) = item.return_ty {
        type_resolve(hir, emit, origin_id, ret_ty)
    } else {
        hir::Type::Basic(ast::BasicType::Void)
    }
}

fn process_enum_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::EnumID,
) {
    let item = hir.registry().enum_item(id);
    let origin_id = hir.registry().enum_data(id).origin_id;
    let mut unique = Vec::<hir::EnumVariant>::new();

    let mut implicit_value: u64 = 0;

    for variant in item.variants.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == variant.name.id) {
            emit.error(ErrorComp::error(
                format!(
                    "variant `{}` is defined multiple times",
                    hir.name_str(variant.name.id)
                ),
                hir.src(origin_id, variant.name.range),
                ErrorComp::info("existing variant", hir.src(origin_id, existing.name.range)),
            ));
        } else {
            let value = if let Some(value) = variant.value {
                super::pass_4::const_expr_resolve(hir, emit, origin_id, value)
            } else {
                let expr = emit.arena.alloc(hir::Expr::LitInt {
                    val: implicit_value,
                    ty: ast::BasicType::U64, //@not considering specified type
                });
                implicit_value += 1; //@not considering prev. user defined values
                hir::ConstExpr(expr)
            };

            unique.push(hir::EnumVariant {
                name: variant.name,
                value,
            });
        }
    }

    hir.registry_mut().enum_data_mut(id).variants = emit.arena.alloc_slice(&unique);
}

fn process_union_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::UnionID,
) {
    let item = hir.registry().union_item(id);
    let origin_id = hir.registry().union_data(id).origin_id;
    let mut unique = Vec::<hir::UnionMember>::new();

    for member in item.members.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == member.name.id) {
            emit.error(ErrorComp::error(
                format!(
                    "member `{}` is defined multiple times",
                    hir.name_str(member.name.id)
                ),
                hir.src(origin_id, member.name.range),
                ErrorComp::info("existing member", hir.src(origin_id, existing.name.range)),
            ));
        } else {
            unique.push(hir::UnionMember {
                name: member.name,
                ty: type_resolve(hir, emit, origin_id, member.ty),
            });
        }
    }

    hir.registry_mut().union_data_mut(id).members = emit.arena.alloc_slice(&unique);
}

fn process_struct_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::StructID,
) {
    let item = hir.registry().struct_item(id);
    let origin_id = hir.registry().struct_data(id).origin_id;
    let mut unique = Vec::<hir::StructField>::new();

    for field in item.fields.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == field.name.id) {
            emit.error(ErrorComp::error(
                format!(
                    "field `{}` is defined multiple times",
                    hir.name_str(field.name.id)
                ),
                hir.src(origin_id, field.name.range),
                ErrorComp::info("existing field", hir.src(origin_id, existing.name.range)),
            ));
        } else {
            unique.push(hir::StructField {
                vis: field.vis,
                name: field.name,
                ty: type_resolve(hir, emit, origin_id, field.ty),
            });
        }
    }

    hir.registry_mut().struct_data_mut(id).fields = emit.arena.alloc_slice(&unique);
}

fn process_const_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::ConstID,
) {
    let item = hir.registry().const_item(id);
    let origin_id = hir.registry().const_data(id).origin_id;

    let ty = type_resolve(hir, emit, origin_id, item.ty);
    let data = hir.registry_mut().const_data_mut(id);
    data.ty = ty;
    //@check const_expr value type with resolved type?
}

fn process_global_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::GlobalID,
) {
    let item = hir.registry().global_item(id);
    let origin_id = hir.registry().global_data(id).origin_id;

    let ty = type_resolve(hir, emit, origin_id, item.ty);
    let data = hir.registry_mut().global_data_mut(id);
    data.ty = ty;
    //@check const_expr value type with resolved type?
}
