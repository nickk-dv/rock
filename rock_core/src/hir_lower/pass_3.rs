use super::constant;
use super::context::HirCtx;
use super::pass_5::Expectation;
use crate::ast;
use crate::error::{Error, ErrorSink, Info, SourceRange};
use crate::hir;
use crate::session::ModuleID;

pub fn process_items(ctx: &mut HirCtx) {
    for id in ctx.registry.proc_ids() {
        process_proc_data(ctx, id)
    }
    for id in ctx.registry.enum_ids() {
        process_enum_data(ctx, id)
    }
    for id in ctx.registry.struct_ids() {
        process_struct_data(ctx, id)
    }
    for id in ctx.registry.const_ids() {
        process_const_data(ctx, id)
    }
    for id in ctx.registry.global_ids() {
        process_global_data(ctx, id)
    }
}

//@deduplicate with type_resolve_delayed 16.05.24
#[must_use]
pub fn type_resolve<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    origin_id: ModuleID,
    ast_ty: ast::Type,
) -> hir::Type<'hir> {
    match ast_ty.kind {
        ast::TypeKind::Basic(basic) => hir::Type::Basic(basic),
        ast::TypeKind::Custom(path) => super::pass_5::path_resolve_type(ctx, origin_id, path),
        ast::TypeKind::Reference(mutt, ref_ty) => {
            let ref_ty = type_resolve(ctx, origin_id, *ref_ty);
            hir::Type::Reference(mutt, ctx.arena.alloc(ref_ty))
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let mut param_types = Vec::with_capacity(proc_ty.param_types.len());
            for param_ty in proc_ty.param_types {
                let ty = type_resolve(ctx, origin_id, *param_ty);
                param_types.push(ty);
            }
            let param_types = ctx.arena.alloc_slice(&param_types);

            let is_variadic = proc_ty.is_variadic;
            let return_ty = type_resolve(ctx, origin_id, proc_ty.return_ty);

            let proc_ty = hir::ProcType {
                param_types,
                is_variadic,
                return_ty,
            };
            hir::Type::Procedure(ctx.arena.alloc(proc_ty))
        }
        ast::TypeKind::ArraySlice(slice) => {
            let elem_ty = type_resolve(ctx, origin_id, slice.elem_ty);

            let slice = hir::ArraySlice {
                mutt: slice.mutt,
                elem_ty,
            };
            hir::Type::ArraySlice(ctx.arena.alloc(slice))
        }
        ast::TypeKind::ArrayStatic(array) => {
            let expect = Expectation::HasType(hir::Type::USIZE, None);
            let len_res = constant::resolve_const_expr(ctx, origin_id, expect, array.len);
            let elem_ty = type_resolve(ctx, origin_id, array.elem_ty);

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
            hir::Type::ArrayStatic(ctx.arena.alloc(array))
        }
    }
}

#[must_use]
pub fn type_resolve_delayed<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    origin_id: ModuleID,
    ast_ty: ast::Type<'ast>,
) -> hir::Type<'hir> {
    match ast_ty.kind {
        ast::TypeKind::Basic(basic) => hir::Type::Basic(basic),
        ast::TypeKind::Custom(path) => super::pass_5::path_resolve_type(ctx, origin_id, path),
        ast::TypeKind::Reference(mutt, ref_ty) => {
            let ref_ty = type_resolve_delayed(ctx, origin_id, *ref_ty);
            hir::Type::Reference(mutt, ctx.arena.alloc(ref_ty))
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let mut param_types = Vec::with_capacity(proc_ty.param_types.len());
            for param_ty in proc_ty.param_types {
                let ty = type_resolve_delayed(ctx, origin_id, *param_ty);
                param_types.push(ty);
            }
            let param_types = ctx.arena.alloc_slice(&param_types);

            let is_variadic = proc_ty.is_variadic;
            let return_ty = type_resolve_delayed(ctx, origin_id, proc_ty.return_ty);

            let proc_ty = hir::ProcType {
                param_types,
                is_variadic,
                return_ty,
            };
            hir::Type::Procedure(ctx.arena.alloc(proc_ty))
        }
        ast::TypeKind::ArraySlice(slice) => {
            let elem_ty = type_resolve_delayed(ctx, origin_id, slice.elem_ty);

            let slice = hir::ArraySlice {
                mutt: slice.mutt,
                elem_ty,
            };
            hir::Type::ArraySlice(ctx.arena.alloc(slice))
        }
        ast::TypeKind::ArrayStatic(array) => {
            let len = ctx.registry.add_const_eval(array.len, origin_id);
            let elem_ty = type_resolve_delayed(ctx, origin_id, array.elem_ty);

            let array = hir::ArrayStatic {
                len: hir::ArrayStaticLen::ConstEval(len),
                elem_ty,
            };
            hir::Type::ArrayStatic(ctx.arena.alloc(array))
        }
    }
}

pub fn process_proc_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::ProcID<'hir>) {
    let item = ctx.registry.proc_item(id);
    let origin_id = ctx.registry.proc_data(id).origin_id;
    let mut unique = Vec::<hir::Param>::new();

    for param in item.params.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == param.name.id) {
            ctx.emit.error(Error::new(
                format!(
                    "parameter `{}` is defined multiple times",
                    ctx.name_str(param.name.id)
                ),
                SourceRange::new(origin_id, param.name.range),
                Info::new(
                    "existing parameter",
                    SourceRange::new(origin_id, existing.name.range),
                ),
            ));
        } else {
            let ty_range = param.ty.range;
            let ty = type_resolve_delayed(ctx, origin_id, param.ty);

            let param = hir::Param {
                mutt: param.mutt,
                name: param.name,
                ty,
                ty_range,
            };
            unique.push(param);
        }
    }

    let return_ty = type_resolve_delayed(ctx, origin_id, item.return_ty);
    let data = ctx.registry.proc_data_mut(id);
    data.params = ctx.arena.alloc_slice(&unique);
    data.return_ty = return_ty;
}

fn process_enum_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::EnumID<'hir>) {
    let item = ctx.registry.enum_item(id);
    let data = ctx.registry.enum_data(id);

    let mut unique = Vec::<hir::Variant>::new();
    let mut tag_ty = data.tag_ty;
    let mut any_constant = false;
    let mut any_has_fields = false;
    let origin_id = data.origin_id;
    let enum_name = data.name;

    for variant in item.variants.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == variant.name.id) {
            ctx.emit.error(Error::new(
                format!(
                    "variant `{}` is defined multiple times",
                    ctx.name_str(variant.name.id)
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
                    let eval_id = ctx.registry.add_variant_eval();

                    hir::Variant {
                        name: variant.name,
                        kind: hir::VariantKind::Default(eval_id),
                        fields: &[],
                    }
                }
                ast::VariantKind::Constant(value) => {
                    let eval_id = ctx.registry.add_const_eval(value, origin_id);
                    any_constant = true;

                    hir::Variant {
                        name: variant.name,
                        kind: hir::VariantKind::Constant(eval_id),
                        fields: &[],
                    }
                }
                //@could be empty field list, error and dont set the `any_has_fields`
                ast::VariantKind::HasValues(types) => {
                    let eval_id = ctx.registry.add_variant_eval();
                    if !any_has_fields && types.len() > 0 {
                        any_has_fields = true;
                    }

                    let mut fields = Vec::with_capacity(types.len());
                    for ty in types {
                        let ty_range = ty.range;
                        let ty = type_resolve_delayed(ctx, origin_id, *ty);
                        fields.push(hir::VariantField { ty, ty_range });
                    }
                    let fields = ctx.arena.alloc_slice(&fields);

                    hir::Variant {
                        name: variant.name,
                        kind: hir::VariantKind::Default(eval_id),
                        fields,
                    }
                }
            };
            unique.push(variant);
        }
    }

    if any_has_fields {
        //@bypassing the attr_check set flag, for now no conflits are possible
        let data = ctx.registry.enum_data_mut(id);
        data.attr_set.set(hir::EnumFlag::HasFields);
    }

    if tag_ty.is_err() && any_constant {
        ctx.emit.error(Error::new(
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

        let data = ctx.registry.enum_data_mut(id);
        data.tag_ty = Ok(int_ty);
        tag_ty = Ok(int_ty);
    }

    if tag_ty.is_err() {
        for variant in unique.iter() {
            match variant.kind {
                hir::VariantKind::Default(eval_id) => {
                    let eval = ctx.registry.variant_eval_mut(eval_id);
                    *eval = hir::Eval::ResolvedError;
                }
                hir::VariantKind::Constant(eval_id) => {
                    let (eval, _) = ctx.registry.const_eval_mut(eval_id);
                    *eval = hir::Eval::ResolvedError;
                }
            }
        }
    }

    ctx.registry.enum_data_mut(id).variants = ctx.arena.alloc_slice(&unique);
}

fn process_struct_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::StructID<'hir>) {
    let item = ctx.registry.struct_item(id);
    let origin_id = ctx.registry.struct_data(id).origin_id;
    let mut unique = Vec::<hir::Field>::new();

    for field in item.fields.iter() {
        if let Some(existing) = unique.iter().find(|&it| it.name.id == field.name.id) {
            ctx.emit.error(Error::new(
                format!(
                    "field `{}` is defined multiple times",
                    ctx.name_str(field.name.id)
                ),
                SourceRange::new(origin_id, field.name.range),
                Info::new(
                    "existing field",
                    SourceRange::new(origin_id, existing.name.range),
                ),
            ));
        } else {
            let ty_range = field.ty.range;
            let ty = type_resolve_delayed(ctx, origin_id, field.ty);

            let field = hir::Field {
                vis: field.vis,
                name: field.name,
                ty,
                ty_range,
            };
            unique.push(field);
        }
    }

    ctx.registry.struct_data_mut(id).fields = ctx.arena.alloc_slice(&unique);
}

fn process_const_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::ConstID<'hir>) {
    let origin_id = ctx.registry.const_data(id).origin_id;
    let item = ctx.registry.const_item(id);
    let ty = type_resolve_delayed(ctx, origin_id, item.ty);
    ctx.registry.const_data_mut(id).ty = ty;
}

fn process_global_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::GlobalID<'hir>) {
    let origin_id = ctx.registry.global_data(id).origin_id;
    let item = ctx.registry.global_item(id);
    let ty = type_resolve_delayed(ctx, origin_id, item.ty);
    ctx.registry.global_data_mut(id).ty = ty;
}
