use super::check_directive;
use super::check_path;
use super::constant;
use super::context::HirCtx;
use super::pass_5::Expectation;
use crate::ast;
use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::support::AsStr;
use crate::support::BitSet;

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

    ctx.scope.set_poly(None);
    for id in ctx.registry.const_ids() {
        process_const_data(ctx, id)
    }
    for id in ctx.registry.global_ids() {
        process_global_data(ctx, id)
    }
}

fn process_proc_data(ctx: &mut HirCtx, id: hir::ProcID) {
    ctx.scope.set_origin(ctx.registry.proc_data(id).origin_id);
    ctx.scope.set_poly(Some(hir::PolymorphDefID::Proc(id)));
    let item = ctx.registry.proc_item(id);

    if let Some(poly_params) = item.poly_params {
        let poly_params = process_polymorph_params(ctx, poly_params);
        ctx.registry.proc_data_mut(id).poly_params = poly_params;
    }
    ctx.cache.proc_params.clear();

    for param in item.params.iter() {
        let (ty, kind) = match param.kind {
            ast::ParamKind::Normal(ty) => (type_resolve(ctx, ty, true), hir::ParamKind::Normal),
            ast::ParamKind::Implicit(dir) => check_directive::check_param_directive(ctx, dir),
        };

        if ctx
            .scope
            .check_already_defined_global(param.name, ctx.session, &ctx.registry, &mut ctx.emit)
            .is_err()
        {
            continue;
        }

        let existing = ctx.cache.proc_params.iter().find(|&it| it.name.id == param.name.id);
        if let Some(existing) = existing {
            let param_src = ctx.src(param.name.range);
            let existing = ctx.src(existing.name.range);
            let name = ctx.name(param.name.id);
            err::item_param_already_defined(&mut ctx.emit, param_src, existing, name);
            continue;
        }

        let ty_range = match param.kind {
            ast::ParamKind::Normal(ty) => ty.range,
            ast::ParamKind::Implicit(dir) => dir.range,
        };
        let param = hir::Param { mutt: param.mutt, name: param.name, ty, ty_range, kind };
        ctx.cache.proc_params.push(param);
    }

    let param_count = ctx.cache.proc_params.len();
    for (idx, param) in ctx.cache.proc_params.iter_mut().enumerate() {
        if idx + 1 != param_count {
            if matches!(param.kind, hir::ParamKind::Variadic | hir::ParamKind::CVariadic) {
                let src = SourceRange::new(ctx.scope.origin(), param.name.range);
                let dir_name = param.kind.as_str();
                err::directive_param_must_be_last(&mut ctx.emit, src, dir_name);

                param.ty = hir::Type::Error;
                param.kind = hir::ParamKind::ErrorDirective;
            }
        }
    }

    //@not checking flag compatibility, its always valid to set
    //check if entire flag compat system could be removed?
    if let Some(last) = ctx.cache.proc_params.last() {
        let data = ctx.registry.proc_data_mut(id);

        match last.kind {
            hir::ParamKind::Variadic => {
                if data.flag_set.contains(hir::ProcFlag::External) {
                    let proc_src = ctx.src(item.name.range);
                    err::flag_proc_variadic_external(&mut ctx.emit, proc_src);
                }
            }
            hir::ParamKind::CVariadic => {
                if !data.flag_set.contains(hir::ProcFlag::External) {
                    let proc_src = ctx.src(item.name.range);
                    err::flag_proc_c_variadic_not_external(&mut ctx.emit, proc_src);
                } else {
                    data.flag_set.set(hir::ProcFlag::CVariadic);
                    if item.params.is_empty() {
                        let proc_src = ctx.src(item.name.range);
                        err::flag_proc_c_variadic_zero_params(&mut ctx.emit, proc_src);
                    }
                }
            }
            _ => {}
        }
    }

    let return_ty = type_resolve(ctx, item.return_ty, true);
    let data = ctx.registry.proc_data_mut(id);
    data.params = ctx.arena.alloc_slice(&ctx.cache.proc_params);
    data.return_ty = return_ty;
}

fn process_enum_data(ctx: &mut HirCtx, id: hir::EnumID) {
    ctx.scope.set_origin(ctx.registry.enum_data(id).origin_id);
    ctx.scope.set_poly(Some(hir::PolymorphDefID::Enum(id)));
    let item = ctx.registry.enum_item(id);

    if let Some(poly_params) = item.poly_params {
        let poly_params = process_polymorph_params(ctx, poly_params);
        ctx.registry.enum_data_mut(id).poly_params = poly_params;
    }
    ctx.cache.enum_variants.clear();

    let mut any_constant = false;

    for variant in item.variants.iter() {
        let config = check_directive::check_expect_config(ctx, variant.dir_list, "variants");
        if config.disabled() {
            continue;
        }

        let existing = ctx.cache.enum_variants.iter().find(|&it| it.name.id == variant.name.id);
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
            ast::VariantKind::HasFields(field_list) => {
                let eval_id = ctx.registry.add_variant_eval();

                if field_list.types.is_empty() {
                    let src = ctx.src(field_list.range);
                    err::item_variant_fields_empty(&mut ctx.emit, src);
                }

                let mut fields = Vec::with_capacity(field_list.types.len());
                for ty in field_list.types {
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
        ctx.cache.enum_variants.push(variant);
    }

    let data = ctx.registry.enum_data(id);
    let mut tag_ty = hir::Eval::Unresolved(());

    if let Some(tag) = item.tag_ty {
        if let Some(int_ty) = hir::IntType::from_basic(tag.basic) {
            tag_ty = hir::Eval::Resolved(int_ty);
        } else {
            let tag_src = ctx.src(tag.range);
            err::item_enum_non_int_tag_ty(&mut ctx.emit, tag_src);
            tag_ty = hir::Eval::ResolvedError;
        }
    }

    // when `tag_ty` is unknown and all fields are non constant:
    // perform default enum tag sizing: 0..<variant_count
    if tag_ty.is_unresolved() && !any_constant {
        let variant_count = ctx.cache.enum_variants.len() as u64;
        let int_ty = if variant_count <= u8::MAX as u64 {
            hir::IntType::U8
        } else if variant_count <= u16::MAX as u64 {
            hir::IntType::U16
        } else if variant_count <= u32::MAX as u64 {
            hir::IntType::U32
        } else {
            hir::IntType::U64
        };
        tag_ty = hir::Eval::Resolved(int_ty);
    }

    // when `tag_ty` is unknown and any field is constant:
    // force enum tag type to be specified
    if tag_ty.is_unresolved() && any_constant {
        let enum_src = ctx.src(data.name.range);
        err::item_enum_unknown_tag_ty(&mut ctx.emit, enum_src);
        tag_ty = hir::Eval::ResolvedError;
    }

    // when `tag_ty` is unknown: set all Evals to `ResolvedError`
    if !tag_ty.is_resolved_ok() {
        for variant in ctx.cache.enum_variants.iter() {
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
    data.variants = ctx.arena.alloc_slice(&ctx.cache.enum_variants);
    data.tag_ty = tag_ty;
}

fn process_struct_data(ctx: &mut HirCtx, id: hir::StructID) {
    ctx.scope.set_origin(ctx.registry.struct_data(id).origin_id);
    ctx.scope.set_poly(Some(hir::PolymorphDefID::Struct(id)));
    let item = ctx.registry.struct_item(id);

    if let Some(poly_params) = item.poly_params {
        let poly_params = process_polymorph_params(ctx, poly_params);
        ctx.registry.struct_data_mut(id).poly_params = poly_params;
    }
    ctx.cache.struct_fields.clear();

    let struct_vis = ctx.registry.struct_data(id).vis;

    //@process visibility directives, allow stronger vis only
    for field in item.fields.iter() {
        let config = check_directive::check_expect_config(ctx, field.dir_list, "fields");
        if config.disabled() {
            continue;
        }

        let existing = ctx.cache.struct_fields.iter().find(|&it| it.name.id == field.name.id);
        if let Some(existing) = existing {
            let field_src = ctx.src(field.name.range);
            let existing = ctx.src(existing.name.range);
            let name = ctx.name(field.name.id);
            err::item_field_already_defined(&mut ctx.emit, field_src, existing, name);
            continue;
        }

        let field = hir::Field {
            vis: struct_vis,
            name: field.name,
            ty: type_resolve(ctx, field.ty, true),
            ty_range: field.ty.range,
        };
        ctx.cache.struct_fields.push(field);
    }

    let data = ctx.registry.struct_data_mut(id);
    data.fields = ctx.arena.alloc_slice(&ctx.cache.struct_fields);
}

fn process_const_data(ctx: &mut HirCtx, id: hir::ConstID) {
    ctx.scope.set_origin(ctx.registry.const_data(id).origin_id);
    let item = ctx.registry.const_item(id);
    if let Some(ty) = item.ty {
        let ty = type_resolve(ctx, ty, true);
        ctx.registry.const_data_mut(id).ty = Some(ty);
    }
}

fn process_global_data(ctx: &mut HirCtx, id: hir::GlobalID) {
    ctx.scope.set_origin(ctx.registry.global_data(id).origin_id);
    let item = ctx.registry.global_item(id);
    let ty = type_resolve(ctx, item.ty, true);
    ctx.registry.global_data_mut(id).ty = ty;
}

fn process_polymorph_params<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    poly_params: &ast::PolymorphParams,
) -> Option<&'hir [ast::Name]> {
    if poly_params.names.is_empty() {
        let src = ctx.src(poly_params.range);
        err::item_poly_params_empty(&mut ctx.emit, src);
        return None;
    }
    ctx.cache.poly_param_names.clear();

    for param in poly_params.names.iter().copied() {
        if ctx
            .scope
            .check_already_defined_global(param, ctx.session, &ctx.registry, &mut ctx.emit)
            .is_err()
        {
            continue;
        };

        let existing = ctx.cache.poly_param_names.iter().find(|&it| it.id == param.id);
        if let Some(existing) = existing {
            let param_src = ctx.src(param.range);
            let existing = ctx.src(existing.range);
            let name = ctx.name(param.id);
            err::item_type_param_already_defined(&mut ctx.emit, param_src, existing, name);
            continue;
        }
        ctx.cache.poly_param_names.push(param);
    }

    Some(ctx.arena.alloc_slice(&ctx.cache.poly_param_names))
}

pub fn type_resolve<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    ast_ty: ast::Type<'ast>,
    in_definition: bool,
) -> hir::Type<'hir> {
    match ast_ty.kind {
        ast::TypeKind::Basic(basic) => match basic {
            ast::BasicType::Char => hir::Type::Char,
            ast::BasicType::Void => hir::Type::Void,
            ast::BasicType::Never => hir::Type::Never,
            ast::BasicType::Rawptr => hir::Type::Rawptr,
            ast::BasicType::S8 => hir::Type::Int(hir::IntType::S8),
            ast::BasicType::S16 => hir::Type::Int(hir::IntType::S16),
            ast::BasicType::S32 => hir::Type::Int(hir::IntType::S32),
            ast::BasicType::S64 => hir::Type::Int(hir::IntType::S64),
            ast::BasicType::Ssize => hir::Type::Int(hir::IntType::Ssize),
            ast::BasicType::U8 => hir::Type::Int(hir::IntType::U8),
            ast::BasicType::U16 => hir::Type::Int(hir::IntType::U16),
            ast::BasicType::U32 => hir::Type::Int(hir::IntType::U32),
            ast::BasicType::U64 => hir::Type::Int(hir::IntType::U64),
            ast::BasicType::Usize => hir::Type::Int(hir::IntType::Usize),
            ast::BasicType::F32 => hir::Type::Float(hir::FloatType::F32),
            ast::BasicType::F64 => hir::Type::Float(hir::FloatType::F64),
            ast::BasicType::Bool => hir::Type::Bool(hir::BoolType::Bool),
            ast::BasicType::Bool16 => hir::Type::Bool(hir::BoolType::Bool16),
            ast::BasicType::Bool32 => hir::Type::Bool(hir::BoolType::Bool32),
            ast::BasicType::Bool64 => hir::Type::Bool(hir::BoolType::Bool64),
            ast::BasicType::String => hir::Type::String(hir::StringType::String),
            ast::BasicType::CString => hir::Type::String(hir::StringType::CString),
        },
        ast::TypeKind::Custom(path) => check_path::path_resolve_type(ctx, path, in_definition),
        ast::TypeKind::Reference(mutt, ref_ty) => {
            let ref_ty = type_resolve(ctx, *ref_ty, in_definition);
            if ref_ty.is_error() {
                hir::Type::Error
            } else {
                hir::Type::Reference(mutt, ctx.arena.alloc(ref_ty))
            }
        }
        ast::TypeKind::MultiReference(mutt, ref_ty) => {
            let ref_ty = type_resolve(ctx, *ref_ty, in_definition);
            if ref_ty.is_error() {
                hir::Type::Error
            } else {
                hir::Type::MultiReference(mutt, ctx.arena.alloc(ref_ty))
            }
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let flag_set = if let Some(directive) = proc_ty.directive {
                check_directive::check_proc_ty_directive(ctx, directive)
            } else {
                BitSet::empty()
            };
            let offset = ctx.cache.types.start();
            for param in proc_ty.params {
                let ty = match param {
                    ast::ParamKind::Normal(ty) => type_resolve(ctx, *ty, in_definition),
                    ast::ParamKind::Implicit(directive) => hir::Type::Error,
                };
                ctx.cache.types.push(ty);
            }
            let param_types = ctx.cache.types.take(offset, &mut ctx.arena);
            let return_ty = type_resolve(ctx, proc_ty.return_ty, in_definition);

            let proc_ty = hir::ProcType { flag_set, param_types, return_ty };
            hir::Type::Procedure(ctx.arena.alloc(proc_ty))
        }
        ast::TypeKind::ArraySlice(slice) => {
            let elem_ty = type_resolve(ctx, slice.elem_ty, in_definition);

            if elem_ty.is_error() {
                hir::Type::Error
            } else {
                let mutt = slice.mutt;
                let slice = hir::ArraySlice { mutt, elem_ty };
                hir::Type::ArraySlice(ctx.arena.alloc(slice))
            }
        }
        ast::TypeKind::ArrayStatic(array) => {
            let elem_ty = type_resolve(ctx, array.elem_ty, in_definition);

            let len = if in_definition {
                let eval_id = ctx.registry.add_const_eval(array.len, ctx.scope.origin());
                hir::ArrayStaticLen::ConstEval(eval_id)
            } else {
                let (len_res, _) = constant::resolve_const_expr(ctx, Expectation::USIZE, array.len);
                match len_res {
                    Ok(value) => hir::ArrayStaticLen::Immediate(value.into_int_u64()),
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
