use super::context::HirCtx;
use crate::ast::{self, DirectiveKind};
use crate::config;
use crate::error::SourceRange;
use crate::errors as err;
use crate::hir;
use crate::support::{AsStr, BitSet};

#[derive(Copy, Clone)]
pub struct ConfigState(pub bool);

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

    if item.block.is_none() {
        flag_set.set(hir::ProcFlag::External);
    }
    if item.is_variadic {
        if flag_set.contains(hir::ProcFlag::External) {
            flag_set.set(hir::ProcFlag::Variadic);
        } else {
            let proc_src = ctx.src(item.name.range);
            err::flag_proc_variadic_not_external(&mut ctx.emit, proc_src);
        }
    }
    if flag_set.contains(hir::ProcFlag::Variadic) && item.params.is_empty() {
        let proc_src = ctx.src(item.name.range);
        err::flag_proc_variadic_zero_params(&mut ctx.emit, proc_src);
    }

    for directive in item.directives {
        if try_check_unknown_directive(ctx, directive) {
            continue;
        }
        if try_check_config_directive(ctx, &mut config, directive) {
            continue;
        }
        let new_flag = match directive.kind {
            DirectiveKind::Inline => hir::ProcFlag::Inline,
            DirectiveKind::Builtin => hir::ProcFlag::Builtin,
            _ => {
                let src = ctx.src(directive.range);
                let name = directive.kind.as_str();
                err::directive_cannot_apply(&mut ctx.emit, src, name, "procedures");
                continue;
            }
        };
        apply_item_flag(
            ctx,
            &mut flag_set,
            new_flag,
            ctx.src(item.name.range),
            Some(directive),
            "procedures",
        );
    }

    if flag_set.contains(hir::ProcFlag::Builtin) {
        if let Some(block) = item.block {
            let proc_src = ctx.src(item.name.range);
            let block_src = ctx.src(block.range);
            err::flag_proc_builtin_with_block(&mut ctx.emit, proc_src, block_src);
        }
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

    for directive in item.directives {
        if try_check_unknown_directive(ctx, directive) {
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

pub fn check_expect_config(
    ctx: &mut HirCtx,
    directives: &[ast::Directive],
    item_kinds: &'static str,
) -> ConfigState {
    let mut config = ConfigState(true);

    for directive in directives {
        if try_check_unknown_directive(ctx, directive) {
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

fn try_check_unknown_directive(ctx: &mut HirCtx, directive: &ast::Directive) -> bool {
    match directive.kind {
        DirectiveKind::Unknown(name) => {
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
        "target_ptr_width" => match config::TargetPtrWidth::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.target_ptr_width)),
            None => Err(()),
        },
        "build_kind" => match config::BuildKind::from_str(param_value) {
            Some(value) => Ok(ConfigState(value == ctx.session.config.build_kind)),
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

pub fn apply_item_flag<T: hir::ItemFlag>(
    ctx: &mut HirCtx,
    flag_set: &mut BitSet<T>,
    new_flag: T,
    item_src: SourceRange,
    directive: Option<&ast::Directive>,
    item_kinds: &'static str,
) where
    T: Copy + Clone + Into<u32> + AsStr,
{
    if flag_set.contains(new_flag) {
        if let Some(directive) = directive {
            let src = ctx.src(directive.range);
            err::directive_duplicate(&mut ctx.emit, src, directive.kind.as_str());
            return;
        } else {
            unreachable!()
        }
    }

    for old_flag in T::ALL.iter().copied() {
        if !flag_set.contains(old_flag) {
            continue;
        }
        if !new_flag.not_compatible(old_flag) {
            continue;
        }
        if let Some(directive) = directive {
            let src = ctx.src(directive.range);
            err::directive_not_compatible(
                &mut ctx.emit,
                src,
                directive.kind.as_str(),
                old_flag.as_str(),
                item_kinds,
            );
        } else {
            err::flag_not_compatible(
                &mut ctx.emit,
                item_src,
                new_flag.as_str(),
                old_flag.as_str(),
                item_kinds,
            );
        }
        return;
    }

    flag_set.set(new_flag);
}
