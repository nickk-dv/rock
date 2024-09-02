use super::constant;
use super::hir_build::{HirData, HirEmit};
use super::pass_5::{self, Expectation};
use super::proc_scope::ProcScope;
use crate::ast;
use crate::error::{ErrorComp, Info, SourceRange};
use crate::hir;
use crate::session::ModuleID;

pub fn process_items<'hir>(hir: &mut HirData<'hir, '_>, emit: &mut HirEmit<'hir>) {
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

//@deduplicate with type_resolve_delayed 16.05.24
#[must_use]
pub fn type_resolve<'hir>(
    hir: &HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    proc: &mut ProcScope<'hir, '_>,
    origin_id: ModuleID,
    ast_ty: ast::Type,
) -> hir::Type<'hir> {
    match ast_ty.kind {
        ast::TypeKind::Basic(basic) => hir::Type::Basic(basic),
        ast::TypeKind::Custom(path) => {
            super::pass_5::path_resolve_type(hir, emit, None, origin_id, path)
        }
        ast::TypeKind::Reference(ref_ty, mutt) => {
            let ref_ty = type_resolve(hir, emit, proc, origin_id, *ref_ty);
            hir::Type::Reference(emit.arena.alloc(ref_ty), mutt)
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let mut param_types = Vec::with_capacity(proc_ty.param_types.len());
            for param_ty in proc_ty.param_types {
                let ty = type_resolve(hir, emit, proc, origin_id, *param_ty);
                param_types.push(ty);
            }
            let param_types = emit.arena.alloc_slice(&param_types);

            let return_ty = if let Some(return_ty) = proc_ty.return_ty {
                type_resolve(hir, emit, proc, origin_id, return_ty)
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
            let elem_ty = type_resolve(hir, emit, proc, origin_id, slice.elem_ty);

            let slice = hir::ArraySlice {
                mutt: slice.mutt,
                elem_ty,
            };
            hir::Type::ArraySlice(emit.arena.alloc(slice))
        }
        ast::TypeKind::ArrayStatic(array) => {
            let expect = Expectation::HasType(hir::Type::USIZE, None);
            let len_res =
                constant::resolve_const_expr(hir, emit, proc, origin_id, expect, array.len);
            let elem_ty = type_resolve(hir, emit, proc, origin_id, array.elem_ty);

            let len = if let Ok(value) = len_res {
                match value {
                    hir::ConstValue::Int { val, .. } => Some(val),
                    _ => unreachable!(),
                }
            } else {
                None
            };

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
    hir: &mut HirData<'hir, 'ast>,
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
    hir: &mut HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::ProcID,
) {
    let item = hir.registry().proc_item(id);
    let origin_id = hir.registry().proc_data(id).origin_id;
    let mut unique = Vec::<hir::Param>::new();

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

            let param = hir::Param {
                mutt: param.mutt,
                name: param.name,
                ty,
            };
            unique.push(param);
        }
    }

    hir.registry_mut().proc_data_mut(id).params = emit.arena.alloc_slice(&unique);
    hir.registry_mut().proc_data_mut(id).return_ty = if let Some(ret_ty) = item.return_ty {
        type_resolve_delayed(hir, emit, origin_id, ret_ty)
    } else {
        hir::Type::Basic(ast::BasicType::Void)
    }
}

fn process_enum_data<'hir>(hir: &mut HirData<'hir, '_>, emit: &mut HirEmit<'hir>, id: hir::EnumID) {
    let item = hir.registry().enum_item(id);
    let data = hir.registry().enum_data(id);

    let mut unique = Vec::<hir::Variant>::new();
    let mut tag_ty = data.tag_ty;
    let mut any_constant = false;
    let origin_id = data.origin_id;
    let enum_name = data.name;

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
            let variant = match variant.kind {
                ast::VariantKind::Default => {
                    let eval = hir::Eval::Unresolved(());

                    hir::Variant {
                        name: variant.name,
                        kind: hir::VariantKind::Default(eval),
                        fields: &[],
                    }
                }
                ast::VariantKind::Constant(value) => {
                    let eval_id = hir.registry_mut().add_const_eval(value, origin_id);
                    any_constant = true;

                    hir::Variant {
                        name: variant.name,
                        kind: hir::VariantKind::Constant(eval_id),
                        fields: &[],
                    }
                }
                ast::VariantKind::HasValues(types) => {
                    let eval = hir::Eval::Unresolved(());

                    let mut fields = Vec::with_capacity(types.len());
                    for ty in types {
                        let ty = type_resolve_delayed(hir, emit, origin_id, *ty);
                        fields.push(ty);
                    }
                    let fields = emit.arena.alloc_slice(&fields);

                    hir::Variant {
                        name: variant.name,
                        kind: hir::VariantKind::Default(eval),
                        fields,
                    }
                }
            };
            unique.push(variant);
        }
    }

    if tag_ty.is_err() && any_constant {
        emit.error(ErrorComp::new(
            "enum type must be specified\nuse #[repr(<int_ty>)] or #[repr(C)] attribute",
            SourceRange::new(origin_id, enum_name.range),
            None,
        ));
    }

    if tag_ty.is_err() && !any_constant {
        let variant_count = unique.len() as u64;

        let int_ty = if variant_count <= u8::MAX as u64 {
            hir::BasicInt::U8
        } else if variant_count <= u16::MAX as u64 {
            hir::BasicInt::U16
        } else if variant_count <= u32::MAX as u64 {
            hir::BasicInt::U32
        } else {
            hir::BasicInt::U64
        };

        let data = hir.registry_mut().enum_data_mut(id);
        data.tag_ty = Ok(int_ty);
        tag_ty = Ok(int_ty);
    }

    if tag_ty.is_err() {
        for variant in unique.iter_mut() {
            match &mut variant.kind {
                hir::VariantKind::Default(value) => *value = hir::Eval::ResolvedError,
                hir::VariantKind::Constant(eval_id) => {
                    let (eval, _) = hir.registry_mut().const_eval_mut(*eval_id);
                    *eval = hir::Eval::ResolvedError;
                }
            }
        }
    }

    let data = hir.registry_mut().enum_data_mut(id);
    data.variants = emit.arena.alloc_slice(&unique);
}

fn process_struct_data<'hir>(
    hir: &mut HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::StructID,
) {
    let item = hir.registry().struct_item(id);
    let origin_id = hir.registry().struct_data(id).origin_id;
    let mut unique = Vec::<hir::Field>::new();

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

            let field = hir::Field {
                vis: field.vis,
                name: field.name,
                ty,
            };
            unique.push(field);
        }
    }

    hir.registry_mut().struct_data_mut(id).fields = emit.arena.alloc_slice(&unique);
}

fn process_const_data<'hir>(
    hir: &mut HirData<'hir, '_>,
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
    hir: &mut HirData<'hir, '_>,
    emit: &mut HirEmit<'hir>,
    id: hir::GlobalID,
) {
    let origin_id = hir.registry().global_data(id).origin_id;
    let item = hir.registry().global_item(id);
    let ty = type_resolve_delayed(hir, emit, origin_id, item.ty);

    pass_5::require_value_type(hir, emit, ty, SourceRange::new(origin_id, item.ty.range));
    hir.registry_mut().global_data_mut(id).ty = ty;
}
