use super::context::HirCtx;
use super::scope::{self, LocalVariableID, SymbolID, SymbolOrModule};
use crate::ast;
use crate::errors as err;
use crate::hir;
use crate::session::ModuleID;
use crate::text::TextRange;

struct PathResolved<'ast, 'hir> {
    kind: PathResolvedKind<'hir>,
    at_segment: ast::PathSegment<'ast>,
    segments: &'ast [ast::PathSegment<'ast>],
}

enum PathResolvedKind<'hir> {
    Symbol(SymbolID),
    Variable(LocalVariableID),
    Module(ModuleID),
    PolyParam(hir::Type<'hir>, TextRange),
}

pub enum ValueID<'ast, 'hir> {
    None,
    Proc(hir::ProcID, Option<&'hir [hir::Type<'hir>]>),
    Enum(hir::EnumID, hir::VariantID, Option<&'hir [hir::Type<'hir>]>),
    Const(hir::ConstID, &'ast [ast::PathSegment<'ast>]),
    Global(hir::GlobalID, &'ast [ast::PathSegment<'ast>]),
    Param(hir::ParamID, &'ast [ast::PathSegment<'ast>]),
    Variable(hir::VariableID, &'ast [ast::PathSegment<'ast>]),
}

fn path_resolve<'ast, 'hir>(
    ctx: &mut HirCtx,
    path: &ast::Path<'ast>,
) -> Result<PathResolved<'ast, 'hir>, ()> {
    let segment_0 = path.segments.first().copied().unwrap(); //path cannot be empty

    // <variable> | <poly_param> | <symbol> | <module>
    if let Some(var_id) = ctx.scope.local.find_variable(segment_0.name.id, true) {
        return Ok(PathResolved {
            kind: PathResolvedKind::Variable(var_id),
            at_segment: segment_0,
            segments: path.segments.split_at(1).1,
        });
    }
    if let Some((poly_ty, range)) = ctx.scope.poly.find_poly_param(segment_0.name.id, &ctx.registry)
    {
        return Ok(PathResolved {
            kind: PathResolvedKind::PolyParam(poly_ty, range),
            at_segment: segment_0,
            segments: path.segments.split_at(1).1,
        });
    }
    let symbol = ctx.scope.global.find_symbol(
        ctx.scope.origin,
        ctx.scope.origin,
        segment_0.name,
        ctx.session,
        &ctx.registry,
        &mut ctx.emit,
    )?;

    let target_id = match symbol {
        SymbolOrModule::Symbol(symbol_id) => {
            return Ok(PathResolved {
                kind: PathResolvedKind::Symbol(symbol_id),
                at_segment: segment_0,
                segments: path.segments.split_at(1).1,
            });
        }
        SymbolOrModule::Module(module_id) => {
            // module can be accessed by another segment, check poly args here
            check_unexpected_poly_args(ctx, &segment_0, "module")?;
            module_id
        }
    };

    // another <segment> | <module>
    let segment_1 = if let Some(segment) = path.segments.get(1).copied() {
        segment
    } else {
        return Ok(PathResolved {
            kind: PathResolvedKind::Module(target_id),
            at_segment: segment_0,
            segments: path.segments.split_at(1).1,
        });
    };

    let symbol = ctx.scope.global.find_symbol(
        ctx.scope.origin,
        target_id,
        segment_1.name,
        ctx.session,
        &ctx.registry,
        &mut ctx.emit,
    )?;

    match symbol {
        SymbolOrModule::Symbol(symbol_id) => Ok(PathResolved {
            kind: PathResolvedKind::Symbol(symbol_id),
            at_segment: segment_1,
            segments: path.segments.split_at(2).1,
        }),
        SymbolOrModule::Module(_) => unreachable!("module from other module"),
    }
}

fn check_unexpected_segments(
    ctx: &mut HirCtx,
    segments: &[ast::PathSegment],
    after_kind: &'static str,
) -> Result<(), ()> {
    if let Some(first) = segments.first().copied() {
        let start = first.name.range.start();
        let end = segments.last().unwrap().name.range.end();
        let src = ctx.src(TextRange::new(start, end));
        err::path_unexpected_segments(&mut ctx.emit, src, after_kind);
        Err(())
    } else {
        Ok(())
    }
}

fn check_unexpected_poly_args(
    ctx: &mut HirCtx,
    segment: &ast::PathSegment,
    after_kind: &'static str,
) -> Result<(), ()> {
    if let Some(poly_args) = segment.poly_args {
        let src = ctx.src(poly_args.range);
        err::path_unexpected_poly_args(&mut ctx.emit, src, after_kind);
        Err(())
    } else {
        Ok(())
    }
}

fn check_unexpected_parts_for_value(
    ctx: &mut HirCtx,
    path: &PathResolved,
    after_kind: &'static str,
) -> Result<(), ()> {
    check_unexpected_poly_args(ctx, &path.at_segment, after_kind)?;
    for field in path.segments {
        check_unexpected_poly_args(ctx, field, "field access")?;
    }
    Ok(())
}

pub fn path_resolve_type<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    path: &ast::Path<'ast>,
    const_eval_len: bool,
) -> hir::Type<'hir> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return hir::Type::Error,
    };
    if let PathResolvedKind::Symbol(symbol_id) = path.kind {
        set_symbol_used_flag(ctx, symbol_id);
    }

    let ty = match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            SymbolID::Enum(enum_id) => {
                let data = ctx.registry.enum_data(enum_id);
                let poly_types = resolve_type_poly_args(
                    ctx,
                    path.at_segment,
                    data.poly_params,
                    true,
                    const_eval_len,
                    data.name,
                    "enum",
                );
                match poly_types {
                    Some(poly_types) => hir::Type::Enum(enum_id, poly_types),
                    None => hir::Type::Error,
                }
            }
            SymbolID::Struct(struct_id) => {
                let data = ctx.registry.struct_data(struct_id);
                let poly_types = resolve_type_poly_args(
                    ctx,
                    path.at_segment,
                    data.poly_params,
                    true,
                    const_eval_len,
                    data.name,
                    "struct",
                );
                match poly_types {
                    Some(poly_types) => hir::Type::Struct(struct_id, poly_types),
                    None => hir::Type::Error,
                }
            }
            _ => {
                let src = ctx.src(path.at_segment.name.range);
                let defined_src = Some(symbol_id.src(&ctx.registry));
                let name = ctx.name(path.at_segment.name.id);
                #[rustfmt::skip]
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", symbol_id.desc());
                return hir::Type::Error;
            }
        },
        PathResolvedKind::Variable(var_id) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = Some(ctx.scope.var_src(var_id));
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", var_id.desc());
            return hir::Type::Error;
        }
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_segment.name.range);
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, None, name, "type", "module");
            return hir::Type::Error;
        }
        PathResolvedKind::PolyParam(poly_ty, _) => {
            if check_unexpected_poly_args(ctx, &path.at_segment, "type parameter").is_err() {
                return hir::Type::Error;
            }
            poly_ty
        }
    };

    if check_unexpected_segments(ctx, path.segments, "type").is_err() {
        return hir::Type::Error;
    }
    ty
}

pub fn path_resolve_enum_opt<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    path: &ast::Path<'ast>,
    const_eval_len: bool,
) -> Option<hir::EnumID> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return None,
    };
    if let PathResolvedKind::Symbol(symbol_id) = path.kind {
        set_symbol_used_flag(ctx, symbol_id);
    }

    let enum_id = match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            SymbolID::Enum(enum_id) => {
                let data = ctx.registry.enum_data(enum_id);
                let _ = resolve_type_poly_args(
                    ctx,
                    path.at_segment,
                    data.poly_params,
                    true,
                    const_eval_len,
                    data.name,
                    "enum",
                );
                Some(enum_id)
            }
            _ => return None,
        },
        _ => return None,
    };

    if check_unexpected_segments(ctx, path.segments, "enum type").is_err() {
        return None;
    }
    enum_id
}

fn resolve_type_poly_args<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    segment: ast::PathSegment<'ast>,
    poly_params: Option<&'hir [ast::Name]>,
    require_poly: bool,
    const_eval_len: bool,
    item_name: ast::Name,
    item_kind: &'static str,
) -> Option<&'hir [hir::Type<'hir>]> {
    match (poly_params, segment.poly_args) {
        (None, None) => Some(&[]),
        (None, Some(poly_args)) => {
            let src = ctx.src(poly_args.range);
            let name = ctx.name(item_name.id);
            err::path_type_unexpected_poly_args(&mut ctx.emit, src, name, item_kind);
            Some(&[])
        }
        (Some(_), None) => {
            if require_poly {
                let src = ctx.src(segment.name.range);
                let name = ctx.name(item_name.id);
                err::path_type_missing_poly_args(&mut ctx.emit, src, name, item_kind);
            }
            None
        }
        (Some(poly_params), Some(poly_args)) => {
            let input_count = poly_args.types.len();
            let expected_count = poly_params.len();

            if input_count != expected_count {
                let src = ctx.src(poly_args_range(poly_args));
                err::path_unexpected_poly_arg_count(
                    &mut ctx.emit,
                    src,
                    input_count,
                    expected_count,
                );
            }

            let mut error = false;
            let offset = ctx.cache.types.start();
            for idx in 0..poly_params.len() {
                if let Some(arg_type) = poly_args.types.get(idx) {
                    let ty = super::pass_3::type_resolve(ctx, *arg_type, const_eval_len);
                    ctx.cache.types.push(ty);
                } else {
                    error = true;
                };
            }

            if error {
                ctx.cache.types.pop_view(offset);
                None
            } else {
                Some(ctx.cache.types.take(offset, &mut ctx.arena))
            }
        }
    }
}

fn poly_args_range(poly_args: &ast::PolymorphArgs) -> TextRange {
    if poly_args.types.is_empty() {
        poly_args.range
    } else {
        let end = poly_args.range.end();
        TextRange::new(end - 1.into(), end)
    }
}

pub fn path_resolve_struct<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    path: &ast::Path<'ast>,
    const_eval_len: bool,
) -> Option<(hir::StructID, Option<&'hir [hir::Type<'hir>]>)> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return None,
    };
    if let PathResolvedKind::Symbol(symbol_id) = path.kind {
        set_symbol_used_flag(ctx, symbol_id);
    }

    let (struct_id, poly_types) = match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            SymbolID::Struct(struct_id) => {
                let data = ctx.registry.struct_data(struct_id);
                let poly_types = resolve_type_poly_args(
                    ctx,
                    path.at_segment,
                    data.poly_params,
                    false,
                    const_eval_len,
                    data.name,
                    "struct",
                );
                (struct_id, poly_types)
            }
            _ => {
                let src = ctx.src(path.at_segment.name.range);
                let defined_src = Some(symbol_id.src(&ctx.registry));
                let name = ctx.name(path.at_segment.name.id);
                #[rustfmt::skip]
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", symbol_id.desc());
                return None;
            }
        },
        PathResolvedKind::Variable(var_id) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = Some(ctx.scope.var_src(var_id));
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", var_id.desc());
            return None;
        }
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_segment.name.range);
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, None, name, "struct", "module");
            return None;
        }
        PathResolvedKind::PolyParam(_, range) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = Some(ctx.src(range));
            let name = ctx.name(path.at_segment.name.id);
            #[rustfmt::skip]
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", "type parameter");
            return None;
        }
    };

    if check_unexpected_segments(ctx, path.segments, "struct").is_err() {
        return None;
    }
    Some((struct_id, poly_types))
}

pub fn path_resolve_value<'hir, 'ast>(
    ctx: &mut HirCtx<'hir, 'ast, '_>,
    path: &ast::Path<'ast>,
    const_eval_len: bool,
) -> ValueID<'ast, 'hir> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return ValueID::None,
    };
    if let PathResolvedKind::Symbol(symbol_id) = path.kind {
        set_symbol_used_flag(ctx, symbol_id);
    }

    match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            SymbolID::Proc(proc_id) => {
                if check_unexpected_segments(ctx, path.segments, "procedure").is_err() {
                    ValueID::None
                } else {
                    let data = ctx.registry.proc_data(proc_id);
                    let poly_types = resolve_type_poly_args(
                        ctx,
                        path.at_segment,
                        data.poly_params,
                        false,
                        const_eval_len,
                        data.name,
                        "procedure",
                    );
                    ValueID::Proc(proc_id, poly_types)
                }
            }
            SymbolID::Enum(enum_id) => {
                if let Some(next) = path.segments.first().copied() {
                    if let Some(variant_id) =
                        scope::check_find_enum_variant(ctx, enum_id, next.name)
                    {
                        let names = path.segments.split_at(1).1;
                        if check_unexpected_segments(ctx, names, "enum variant").is_err() {
                            ValueID::None
                        } else {
                            let data = ctx.registry.enum_data(enum_id);
                            let poly_types = resolve_type_poly_args(
                                ctx,
                                path.at_segment,
                                data.poly_params,
                                false,
                                const_eval_len,
                                data.name,
                                "enum",
                            );
                            ValueID::Enum(enum_id, variant_id, poly_types)
                        }
                    } else {
                        ValueID::None
                    }
                } else {
                    let src = ctx.src(path.at_segment.name.range);
                    let defined_src = Some(symbol_id.src(&ctx.registry));
                    let name = ctx.name(path.at_segment.name.id);
                    err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "enum");
                    ValueID::None
                }
            }
            SymbolID::Struct(_) => {
                let src = ctx.src(path.at_segment.name.range);
                let defined_src = Some(symbol_id.src(&ctx.registry));
                let name = ctx.name(path.at_segment.name.id);
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "struct");
                ValueID::None
            }
            SymbolID::Const(id) => {
                if check_unexpected_parts_for_value(ctx, &path, "constant").is_err() {
                    return ValueID::None;
                }
                ValueID::Const(id, path.segments)
            }
            SymbolID::Global(id) => {
                if check_unexpected_parts_for_value(ctx, &path, "global").is_err() {
                    return ValueID::None;
                }
                ValueID::Global(id, path.segments)
            }
        },
        PathResolvedKind::Variable(local_var_id) => {
            if check_unexpected_parts_for_value(ctx, &path, local_var_id.desc()).is_err() {
                return ValueID::None;
            }
            match local_var_id {
                LocalVariableID::Param(id) => ValueID::Param(id, path.segments),
                LocalVariableID::Variable(id) => ValueID::Variable(id, path.segments),
            }
        }
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_segment.name.range);
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, None, name, "value", "module");
            ValueID::None
        }
        PathResolvedKind::PolyParam(_, range) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = Some(ctx.src(range));
            let name = ctx.name(path.at_segment.name.id);
            #[rustfmt::skip]
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "type parameter");
            ValueID::None
        }
    }
}

// `was_used` are set regardless of path resolve being valid.
// finding a symbol acts like a `usage` even in invalid context.
#[rustfmt::skip]
pub fn set_symbol_used_flag(ctx: &mut HirCtx, symbol_id: SymbolID) {
    match symbol_id {
        SymbolID::Proc(id) => ctx.registry.proc_data_mut(id).flag_set.set(hir::ProcFlag::WasUsed),
        SymbolID::Enum(id) => ctx.registry.enum_data_mut(id).flag_set.set(hir::EnumFlag::WasUsed),
        SymbolID::Struct(id) => ctx.registry.struct_data_mut(id).flag_set.set(hir::StructFlag::WasUsed),
        SymbolID::Const(id) => ctx.registry.const_data_mut(id).flag_set.set(hir::ConstFlag::WasUsed),
        SymbolID::Global(id) => ctx.registry.global_data_mut(id).flag_set.set(hir::GlobalFlag::WasUsed),
    }
}
