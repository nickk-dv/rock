use super::context::scope::{self, SymbolID, SymbolOrModule, VariableID};
use super::context::HirCtx;
use crate::ast;
use crate::errors as err;
use crate::hir;
use crate::session::ModuleID;
use crate::text::TextRange;

struct PathResolved<'ast> {
    kind: PathResolvedKind,
    at_name: ast::Name,
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
    let name = path.segments.get(0).copied().expect("non empty path");

    if let Some(var_id) = ctx.scope.local.find_variable(name.name.id) {
        return Ok(PathResolved {
            kind: PathResolvedKind::Variable(var_id),
            at_name: name.name,
            names: path.segments.split_at(1).1,
        });
    }

    if let Some((poly_def, poly_param_idx)) =
        ctx.scope.poly.find_poly_param(name.name.id, &ctx.registry)
    {
        return Ok(PathResolved {
            kind: PathResolvedKind::PolyParam(poly_def, poly_param_idx),
            at_name: name.name,
            names: path.segments.split_at(1).1,
        });
    }

    let symbol = ctx.scope.global.find_symbol(
        ctx.scope.origin(),
        ctx.scope.origin(),
        name.name,
        &ctx.session,
        &ctx.registry,
        &mut ctx.emit,
    )?;

    let target_id = match symbol {
        SymbolOrModule::Symbol(symbol_id) => {
            return Ok(PathResolved {
                kind: PathResolvedKind::Symbol(symbol_id),
                at_name: name.name,
                names: path.segments.split_at(1).1,
            })
        }
        SymbolOrModule::Module(module_id) => module_id,
    };

    let name = if let Some(name) = path.segments.get(1).copied() {
        name
    } else {
        return Ok(PathResolved {
            kind: PathResolvedKind::Module(target_id),
            at_name: name.name,
            names: path.segments.split_at(1).1,
        });
    };

    let symbol = ctx.scope.global.find_symbol(
        ctx.scope.origin(),
        target_id,
        name.name,
        &ctx.session,
        &ctx.registry,
        &mut ctx.emit,
    )?;

    match symbol {
        SymbolOrModule::Symbol(symbol_id) => Ok(PathResolved {
            kind: PathResolvedKind::Symbol(symbol_id),
            at_name: name.name,
            names: path.segments.split_at(2).1,
        }),
        SymbolOrModule::Module(_) => unreachable!("module from other module"),
    }
}

fn path_check_unexpected_segment(
    ctx: &mut HirCtx,
    names: &[ast::PathSegment],
    after_kind: &'static str,
) -> Result<(), ()> {
    if let Some(next) = names.first().copied() {
        let start = next.name.range.start();
        let end = names.last().unwrap().name.range.end();
        let src = ctx.src(TextRange::new(start, end));
        err::path_unexpected_segment(&mut ctx.emit, src, after_kind);
        Err(())
    } else {
        Ok(())
    }
}

pub fn path_resolve_type<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    path: &ast::Path,
) -> hir::Type<'hir> {
    let path = match path_resolve(ctx, path) {
        Ok(path) => path,
        Err(()) => return hir::Type::Error,
    };

    let ty = match path.kind {
        PathResolvedKind::Symbol(symbol_id) => match symbol_id {
            //@check input poly_args
            SymbolID::Enum(id) => hir::Type::Enum(id, None),
            SymbolID::Struct(id) => hir::Type::Struct(id, None),
            _ => {
                let src = ctx.src(path.at_name.range);
                let defined_src = symbol_id.src(&ctx.registry);
                let name = ctx.name(path.at_name.id);
                #[rustfmt::skip] //@set line len to like 120, to stop wrapping
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", symbol_id.desc());
                return hir::Type::Error;
            }
        },
        PathResolvedKind::Variable(var_id) => {
            let src = ctx.src(path.at_name.range);
            let defined_src = ctx.scope.var_src(var_id);
            let name = ctx.name(path.at_name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", var_id.desc());
            return hir::Type::Error;
        }
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_name.range);
            let defined_src = src; //@no src avaiblable, store imported src
            let name = ctx.name(path.at_name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "type", "module");
            return hir::Type::Error;
        }
        //@check input poly_args (not allowed)
        PathResolvedKind::PolyParam(poly_def, poly_param_idx) => {
            hir::Type::InferDef(poly_def, poly_param_idx)
        }
    };

    if path_check_unexpected_segment(ctx, path.names, "type").is_err() {
        return hir::Type::Error;
    }
    ty
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
                let src = ctx.src(path.at_name.range);
                let defined_src = symbol_id.src(&ctx.registry);
                let name = ctx.name(path.at_name.id);
                #[rustfmt::skip]
                err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", symbol_id.desc());
                return None;
            }
        },
        PathResolvedKind::Variable(var_id) => {
            let src = ctx.src(path.at_name.range);
            let defined_src = ctx.scope.var_src(var_id);
            let name = ctx.name(path.at_name.id);
            #[rustfmt::skip]
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", var_id.desc());
            return None;
        }
        PathResolvedKind::Module(_) => {
            let src = ctx.src(path.at_name.range);
            let defined_src = src; //@no src avaiblable, store imported src
            let name = ctx.name(path.at_name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "struct", "module");
            return None;
        }
        PathResolvedKind::PolyParam(poly_def, poly_param_idx) => {
            let src = ctx.src(path.at_name.range);
            let defined_src = ctx.src(ctx.poly_param_name(poly_def, poly_param_idx).range);
            let name = ctx.name(path.at_name.id);
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

    if path_check_unexpected_segment(ctx, path.names, "struct").is_err() {
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
                if path_check_unexpected_segment(ctx, path.names, "procedure").is_err() {
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
                        if path_check_unexpected_segment(ctx, names, "enum variant").is_err() {
                            ValueID::None
                        } else {
                            ValueID::Enum(enum_id, variant_id)
                        }
                    } else {
                        ValueID::None
                    }
                } else {
                    let src = ctx.src(path.at_name.range);
                    let defined_src = symbol_id.src(&ctx.registry);
                    let name = ctx.name(path.at_name.id);
                    err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "enum");
                    ValueID::None
                }
            }
            SymbolID::Struct(_) => {
                let src = ctx.src(path.at_name.range);
                let defined_src = symbol_id.src(&ctx.registry);
                let name = ctx.name(path.at_name.id);
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
            let src = ctx.src(path.at_name.range);
            let defined_src = src; //@no src avaiblable, store imported src
            let name = ctx.name(path.at_name.id);
            err::path_not_expected(&mut ctx.emit, src, defined_src, name, "value", "module");
            ValueID::None
        }
        PathResolvedKind::PolyParam(poly_def, poly_param_idx) => {
            let src = ctx.src(path.at_name.range);
            let defined_src = ctx.src(ctx.poly_param_name(poly_def, poly_param_idx).range);
            let name = ctx.name(path.at_name.id);
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
