use super::context::HirCtx;
use crate::ast::{self, DirectiveKind};
use crate::errors as err;
use crate::hir;
use crate::session::{self, config};
use crate::support::{AsStr, BitSet};

#[derive(Copy, Clone)]
pub struct ConfigState(bool);

impl ConfigState {
    #[inline]
    pub fn disabled(self) -> bool {
        !self.0
    }
}

pub fn check_proc_directives(
    ctx: &mut HirCtx,
    item: &ast::ProcItem,
) -> (ConfigState, BitSet<hir::ProcFlag>) {
    let mut config = ConfigState(true);
    let mut flag_set = BitSet::empty();

    for directive in item.dir_list.map_or([].as_slice(), |l| l.directives) {
        if try_check_error_directive(ctx, directive) {
            continue;
        }
        if try_check_config_directive(ctx, &mut config, directive) {
            continue;
        }
        let new_flag = match directive.kind {
            DirectiveKind::InlineNever => {
                if flag_set.contains(hir::ProcFlag::InlineAlways) {
                    let src = ctx.src(directive.range);
                    err::flag_proc_conflict(
                        &mut ctx.emit,
                        src,
                        hir::ProcFlag::InlineNever.as_str(),
                        hir::ProcFlag::InlineAlways.as_str(),
                    );
                    continue;
                }
                hir::ProcFlag::InlineNever
            }
            DirectiveKind::InlineAlways => {
                if flag_set.contains(hir::ProcFlag::InlineNever) {
                    let src = ctx.src(directive.range);
                    err::flag_proc_conflict(
                        &mut ctx.emit,
                        src,
                        hir::ProcFlag::InlineAlways.as_str(),
                        hir::ProcFlag::InlineNever.as_str(),
                    );
                    continue;
                }
                hir::ProcFlag::InlineAlways
            }
            DirectiveKind::Intrinsic => {
                let module = ctx.session.module.get(ctx.scope.origin);
                if module.origin == session::CORE_PACKAGE_ID {
                    hir::ProcFlag::Intrinsic
                } else {
                    let src = ctx.src(directive.range);
                    err::flag_proc_intrinsic_non_core(&mut ctx.emit, src);
                    continue;
                }
            }
            _ => {
                let src = ctx.src(directive.range);
                let name = directive.kind.as_str();
                err::directive_cannot_apply(&mut ctx.emit, src, name, "procedures");
                continue;
            }
        };
        if flag_set.contains(new_flag) {
            let src = ctx.src(directive.range);
            err::directive_duplicate(&mut ctx.emit, src, directive.kind.as_str());
        } else {
            flag_set.set(new_flag);
        }
    }

    if item.block.is_none() && !flag_set.contains(hir::ProcFlag::Intrinsic) {
        flag_set.set(hir::ProcFlag::External);
    }
    (config, flag_set)
}

pub fn check_enum_directives(
    ctx: &mut HirCtx,
    item: &ast::EnumItem,
) -> (ConfigState, BitSet<hir::EnumFlag>) {
    let mut config = ConfigState(true);
    let mut flag_set = BitSet::empty();

    for variant in item.variants {
        if let ast::VariantKind::HasFields(_) = variant.kind {
            flag_set.set(hir::EnumFlag::WithFields);
            break;
        }
    }
    for variant in item.variants {
        if let ast::VariantKind::Constant(_) = variant.kind {
            flag_set.set(hir::EnumFlag::WithConstantInit);
            break;
        }
    }

    for directive in item.dir_list.map_or([].as_slice(), |l| l.directives) {
        if try_check_error_directive(ctx, directive) {
            continue;
        }
        if try_check_config_directive(ctx, &mut config, directive) {
            continue;
        }
        let src = ctx.src(directive.range);
        let name = directive.kind.as_str();
        err::directive_cannot_apply(&mut ctx.emit, src, name, "enums");
    }

    (config, flag_set)
}

pub fn check_param_directive<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    param_idx: usize,
    param_count: usize,
    flag_set: &mut BitSet<hir::ProcFlag>,
    directive: &ast::Directive,
) -> Option<(hir::Type<'hir>, hir::ParamKind)> {
    if try_check_error_directive(ctx, directive) {
        return Some((hir::Type::Error, hir::ParamKind::Normal));
    }
    match directive.kind {
        DirectiveKind::Variadic => {
            if param_idx + 1 != param_count {
                let src = ctx.src(directive.range);
                err::directive_param_must_be_last(&mut ctx.emit, src, directive.kind.as_str());
                return None;
            }
            if flag_set.contains(hir::ProcFlag::External) {
                let src = ctx.src(directive.range);
                err::flag_proc_variadic_external(&mut ctx.emit, src);
                return None;
            }
            flag_set.set(hir::ProcFlag::Variadic);
            let elem_ty = ctx.core.any.map_or(hir::Type::Error, |id| hir::Type::Struct(id, &[]));
            let slice = hir::ArraySlice { mutt: ast::Mut::Immutable, elem_ty };
            Some((hir::Type::ArraySlice(ctx.arena.alloc(slice)), hir::ParamKind::Variadic))
        }
        DirectiveKind::CVariadic => {
            if param_idx + 1 != param_count {
                let src = ctx.src(directive.range);
                err::directive_param_must_be_last(&mut ctx.emit, src, directive.kind.as_str());
                return None;
            }
            if !flag_set.contains(hir::ProcFlag::External) {
                let src = ctx.src(directive.range);
                err::flag_proc_c_variadic_not_external(&mut ctx.emit, src);
                return None;
            }
            if param_count == 1 {
                let src = ctx.src(directive.range);
                err::flag_proc_c_variadic_zero_params(&mut ctx.emit, src);
                return None;
            }
            flag_set.set(hir::ProcFlag::CVariadic);
            None
        }
        DirectiveKind::CallerLocation => {
            let ty = ctx.core.source_loc.map_or(hir::Type::Error, |id| hir::Type::Struct(id, &[]));
            Some((ty, hir::ParamKind::CallerLocation))
        }
        _ => {
            let src = ctx.src(directive.range);
            let name = directive.kind.as_str();
            err::directive_cannot_apply(&mut ctx.emit, src, name, "parameters");
            Some((hir::Type::Error, hir::ParamKind::Normal))
        }
    }
}

pub fn check_field_directives(
    ctx: &mut HirCtx,
    dir_list: Option<&ast::DirectiveList>,
    struct_vis: hir::Vis,
) -> (ConfigState, hir::Vis) {
    let mut config = ConfigState(true);
    let mut vis = struct_vis;

    let directives = if let Some(dir_list) = dir_list {
        dir_list.directives
    } else {
        return (config, vis);
    };

    for directive in directives {
        if try_check_error_directive(ctx, directive) {
            continue;
        }
        if try_check_config_directive(ctx, &mut config, directive) {
            continue;
        }
        let scope_vis = match directive.kind {
            DirectiveKind::ScopePublic => hir::Vis::Public,
            DirectiveKind::ScopePackage => hir::Vis::Package,
            DirectiveKind::ScopePrivate => hir::Vis::Private,
            _ => {
                let src = ctx.src(directive.range);
                let name = directive.kind.as_str();
                err::directive_cannot_apply(&mut ctx.emit, src, name, "fields");
                continue;
            }
        };
        if scope_vis == vis {
            let src = ctx.src(directive.range);
            err::directive_scope_vis_redundant(&mut ctx.emit, src, None, scope_vis.as_str());
        } else if !scope_vis.stronger(struct_vis) {
            let src = ctx.src(directive.range);
            err::directive_field_vis_not_stronger(&mut ctx.emit, src, struct_vis.as_str());
        } else {
            vis = scope_vis;
        }
    }
    (config, vis)
}

pub fn check_proc_ty_directive(
    ctx: &mut HirCtx,
    directive: &ast::Directive,
) -> BitSet<hir::ProcFlag> {
    if try_check_error_directive(ctx, directive) {
        return BitSet::empty();
    }
    match directive.kind {
        DirectiveKind::CCall => {
            let mut flag_set = BitSet::empty();
            flag_set.set(hir::ProcFlag::External);
            flag_set
        }
        _ => {
            let src = ctx.src(directive.range);
            let name = directive.kind.as_str();
            err::directive_cannot_apply(&mut ctx.emit, src, name, "procedure types");
            BitSet::empty()
        }
    }
}

pub fn check_expect_config(
    ctx: &mut HirCtx,
    dir_list: Option<&ast::DirectiveList>,
    item_kinds: &'static str,
) -> ConfigState {
    let mut config = ConfigState(true);

    let directives = if let Some(dir_list) = dir_list {
        dir_list.directives
    } else {
        return config;
    };

    for directive in directives {
        if try_check_error_directive(ctx, directive) {
            continue;
        }
        if try_check_config_directive(ctx, &mut config, directive) {
            continue;
        }
        let src = ctx.src(directive.range);
        let name = directive.kind.as_str();
        err::directive_cannot_apply(&mut ctx.emit, src, name, item_kinds);
    }
    config
}

pub fn try_check_error_directive(ctx: &mut HirCtx, directive: &ast::Directive) -> bool {
    match directive.kind {
        DirectiveKind::Error(name) => {
            let src = ctx.src(directive.range);
            let name = ctx.name(name.id);
            err::directive_unknown(&mut ctx.emit, src, name);
            true
        }
        _ => false,
    }
}

fn try_check_config_directive(
    ctx: &mut HirCtx,
    total: &mut ConfigState,
    directive: &ast::Directive,
) -> bool {
    match directive.kind {
        DirectiveKind::Config(params) => {
            check_config_directive(ctx, total, params, |a, b| a && b);
        }
        DirectiveKind::ConfigAny(params) => {
            check_config_directive(ctx, total, params, |a, b| a || b);
        }
        DirectiveKind::ConfigNot(params) => {
            check_config_directive(ctx, total, params, |a, b| a && !b)
        }
        _ => return false,
    }
    true
}

fn check_config_directive<F: Fn(bool, bool) -> bool>(
    ctx: &mut HirCtx,
    total: &mut ConfigState,
    params: &[ast::DirectiveParam],
    combine: F,
) {
    let mut config = ConfigState(true);
    for param in params {
        if let Ok(state) = check_config_parameter(ctx, param) {
            config.0 = combine(config.0, state.0);
        }
    }
    total.0 = total.0 && config.0;
}

fn check_config_parameter(
    ctx: &mut HirCtx,
    param: &ast::DirectiveParam,
) -> Result<ConfigState, ()> {
    let param_name = ctx.name(param.name.id);
    let param_value = ctx.session.intern_lit.get(param.value);

    let config = match param_name {
        "target" => match config::TargetTriple::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.target)),
            None => Err(()),
        },
        "target_os" => match config::TargetOS::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.target_os)),
            None => Err(()),
        },
        "target_arch" => match config::TargetArch::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.target_arch)),
            None => Err(()),
        },
        "build" => match config::Build::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.build)),
            None => Err(()),
        },
        _ => {
            let param_src = ctx.src(param.name.range);
            err::directive_param_unknown(&mut ctx.emit, param_src, param_name);
            return Err(());
        }
    };

    if config.is_err() {
        let value_src = ctx.src(param.value_range);
        err::directive_param_value_unknown(&mut ctx.emit, value_src, param_name, param_value);
    }
    config
}
