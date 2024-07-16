use super::hir_build::{HirData, HirEmit};
use super::pass_4;
use super::pass_5::{self, Expectation};
use crate::ast;
use crate::error::{ErrorComp, Info, SourceRange};
use crate::hir;
use crate::session::ModuleID;

pub fn process_items<'hir>(hir: &mut HirData<'hir, '_, '_>, emit: &mut HirEmit<'hir>) {
    for id in hir.registry().proc_ids() {
        process_proc_data(hir, emit, id)
    }
    for id in hir.registry().enum_ids() {
        process_enum_data(hir, emit, id)
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

//@deduplicate with type_resolve_consteval 16.05.24
#[must_use]
pub fn type_resolve<'hir>(
    hir: &HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    ast_ty: ast::Type,
) -> hir::Type<'hir> {
    match ast_ty.kind {
        ast::TypeKind::Basic(basic) => hir::Type::Basic(basic),
        ast::TypeKind::Custom(path) => {
            super::pass_5::path_resolve_type(hir, emit, None, origin_id, path)
        }
        ast::TypeKind::Reference(ref_ty, mutt) => {
            let ref_ty = type_resolve(hir, emit, origin_id, *ref_ty);
            hir::Type::Reference(emit.arena.alloc(ref_ty), mutt)
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let mut param_types = Vec::with_capacity(proc_ty.param_types.len());
            for param_ty in proc_ty.param_types {
                let ty = type_resolve(hir, emit, origin_id, *param_ty);
                param_types.push(ty);
            }
            let param_types = emit.arena.alloc_slice(&param_types);

            let return_ty = if let Some(return_ty) = proc_ty.return_ty {
                type_resolve(hir, emit, origin_id, return_ty)
            } else {
                hir::Type::Basic(ast::BasicType::Void)
            };

            let proc_ty = hir::ProcType {
                param_types,
                is_variadic: proc_ty.is_variadic,
                return_ty,
            };
            hir::Type::Procedure(emit.arena.alloc(proc_ty))
        }
        ast::TypeKind::ArraySlice(slice) => {
            let elem_ty = type_resolve(hir, emit, origin_id, slice.elem_ty);

            let slice = hir::ArraySlice {
                mutt: slice.mutt,
                elem_ty,
            };
            hir::Type::ArraySlice(emit.arena.alloc(slice))
        }
        ast::TypeKind::ArrayStatic(array) => {
            let expect = Expectation::HasType(hir::Type::USIZE, None);
            let value = pass_4::resolve_const_expr(hir, emit, origin_id, expect, array.len);

            let len = match value {
                hir::ConstValue::Int { val, int_ty, neg } => {
                    if neg {
                        None
                    } else {
                        Some(val)
                    }
                }
                _ => None,
            };
            let elem_ty = type_resolve(hir, emit, origin_id, array.elem_ty);

            let array = hir::ArrayStatic {
                len: hir::ArrayStaticLen::Immediate(len),
                elem_ty,
            };
            hir::Type::ArrayStatic(emit.arena.alloc(array))
        }
    }
}

#[must_use]
pub fn type_resolve_delayed<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    ast_ty: ast::Type<'ast>,
) -> hir::Type<'hir> {
    match ast_ty.kind {
        ast::TypeKind::Basic(basic) => hir::Type::Basic(basic),
        ast::TypeKind::Custom(path) => {
            super::pass_5::path_resolve_type(hir, emit, None, origin_id, path)
        }
        ast::TypeKind::Reference(ref_ty, mutt) => {
            let ref_ty = type_resolve_delayed(hir, emit, origin_id, *ref_ty);
            hir::Type::Reference(emit.arena.alloc(ref_ty), mutt)
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let mut param_types = Vec::with_capacity(proc_ty.param_types.len());
            for param_ty in proc_ty.param_types {
                let ty = type_resolve_delayed(hir, emit, origin_id, *param_ty);
                param_types.push(ty);
            }
            let param_types = emit.arena.alloc_slice(&param_types);

            let return_ty = if let Some(return_ty) = proc_ty.return_ty {
                type_resolve_delayed(hir, emit, origin_id, return_ty)
            } else {
                hir::Type::Basic(ast::BasicType::Void)
            };

            let proc_ty = hir::ProcType {
                param_types,
                is_variadic: proc_ty.is_variadic,
                return_ty,
            };
            hir::Type::Procedure(emit.arena.alloc(proc_ty))
        }
        ast::TypeKind::ArraySlice(slice) => {
            let elem_ty = type_resolve_delayed(hir, emit, origin_id, slice.elem_ty);

            let slice = hir::ArraySlice {
                mutt: slice.mutt,
                elem_ty,
            };
            hir::Type::ArraySlice(emit.arena.alloc(slice))
        }
        ast::TypeKind::ArrayStatic(array) => {
            let len = hir.registry_mut().add_const_eval(array.len, origin_id);
            let elem_ty = type_resolve_delayed(hir, emit, origin_id, array.elem_ty);

            let array = hir::ArrayStatic {
                len: hir::ArrayStaticLen::ConstEval(len),
                elem_ty,
            };
            hir::Type::ArrayStatic(emit.arena.alloc(array))
        }
    }
}

pub fn process_proc_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::ProcID,
) {
    let item = hir.registry().proc_item(id);
    let origin_id = hir.registry().proc_data(id).origin_id;
    let mut unique = Vec::<hir::ProcParam>::new();

    for param in item.params.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == param.name.id) {
            emit.error(ErrorComp::new(
                format!(
                    "parameter `{}` is defined multiple times",
                    hir.name_str(param.name.id)
                ),
                SourceRange::new(origin_id, param.name.range),
                Info::new(
                    "existing parameter",
                    SourceRange::new(origin_id, existing.name.range),
                ),
            ));
        } else {
            let ty = type_resolve_delayed(hir, emit, origin_id, param.ty);
            pass_5::require_value_type(hir, emit, ty, SourceRange::new(origin_id, param.ty.range));

            unique.push(hir::ProcParam {
                mutt: param.mutt,
                name: param.name,
                ty,
            });
        }
    }

    hir.registry_mut().proc_data_mut(id).params = emit.arena.alloc_slice(&unique);
    hir.registry_mut().proc_data_mut(id).return_ty = if let Some(ret_ty) = item.return_ty {
        type_resolve_delayed(hir, emit, origin_id, ret_ty)
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
    let data = hir.registry().enum_data(id);
    let origin_id = hir.registry().enum_data(id).origin_id;
    let enum_name = data.name;

    let mut unique = Vec::<hir::EnumVariant>::new();

    for variant in item.variants.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == variant.name.id) {
            emit.error(ErrorComp::new(
                format!(
                    "variant `{}` is defined multiple times",
                    hir.name_str(variant.name.id)
                ),
                SourceRange::new(origin_id, variant.name.range),
                Info::new(
                    "existing variant",
                    SourceRange::new(origin_id, existing.name.range),
                ),
            ));
        } else {
            let kind = match variant.kind {
                ast::VariantKind::Default => {
                    let value_error = hir::ConstValue::Error;
                    hir::VariantKind::Default(value_error)
                }
                ast::VariantKind::Constant(value) => {
                    //@perf (same for array sizes) `fast` resolve and dont add the ConstEvalID?
                    let value = hir.registry_mut().add_const_eval(value, origin_id);
                    hir::VariantKind::Constant(value)
                }
                ast::VariantKind::HasValues(types) => {
                    //@temp buffer this allocation (applies for all allocs in hir_lower)
                    let mut variant_types = Vec::with_capacity(types.len());
                    for ty in types {
                        let ty = type_resolve_delayed(hir, emit, origin_id, *ty);
                        variant_types.push(ty);
                    }
                    let types = emit.arena.alloc_slice(&variant_types);
                    hir::VariantKind::HasValues(types)
                }
            };

            let variant = hir::EnumVariant {
                name: variant.name,
                kind,
            };
            unique.push(variant);
        }
    }

    let any_default = unique
        .iter()
        .any(|v| matches!(v.kind, hir::VariantKind::Default(_)));
    let any_constant = unique
        .iter()
        .any(|v| matches!(v.kind, hir::VariantKind::Constant(_)));

    let int_ty = if let Some((basic, basic_range)) = item.basic {
        // use defined integer type
        if let Some(int_ty) = hir::BasicInt::from_basic(basic) {
            int_ty
        } else {
            emit.error(ErrorComp::new(
                format!(
                    "enum type must be an integer, found `{}`\ndefault `s32` will be used",
                    basic.as_str()
                ),
                SourceRange::new(origin_id, basic_range),
                None,
            ));
            hir::BasicInt::S32
        }
    } else if any_constant {
        // require integer type
        emit.error(ErrorComp::new(
            "enum type must be specified, add it after the enum name\ndefault `s32` will be used",
            SourceRange::new(origin_id, enum_name.range),
            None,
        ));
        hir::BasicInt::S32
    } else {
        // infer type for Default | HasValues variants
        let variant_count = unique.len() as u64;
        if variant_count <= u8::MAX as u64 {
            hir::BasicInt::U8
        } else if variant_count <= u16::MAX as u64 {
            hir::BasicInt::U16
        } else if variant_count <= u32::MAX as u64 {
            hir::BasicInt::U32
        } else {
            hir::BasicInt::U64
        }
    };

    if any_constant && any_default {
        let mut info_vec = Vec::new();
        for variant in unique.iter() {
            if matches!(variant.kind, hir::VariantKind::Default(_)) {
                info_vec.push(Info::new_value(
                    "assign an explicit value",
                    SourceRange::new(origin_id, variant.name.range),
                ));
            }
        }
        emit.error(ErrorComp::new_detailed_info_vec(
            "default variants must have a value",
            "in this enum",
            SourceRange::new(origin_id, enum_name.range),
            info_vec,
        ));
    } else {
        //@tag for HasValues variants is not clear and can only be known after all constants (if any) were resolved
        // (with just Default | HasValues) it sequential (also HasValues tag value is not stored anywhere!)
        //@bounds check user defined type for Default | HasValues variants with their tag values
        for (idx, variant) in unique.iter_mut().enumerate() {
            if let hir::VariantKind::Default(value) = &mut variant.kind {
                *value = hir::ConstValue::Int {
                    val: idx as u64,
                    neg: false,
                    int_ty,
                };
            }
        }
    }

    let data = hir.registry_mut().enum_data_mut(id);
    data.int_ty = int_ty;
    data.variants = emit.arena.alloc_slice(&unique);
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
            emit.error(ErrorComp::new(
                format!(
                    "field `{}` is defined multiple times",
                    hir.name_str(field.name.id)
                ),
                SourceRange::new(origin_id, field.name.range),
                Info::new(
                    "existing field",
                    SourceRange::new(origin_id, existing.name.range),
                ),
            ));
        } else {
            let ty = type_resolve_delayed(hir, emit, origin_id, field.ty);
            pass_5::require_value_type(hir, emit, ty, SourceRange::new(origin_id, field.ty.range));

            unique.push(hir::StructField {
                vis: field.vis,
                name: field.name,
                ty,
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
    let origin_id = hir.registry().const_data(id).origin_id;
    let item = hir.registry().const_item(id);
    let ty = type_resolve_delayed(hir, emit, origin_id, item.ty);

    pass_5::require_value_type(hir, emit, ty, SourceRange::new(origin_id, item.ty.range));
    hir.registry_mut().const_data_mut(id).ty = ty;
}

fn process_global_data<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::GlobalID,
) {
    let origin_id = hir.registry().global_data(id).origin_id;
    let item = hir.registry().global_item(id);
    let ty = type_resolve_delayed(hir, emit, origin_id, item.ty);

    pass_5::require_value_type(hir, emit, ty, SourceRange::new(origin_id, item.ty.range));
    hir.registry_mut().global_data_mut(id).ty = ty;
}
