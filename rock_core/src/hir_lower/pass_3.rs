use super::check_directive;
use super::check_path;
use super::context::HirCtx;
use super::pass_4;
use super::pass_5::Expectation;
use super::scope::PolyScope;
use crate::ast;
use crate::errors as err;
use crate::hir;
use crate::support::BitSet;
use crate::text::TextRange;

pub fn process_items(ctx: &mut HirCtx) {
    ctx.registry.proc_ids().for_each(|id| process_proc_poly_params(ctx, id));
    ctx.registry.enum_ids().for_each(|id| process_enum_poly_params(ctx, id));
    ctx.registry.struct_ids().for_each(|id| process_struct_poly_params(ctx, id));

    ctx.registry.proc_ids().for_each(|id| process_proc_data(ctx, id));
    ctx.registry.enum_ids().for_each(|id| process_enum_data(ctx, id));
    ctx.registry.struct_ids().for_each(|id| process_struct_data(ctx, id));

    ctx.scope.poly = PolyScope::None;
    ctx.registry.const_ids().for_each(|id| process_const_data(ctx, id));
    ctx.registry.global_ids().for_each(|id| process_global_data(ctx, id));
}

fn process_proc_poly_params(ctx: &mut HirCtx, id: hir::ProcID) {
    ctx.scope.origin = ctx.registry.proc_data(id).origin_id;
    let item = ctx.registry.proc_item(id);
    if let Some(poly_params) = item.poly_params {
        let poly_params = process_polymorph_params(ctx, poly_params);
        ctx.registry.proc_data_mut(id).poly_params = poly_params;
    }
}

fn process_enum_poly_params(ctx: &mut HirCtx, id: hir::EnumID) {
    ctx.scope.origin = ctx.registry.enum_data(id).origin_id;
    let item = ctx.registry.enum_item(id);
    if let Some(poly_params) = item.poly_params {
        let poly_params = process_polymorph_params(ctx, poly_params);
        ctx.registry.enum_data_mut(id).poly_params = poly_params;
    }
}

fn process_struct_poly_params(ctx: &mut HirCtx, id: hir::StructID) {
    ctx.scope.origin = ctx.registry.struct_data(id).origin_id;
    let item = ctx.registry.struct_item(id);
    if let Some(poly_params) = item.poly_params {
        let poly_params = process_polymorph_params(ctx, poly_params);
        ctx.registry.struct_data_mut(id).poly_params = poly_params;
    }
}

fn process_proc_data(ctx: &mut HirCtx, id: hir::ProcID) {
    ctx.scope.origin = ctx.registry.proc_data(id).origin_id;
    ctx.scope.poly = PolyScope::Proc(id);
    ctx.cache.proc_params.clear();

    let item = ctx.registry.proc_item(id);
    let data = ctx.registry.proc_data(id);
    let mut flag_set = data.flag_set;
    let param_count = item.params.len();
    let skip_global_check = data.flag_set.contains(hir::ProcFlag::External)
        || data.flag_set.contains(hir::ProcFlag::Intrinsic);

    for (param_idx, param) in item.params.iter().enumerate() {
        if !skip_global_check
            && ctx
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

        let (ty, kind) = match param.kind {
            ast::ParamKind::Normal(ty) => (type_resolve(ctx, ty, true), hir::ParamKind::Normal),
            ast::ParamKind::Implicit(dir) => {
                match check_directive::check_param_directive(
                    ctx,
                    param_idx,
                    param_count,
                    &mut flag_set,
                    dir,
                ) {
                    Some(ty_kind) => ty_kind,
                    None => continue,
                }
            }
        };
        let ty_range = match param.kind {
            ast::ParamKind::Normal(ty) => ty.range,
            ast::ParamKind::Implicit(dir) => dir.range,
        };
        let param = hir::Param { mutt: param.mutt, name: param.name, ty, ty_range, kind };
        ctx.cache.proc_params.push(param);
    }

    let return_ty = type_resolve(ctx, item.return_ty, true);
    let data = ctx.registry.proc_data_mut(id);
    data.flag_set = flag_set;
    data.params = ctx.arena.alloc_slice(&ctx.cache.proc_params);
    data.return_ty = return_ty;
}

fn process_enum_data(ctx: &mut HirCtx, id: hir::EnumID) {
    ctx.scope.origin = ctx.registry.enum_data(id).origin_id;
    ctx.scope.poly = PolyScope::Enum(id);
    ctx.cache.enum_variants.clear();

    let item = ctx.registry.enum_item(id);
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
                let eval_id = ctx.registry.add_const_eval(value, ctx.scope.origin, ctx.scope.poly);
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

    // infer default enum tag size
    if tag_ty.is_unresolved() && !any_constant {
        let int_ty = match ctx.cache.enum_variants.len() {
            0..=0xFF => hir::IntType::U8,
            0..=0xFFFF => hir::IntType::U16,
            0..=0xFFFF_FFFF => hir::IntType::U32,
            _ => hir::IntType::U64,
        };
        tag_ty = hir::Eval::Resolved(int_ty);
    }

    // force enum tag type to be specified
    if tag_ty.is_unresolved() && any_constant {
        let enum_src = ctx.src(data.name.range);
        err::item_enum_unknown_tag_ty(&mut ctx.emit, enum_src);
        tag_ty = hir::Eval::ResolvedError;
    }

    // variant tags cannot be resolved without tag_ty
    if !tag_ty.is_resolved_ok() {
        for variant in ctx.cache.enum_variants.iter() {
            match variant.kind {
                hir::VariantKind::Default(eval_id) => {
                    *ctx.registry.variant_eval_mut(eval_id) = hir::Eval::ResolvedError;
                }
                hir::VariantKind::Constant(eval_id) => {
                    ctx.registry.const_eval_mut(eval_id).0 = hir::Eval::ResolvedError;
                }
            }
        }
    }

    let data = ctx.registry.enum_data_mut(id);
    data.variants = ctx.arena.alloc_slice(&ctx.cache.enum_variants);
    data.tag_ty = tag_ty;
}

fn process_struct_data(ctx: &mut HirCtx, id: hir::StructID) {
    ctx.scope.origin = ctx.registry.struct_data(id).origin_id;
    ctx.scope.poly = PolyScope::Struct(id);
    ctx.cache.struct_fields.clear();

    let item = ctx.registry.struct_item(id);
    let struct_vis = ctx.registry.struct_data(id).vis;

    for field in item.fields.iter() {
        let (config, vis) =
            check_directive::check_field_directives(ctx, field.dir_list, struct_vis);
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
            vis,
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
    ctx.scope.origin = ctx.registry.const_data(id).origin_id;
    let item = ctx.registry.const_item(id);
    if let Some(ty) = item.ty {
        let ty = type_resolve(ctx, ty, true);
        ctx.registry.const_data_mut(id).ty = Some(ty);
    }
}

fn process_global_data(ctx: &mut HirCtx, id: hir::GlobalID) {
    ctx.scope.origin = ctx.registry.global_data(id).origin_id;
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
    const_eval_len: bool,
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
        ast::TypeKind::Custom(path) => check_path::path_resolve_type(ctx, path, const_eval_len),
        ast::TypeKind::Reference(mutt, ref_ty) => {
            let ref_ty = type_resolve(ctx, *ref_ty, const_eval_len);
            if ref_ty.is_error() {
                hir::Type::Error
            } else {
                hir::Type::Reference(mutt, ctx.arena.alloc(ref_ty))
            }
        }
        ast::TypeKind::MultiReference(mutt, ref_ty) => {
            let ref_ty = type_resolve(ctx, *ref_ty, const_eval_len);
            if ref_ty.is_error() {
                hir::Type::Error
            } else {
                hir::Type::MultiReference(mutt, ctx.arena.alloc(ref_ty))
            }
        }
        ast::TypeKind::Procedure(proc_ty) => {
            let param_count = proc_ty.params.len();
            let mut flag_set = if let Some(directive) = proc_ty.directive {
                check_directive::check_proc_ty_directive(ctx, directive)
            } else {
                BitSet::empty()
            };
            let offset = ctx.cache.proc_ty_params.start();
            for (param_idx, param) in proc_ty.params.iter().enumerate() {
                let (ty, kind) = match *param {
                    ast::ParamKind::Normal(ty) => {
                        (type_resolve(ctx, ty, const_eval_len), hir::ParamKind::Normal)
                    }
                    ast::ParamKind::Implicit(dir) => {
                        match check_directive::check_param_directive(
                            ctx,
                            param_idx,
                            param_count,
                            &mut flag_set,
                            dir,
                        ) {
                            Some(ty_kind) => ty_kind,
                            None => continue,
                        }
                    }
                };
                let param = hir::ProcTypeParam { ty, kind };
                ctx.cache.proc_ty_params.push(param);
            }
            let params = ctx.cache.proc_ty_params.take(offset, &mut ctx.arena);
            let return_ty = type_resolve(ctx, proc_ty.return_ty, const_eval_len);

            let proc_ty = hir::ProcType { flag_set, params, return_ty };
            hir::Type::Procedure(ctx.arena.alloc(proc_ty))
        }
        ast::TypeKind::ArraySlice(slice) => {
            let elem_ty = type_resolve(ctx, slice.elem_ty, const_eval_len);

            if elem_ty.is_error() {
                hir::Type::Error
            } else {
                let mutt = slice.mutt;
                let slice = hir::ArraySlice { mutt, elem_ty };
                hir::Type::ArraySlice(ctx.arena.alloc(slice))
            }
        }
        ast::TypeKind::ArrayStatic(array) => {
            let enum_id = if let ast::ExprKind::Item { path, args_list } = array.len.0.kind {
                if args_list.is_none() {
                    //@check no field, no poly, default initialized (0..)
                    check_path::path_resolve_enum_opt(ctx, path, const_eval_len)
                } else {
                    None
                }
            } else {
                None
            };

            let elem_ty = type_resolve(ctx, array.elem_ty, const_eval_len);

            if let Some(enum_id) = enum_id {
                return if !check_enumerated_array_type(ctx, enum_id, array.len.0.range) {
                    hir::Type::Error
                } else if elem_ty.is_error() {
                    hir::Type::Error
                } else {
                    let array = hir::ArrayEnumerated { enum_id, elem_ty };
                    hir::Type::ArrayEnumerated(ctx.arena.alloc(array))
                };
            }

            let len = if const_eval_len {
                //@temp hack to immediately resolve some expression
                //will avoid hash/eq != between same array types in codegen
                if let ast::ExprKind::Lit { lit } = array.len.0.kind {
                    if let ast::Lit::Int(value) = lit {
                        hir::ArrayStaticLen::Immediate(value)
                    } else {
                        let eval_id = ctx.registry.add_const_eval(
                            array.len,
                            ctx.scope.origin,
                            ctx.scope.poly,
                        );
                        hir::ArrayStaticLen::ConstEval(eval_id)
                    }
                } else {
                    let eval_id =
                        ctx.registry.add_const_eval(array.len, ctx.scope.origin, ctx.scope.poly);
                    hir::ArrayStaticLen::ConstEval(eval_id)
                }
            } else {
                let (len_res, _) = pass_4::resolve_const_expr(ctx, Expectation::USIZE, array.len);
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

pub fn check_enumerated_array_type(
    ctx: &mut HirCtx,
    enum_id: hir::EnumID,
    error_range: TextRange,
) -> bool {
    let data = ctx.registry.enum_data(enum_id);
    let valid = data.poly_params.is_none()
        && !data.flag_set.contains(hir::EnumFlag::WithFields)
        && !data.flag_set.contains(hir::EnumFlag::WithConstantInit);
    if !valid {
        let src = ctx.src(error_range);
        let name = ctx.name(data.name.id);
        err::tycheck_cannot_use_for_enum_array(&mut ctx.emit, src, name);
    }
    valid
}
