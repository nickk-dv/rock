use super::attr_check;
use super::check_path;
use super::constant;
use super::context::HirCtx;
use super::pass_5::Expectation;
use crate::ast;
use crate::errors as err;
use crate::hir;

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

pub fn process_proc_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::ProcID<'hir>) {
    ctx.scope.set_origin(ctx.registry.proc_data(id).origin_id);
    let item = ctx.registry.proc_item(id);

    let mut unique = Vec::<hir::Param>::new();

    for param in item.params.iter() {
        //@its still added to scope during proc typecheck
        if ctx
            .scope
            .check_already_defined_global(param.name, ctx.session, &ctx.registry, &mut ctx.emit)
            .is_err()
        {
            continue;
        }

        let existing = unique.iter().find(|&it| it.name.id == param.name.id);
        if let Some(existing) = existing {
            let param_src = ctx.src(param.name.range);
            let existing = ctx.src(existing.name.range);
            let name = ctx.name(param.name.id);
            err::item_param_already_defined(&mut ctx.emit, param_src, existing, name);
            continue;
        }

        let param = hir::Param {
            mutt: param.mutt,
            name: param.name,
            ty: type_resolve(ctx, param.ty, true),
            ty_range: param.ty.range,
        };
        unique.push(param);
    }

    let params = ctx.arena.alloc_slice(&unique);
    let return_ty = type_resolve(ctx, item.return_ty, true);
    let data = ctx.registry.proc_data_mut(id);
    data.params = params;
    data.return_ty = return_ty;
}

fn process_enum_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::EnumID<'hir>) {
    ctx.scope.set_origin(ctx.registry.enum_data(id).origin_id);
    let item = ctx.registry.enum_item(id);

    let mut unique = Vec::<hir::Variant>::new();
    let mut any_constant = false;

    for variant in item.variants.iter() {
        let feedback = attr_check::check_attrs_enum_variant(ctx, variant.attrs);
        if feedback.cfg_state.disabled() {
            continue;
        }

        let existing = unique.iter().find(|&it| it.name.id == variant.name.id);
        if let Some(existing) = existing {
            let variant_src = ctx.src(variant.name.range);
            let existing = ctx.src(existing.name.range);
            let name = ctx.name(variant.name.id);
            err::item_variant_already_defined(&mut ctx.emit, variant_src, existing, name);
            continue;
        }

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
                let eval_id = ctx.registry.add_const_eval(value, ctx.scope.origin());
                any_constant = true;

                hir::Variant {
                    name: variant.name,
                    kind: hir::VariantKind::Constant(eval_id),
                    fields: &[],
                }
            }
            //@could be empty field list, error and dont set the `any_has_fields`
            ast::VariantKind::HasFields(types) => {
                let eval_id = ctx.registry.add_variant_eval();

                let mut fields = Vec::with_capacity(types.len());
                for ty in types {
                    let ty_range = ty.range;
                    let ty = type_resolve(ctx, *ty, true);
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

    let data = ctx.registry.enum_data(id);
    let mut tag_ty = Err(());

    // enum tag type gets a priority since in attr_check
    // repr_c cannot be applied if `with tag type` was set
    if let Some(tag) = item.tag_ty {
        if let Some(int_ty) = hir::BasicInt::from_basic(tag.basic) {
            tag_ty = Ok(int_ty);
        } else {
            let tag_src = ctx.src(tag.range);
            err::item_enum_non_int_tag_ty(&mut ctx.emit, tag_src);
        }
    } else if data.attr_set.contains(hir::EnumFlag::ReprC) {
        tag_ty = Ok(hir::BasicInt::S32);
    }

    //@use Eval so that tag_ty can be ResolvedError on invalid enum_tag_ty?
    // when `tag_ty` is unknown and all fields are non constant:
    // perform default enum tag sizing: 0..<variant_count
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
        tag_ty = Ok(int_ty);
    }

    // when `tag_ty` is unknown and any field is constant:
    // force enum tag repr to be specified via attribute
    if tag_ty.is_err() && any_constant {
        let enum_src = ctx.src(data.name.range);
        err::item_enum_unknown_tag_ty(&mut ctx.emit, enum_src);
    }

    // when `tag_ty` is unknown: set all Evals to `ResolvedError`
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

    let data = ctx.registry.enum_data_mut(id);
    data.variants = ctx.arena.alloc_slice(&unique);
    data.tag_ty = tag_ty;
}

fn process_struct_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::StructID<'hir>) {
    ctx.scope.set_origin(ctx.registry.struct_data(id).origin_id);
    let item = ctx.registry.struct_item(id);

    let mut unique = Vec::<hir::Field>::new();

    for field in item.fields.iter() {
        let feedback = attr_check::check_attrs_struct_field(ctx, field.attrs);
        if feedback.cfg_state.disabled() {
            continue;
        }

        let existing = unique.iter().find(|&it| it.name.id == field.name.id);
        if let Some(existing) = existing {
            let field_src = ctx.src(field.name.range);
            let existing = ctx.src(existing.name.range);
            let name = ctx.name(field.name.id);
            err::item_field_already_defined(&mut ctx.emit, field_src, existing, name);
            continue;
        }

        let field = hir::Field {
            vis: field.vis,
            name: field.name,
            ty: type_resolve(ctx, field.ty, true),
            ty_range: field.ty.range,
        };
        unique.push(field);
    }

    let fields = ctx.arena.alloc_slice(&unique);
    ctx.registry.struct_data_mut(id).fields = fields;
}

fn process_const_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::ConstID<'hir>) {
    ctx.scope.set_origin(ctx.registry.const_data(id).origin_id);
    let item = ctx.registry.const_item(id);

    let ty = type_resolve(ctx, item.ty, true);
    ctx.registry.const_data_mut(id).ty = ty;
}

fn process_global_data<'hir>(ctx: &mut HirCtx<'hir, '_, '_>, id: hir::GlobalID<'hir>) {
    ctx.scope.set_origin(ctx.registry.global_data(id).origin_id);
    let item = ctx.registry.global_item(id);

    let ty = type_resolve(ctx, item.ty, true);
    ctx.registry.global_data_mut(id).ty = ty;
}

#[must_use]
pub fn type_resolve<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    ast_ty: ast::Type<'ast>,
    delayed: bool,
) -> hir::Type<'hir> {
    match ast_ty.kind {
        ast::TypeKind::Basic(basic) => hir::Type::Basic(basic),
        ast::TypeKind::Custom(path) => check_path::path_resolve_type(ctx, path),
        ast::TypeKind::Generic(generic) => {
            let src = ctx.src(ast_ty.range);
            err::internal_generic_types_not_implemented(&mut ctx.emit, src);
            hir::Type::Error
        }
        ast::TypeKind::Reference(mutt, ref_ty) => {
            let ref_ty = type_resolve(ctx, *ref_ty, delayed);

            if ref_ty.is_error() {
                hir::Type::Error
            } else {
                hir::Type::Reference(mutt, ctx.arena.alloc(ref_ty))
            }
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let mut param_types = Vec::with_capacity(proc_ty.param_types.len());
            for param_ty in proc_ty.param_types {
                let ty = type_resolve(ctx, *param_ty, delayed);
                param_types.push(ty);
            }
            let param_types = ctx.arena.alloc_slice(&param_types);
            let is_variadic = proc_ty.is_variadic;
            let return_ty = type_resolve(ctx, proc_ty.return_ty, delayed);

            let proc_ty = hir::ProcType {
                param_types,
                is_variadic,
                return_ty,
            };
            hir::Type::Procedure(ctx.arena.alloc(proc_ty))
        }
        ast::TypeKind::ArraySlice(slice) => {
            let elem_ty = type_resolve(ctx, slice.elem_ty, delayed);

            if elem_ty.is_error() {
                hir::Type::Error
            } else {
                let mutt = slice.mutt;
                let slice = hir::ArraySlice { mutt, elem_ty };
                hir::Type::ArraySlice(ctx.arena.alloc(slice))
            }
        }
        ast::TypeKind::ArrayStatic(array) => {
            let elem_ty = type_resolve(ctx, array.elem_ty, delayed);

            let len = if delayed {
                let eval_id = ctx.registry.add_const_eval(array.len, ctx.scope.origin());
                hir::ArrayStaticLen::ConstEval(eval_id)
            } else {
                let expect = Expectation::HasType(hir::Type::USIZE, None);
                match constant::resolve_const_expr(ctx, expect, array.len) {
                    Ok(hir::ConstValue::Int { val, .. }) => hir::ArrayStaticLen::Immediate(val),
                    Ok(_) => unreachable!(),
                    Err(_) => return hir::Type::Error,
                }
            };

            if elem_ty.is_error() {
                hir::Type::Error
            } else {
                let array = hir::ArrayStatic { len, elem_ty };
                hir::Type::ArrayStatic(ctx.arena.alloc(array))
            }
        }
    }
}
