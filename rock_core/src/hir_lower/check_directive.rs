use super::context::HirCtx;
use crate::ast::{self, DirectiveKind};
use crate::config;
use crate::errors as err;
use crate::hir;
use crate::support::{AsStr, BitSet};

pub fn check_proc_directives(
    ctx: &mut HirCtx,
    item: &ast::ProcItem,
) -> (ConfigState, BitSet<hir::ProcFlag>) {
    let mut flag_set = BitSet::empty();
    let mut config = ConfigState(true);

    if item.block.is_none() {
        flag_set.set(hir::ProcFlag::External);
    }
    if item.is_variadic {
        if flag_set.contains(hir::ProcFlag::External) {
            flag_set.set(hir::ProcFlag::Variadic);
        } else {
            let proc_src = ctx.src(item.name.range);
            err::attr_proc_variadic_not_external(&mut ctx.emit, proc_src);
        }
    }
    if flag_set.contains(hir::ProcFlag::Variadic) && item.params.is_empty() {
        let proc_src = ctx.src(item.name.range);
        err::attr_proc_variadic_zero_params(&mut ctx.emit, proc_src);
    }

    for directive in item.directives {
        let flag = match directive.kind {
            DirectiveKind::Inline => hir::ProcFlag::Inline,
            DirectiveKind::Builtin => hir::ProcFlag::Builtin,
            DirectiveKind::Config(params) => {
                check_config_directive(ctx, &mut config, params);
                continue;
            }
            DirectiveKind::ConfigAny(params) => {
                check_config_any_directive(ctx, &mut config, params);
                continue;
            }
            DirectiveKind::ConfigNot(params) => {
                check_config_not_directive(ctx, &mut config, params);
                continue;
            }
            _ => {
                //@cannot apply error
                continue;
            }
        };
    }

    (config, flag_set)
}

#[derive(Copy, Clone)]
pub struct ConfigState(pub bool);

fn check_config_directive(
    ctx: &mut HirCtx,
    total: &mut ConfigState,
    params: &[ast::DirectiveParam],
) {
    let mut config = ConfigState(true);
    for param in params {
        if let Ok(state) = check_config_parameter(ctx, param) {
            config.0 = config.0 && state.0;
        }
    }
    total.0 = total.0 && config.0;
}

fn check_config_any_directive(
    ctx: &mut HirCtx,
    total: &mut ConfigState,
    params: &[ast::DirectiveParam],
) {
    let mut config = ConfigState(true);
    for param in params {
        if let Ok(state) = check_config_parameter(ctx, param) {
            config.0 = config.0 || state.0;
        }
    }
    total.0 = total.0 && config.0;
}

fn check_config_not_directive(
    ctx: &mut HirCtx,
    total: &mut ConfigState,
    params: &[ast::DirectiveParam],
) {
    let mut config = ConfigState(true);
    for param in params {
        if let Ok(state) = check_config_parameter(ctx, param) {
            config.0 = config.0 && !state.0;
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
        "target_ptr_width" => match config::TargetPtrWidth::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.target_ptr_width)),
            None => Err(()),
        },
        "build_kind" => match config::BuildKind::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.build_kind)),
            None => Err(()),
        },
        _ => {
            //@rename error function
            let param_src = ctx.src(param.name.range);
            err::attr_param_unknown(&mut ctx.emit, param_src, param_name);
            return Err(());
        }
    };

    if config.is_err() {
        //@rename error function
        let value_src = ctx.src(param.value_range);
        err::attr_param_value_unknown(&mut ctx.emit, value_src, param_name, param_value);
    }
    config
}
