use super::context::scope::{self, SymbolID, SymbolOrModule, VariableID};
use super::context::HirCtx;
use crate::ast;
use crate::errors as err;
use crate::hir;
use crate::session::ModuleID;
use crate::text::TextRange;

struct PathResolved<'ast> {
    kind: PathResolvedKind,
    at_segment: ast::PathSegment<'ast>,
    names: &'ast [ast::PathSegment<'ast>],
}

enum PathResolvedKind {
    Symbol(SymbolID),
    Variable(VariableID),
    Module(ModuleID),
    PolyParam(hir::PolymorphDefID, u32),
}

pub enum ValueID<'ast> {
    None,
    Proc(hir::ProcID),
    Enum(hir::EnumID, hir::VariantID),
    Const(hir::ConstID, &'ast [ast::PathSegment<'ast>]),
    Global(hir::GlobalID, &'ast [ast::PathSegment<'ast>]),
    Param(hir::ParamID, &'ast [ast::PathSegment<'ast>]),
    Local(hir::LocalID, &'ast [ast::PathSegment<'ast>]),
    LocalBind(hir::LocalBindID, &'ast [ast::PathSegment<'ast>]),
    ForBind(hir::ForBindID, &'ast [ast::PathSegment<'ast>]),
}

fn path_resolve<'ast>(ctx: &mut HirCtx, path: &ast::Path<'ast>) -> Result<PathResolved<'ast>, ()> {
    let segment_0 = path.segments.get(0).copied().expect("non empty path");

    // <variable> | <poly_def> | <symbol> | <module>
    if let Some(var_id) = ctx.scope.local.find_variable(segment_0.name.id) {
        return Ok(PathResolved {
            kind: PathResolvedKind::Variable(var_id),
            at_segment: segment_0,
            names: path.segments.split_at(1).1,
        });
    }
    if let Some((poly_def, poly_param_idx)) = ctx
        .scope
        .poly
        .find_poly_param(segment_0.name.id, &ctx.registry)
    {
        return Ok(PathResolved {
            kind: PathResolvedKind::PolyParam(poly_def, poly_param_idx),
            at_segment: segment_0,
            names: path.segments.split_at(1).1,
        });
    }
    let symbol = ctx.scope.global.find_symbol(
        ctx.scope.origin(),
        ctx.scope.origin(),
        segment_0.name,
        &ctx.session,
        &ctx.registry,
        &mut ctx.emit,
    )?;

    let target_id = match symbol {
        SymbolOrModule::Symbol(symbol_id) => {
            return Ok(PathResolved {
                kind: PathResolvedKind::Symbol(symbol_id),
                at_segment: segment_0,
                names: path.segments.split_at(1).1,
            });
        }
        SymbolOrModule::Module(module_id) => {
            // module can be accessed by another segment, check poly args here
            check_unexpected_poly_args(ctx, segment_0, "module")?;
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
            names: path.segments.split_at(1).1,
        });
    };

    let symbol = ctx.scope.global.find_symbol(
        ctx.scope.origin(),
        target_id,
        segment_1.name,
        &ctx.session,
        &ctx.registry,
        &mut ctx.emit,
    )?;

    match symbol {
        SymbolOrModule::Symbol(symbol_id) => Ok(PathResolved {
            kind: PathResolvedKind::Symbol(symbol_id),
            at_segment: segment_1,
            names: path.segments.split_at(2).1,
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
    segment: ast::PathSegment,
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

pub fn path_resolve_type<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    path: &ast::Path,
    require_poly: bool,
) -> hir::Type<'hir> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return hir::Type::Error,
    };

    let ty = match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            SymbolID::Enum(enum_id) => {
                let data = ctx.registry.enum_data(enum_id);
                let poly_types = resolve_type_poly_args(
                    ctx,
                    path.at_segment,
                    data.poly_params,
                    require_poly,
                    data.name,
                    "enum",
                );
                hir::Type::Enum(enum_id, poly_types)
            }
            SymbolID::Struct(struct_id) => {
                let data = ctx.registry.struct_data(struct_id);
                let poly_types = resolve_type_poly_args(
                    ctx,
                    path.at_segment,
                    data.poly_params,
                    require_poly,
                    data.name,
                    "struct",
                );
                hir::Type::Struct(struct_id, poly_types)
            }
            _ => {
                let src = ctx.src(path.at_segment.name.range);
                let defined_src = symbol_id.src(&ctx.registry);
                let name = ctx.name(path.at_segment.name.id);
                #[rustfmt::skip] //@set line len to like 120, to stop wrapping
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", symbol_id.desc());
                return hir::Type::Error;
            }
        },
        PathResolvedKind::Variable(var_id) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = ctx.scope.var_src(var_id);
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", var_id.desc());
            return hir::Type::Error;
        }
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = src; //@no src avaiblable, store imported src
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", "module");
            return hir::Type::Error;
        }
        PathResolvedKind::PolyParam(poly_def, poly_param_idx) => {
            if check_unexpected_poly_args(ctx, path.at_segment, "type parameter").is_err() {
                return hir::Type::Error;
            }
            hir::Type::InferDef(poly_def, poly_param_idx)
        }
    };

    if check_unexpected_segments(ctx, path.names, "type").is_err() {
        return hir::Type::Error;
    }
    ty
}

fn resolve_type_poly_args<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    segment: ast::PathSegment,
    poly_params: Option<&hir::PolymorphParams>,
    require_poly: bool,
    item_name: ast::Name,
    item_kind: &'static str,
) -> &'hir [hir::Type<'hir>] {
    match (poly_params, segment.poly_args) {
        (None, None) => &[],
        (None, Some(poly_args)) => {
            let src = ctx.src(poly_args.range);
            let name = ctx.name(item_name.id);
            err::path_type_unexpected_poly_args(&mut ctx.emit, src, name, item_kind);
            &[]
        }
        (Some(poly_params), None) => {
            if require_poly {
                let src = ctx.src(segment.name.range);
                let name = ctx.name(item_name.id);
                err::path_type_missing_poly_args(&mut ctx.emit, src, name, item_kind);
                ctx.arena
                    .alloc_slice_with_value(hir::Type::Error, poly_params.names.len())
            } else {
                //@use Type::Infer when its supported
                ctx.arena
                    .alloc_slice_with_value(hir::Type::Error, poly_params.names.len())
            }
        }
        (Some(poly_params), Some(poly_args)) => {
            //check inputs
            //@temp Type::Error
            ctx.arena
                .alloc_slice_with_value(hir::Type::Error, poly_params.names.len())
        }
    }
}

pub fn path_resolve_struct(ctx: &mut HirCtx, path: &ast::Path) -> Option<hir::StructID> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return None,
    };

    let struct_id = match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            //@check input poly_args
            SymbolID::Struct(struct_id) => struct_id,
            _ => {
                let src = ctx.src(path.at_segment.name.range);
                let defined_src = symbol_id.src(&ctx.registry);
                let name = ctx.name(path.at_segment.name.id);
                #[rustfmt::skip]
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", symbol_id.desc());
                return None;
            }
        },
        PathResolvedKind::Variable(var_id) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = ctx.scope.var_src(var_id);
            let name = ctx.name(path.at_segment.name.id);
            #[rustfmt::skip]
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", var_id.desc());
            return None;
        }
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = src; //@no src avaiblable, store imported src
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", "module");
            return None;
        }
        PathResolvedKind::PolyParam(poly_def, poly_param_idx) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = ctx.src(ctx.poly_param_name(poly_def, poly_param_idx).range);
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(
                &mut ctx.emit,
                src,
                defined_src,
                name,
                "struct",
                "type parameter",
            );
            return None;
        }
    };

    if check_unexpected_segments(ctx, path.names, "struct").is_err() {
        return None;
    }
    Some(struct_id)
}

pub fn path_resolve_value<'ast>(
    ctx: &mut HirCtx<'_, 'ast, '_>,
    path: &ast::Path<'ast>,
) -> ValueID<'ast> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return ValueID::None,
    };

    match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            SymbolID::Proc(proc_id) => {
                if check_unexpected_segments(ctx, path.names, "procedure").is_err() {
                    ValueID::None
                } else {
                    ValueID::Proc(proc_id)
                }
            }
            //@obtain input poly_args from path_resolve
            SymbolID::Enum(enum_id) => {
                if let Some(next) = path.names.first().copied() {
                    if let Some(variant_id) =
                        scope::check_find_enum_variant(ctx, enum_id, next.name)
                    {
                        let names = path.names.split_at(1).1;
                        if check_unexpected_segments(ctx, names, "enum variant").is_err() {
                            ValueID::None
                        } else {
                            ValueID::Enum(enum_id, variant_id)
                        }
                    } else {
                        ValueID::None
                    }
                } else {
                    let src = ctx.src(path.at_segment.name.range);
                    let defined_src = symbol_id.src(&ctx.registry);
                    let name = ctx.name(path.at_segment.name.id);
                    err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "enum");
                    ValueID::None
                }
            }
            SymbolID::Struct(_) => {
                let src = ctx.src(path.at_segment.name.range);
                let defined_src = symbol_id.src(&ctx.registry);
                let name = ctx.name(path.at_segment.name.id);
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "struct");
                ValueID::None
            }
            SymbolID::Const(id) => ValueID::Const(id, path.names),
            SymbolID::Global(id) => ValueID::Global(id, path.names),
        },
        PathResolvedKind::Variable(var_id) => match var_id {
            VariableID::Param(id) => ValueID::Param(id, path.names),
            VariableID::Local(id) => ValueID::Local(id, path.names),
            VariableID::Bind(id) => ValueID::LocalBind(id, path.names),
            VariableID::ForBind(id) => ValueID::ForBind(id, path.names),
        },
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = src; //@no src avaiblable, store imported src
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "module");
            ValueID::None
        }
        PathResolvedKind::PolyParam(poly_def, poly_param_idx) => {
            let src = ctx.src(path.at_segment.name.range);
            let defined_src = ctx.src(ctx.poly_param_name(poly_def, poly_param_idx).range);
            let name = ctx.name(path.at_segment.name.id);
            err::path_not_expected(
                &mut ctx.emit,
                src,
                defined_src,
                name,
                "value",
                "type parameter",
            );
            ValueID::None
        }
    }
}
