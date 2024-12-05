use super::context::HirCtx;
use crate::ast;
use crate::config;
use crate::error::{ErrorWarningBuffer, SourceRange};
use crate::errors as err;
use crate::hir;
use crate::support::{AsStr, BitSet};

pub struct AttrFeedbackProc {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<hir::ProcFlag>,
}

pub struct AttrFeedbackEnum {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<hir::EnumFlag>,
}

pub struct AttrFeedbackStruct {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<hir::StructFlag>,
}

pub struct AttrFeedbackConst {
    pub cfg_state: CfgState,
}

pub struct AttrFeedbackGlobal {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<hir::GlobalFlag>,
}

pub struct AttrFeedbackImport {
    pub cfg_state: CfgState,
}

pub struct AttrFeedbackStmt {
    pub cfg_state: CfgState,
}

pub struct AttrFeedbackEnumVariant {
    pub cfg_state: CfgState,
}

pub struct AttrFeedbackStructField {
    pub cfg_state: CfgState,
}

pub fn check_attrs_proc<'ast>(ctx: &mut HirCtx, item: &ast::ProcItem) -> AttrFeedbackProc {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    if item.block.is_none() {
        attr_set.set(hir::ProcFlag::External);
    }

    if item.is_variadic {
        if attr_set.contains(hir::ProcFlag::External) {
            attr_set.set(hir::ProcFlag::Variadic);
        } else {
            let proc_src = ctx.src(item.name.range);
            err::attr_proc_variadic_not_external(&mut ctx.emit, proc_src);
        }
    }

    if attr_set.contains(hir::ProcFlag::Variadic) && item.params.is_empty() {
        let proc_src = ctx.src(item.name.range);
        err::attr_proc_variadic_zero_params(&mut ctx.emit, proc_src);
    }

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = ctx.src(attr.range);

        let flag = match resolved.data {
            AttrResolved::Builtin => hir::ProcFlag::Builtin,
            AttrResolved::Inline => hir::ProcFlag::Inline,
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "procedures");
                continue;
            }
        };

        let attr_data = Some((resolved.kind, attr_src));
        let item_src = ctx.src(item.name.range);
        apply_item_flag(
            &mut ctx.emit,
            &mut attr_set,
            flag,
            attr_data,
            item_src,
            "procedures",
        );
    }

    if attr_set.contains(hir::ProcFlag::Builtin) {
        if let Some(block) = item.block {
            let proc_src = ctx.src(item.name.range);
            let block_src = ctx.src(block.range);
            err::attr_proc_builtin_with_block(&mut ctx.emit, proc_src, block_src);
        }
    }

    AttrFeedbackProc {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_enum<'ast>(ctx: &mut HirCtx, item: &ast::EnumItem) -> AttrFeedbackEnum {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    if item.tag_ty.is_some() {
        attr_set.set(hir::EnumFlag::WithTagType);
    }

    for variant in item.variants {
        match variant.kind {
            ast::VariantKind::Default => {}
            ast::VariantKind::Constant(_) => {}
            ast::VariantKind::HasFields(_) => {
                attr_set.set(hir::EnumFlag::WithFields);
                break;
            }
        }
    }

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = ctx.src(attr.range);

        let flag = match resolved.data {
            AttrResolved::ReprC => hir::EnumFlag::ReprC,
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "enums");
                continue;
            }
        };

        let attr_data = Some((resolved.kind, attr_src));
        let item_src = ctx.src(item.name.range);
        apply_item_flag(
            &mut ctx.emit,
            &mut attr_set,
            flag,
            attr_data,
            item_src,
            "enums",
        );
    }

    AttrFeedbackEnum {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_struct<'ast>(ctx: &mut HirCtx, item: &ast::StructItem) -> AttrFeedbackStruct {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = ctx.src(attr.range);

        let flag = match resolved.data {
            AttrResolved::ReprC => hir::StructFlag::ReprC,
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "structs");
                continue;
            }
        };

        let attr_data = Some((resolved.kind, attr_src));
        let item_src = ctx.src(item.name.range);
        apply_item_flag(
            &mut ctx.emit,
            &mut attr_set,
            flag,
            attr_data,
            item_src,
            "structs",
        );
    }

    AttrFeedbackStruct {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_const<'ast>(ctx: &mut HirCtx, item: &ast::ConstItem) -> AttrFeedbackConst {
    let cfg_state = check_attrs_expect_cfg(ctx, item.attrs, "constants");
    AttrFeedbackConst { cfg_state }
}

pub fn check_attrs_global<'ast>(ctx: &mut HirCtx, item: &ast::GlobalItem) -> AttrFeedbackGlobal {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = ctx.src(attr.range);

        let flag = match resolved.data {
            AttrResolved::ThreadLocal => hir::GlobalFlag::ThreadLocal,
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "globals");
                continue;
            }
        };

        let attr_data = Some((resolved.kind, attr_src));
        let item_src = ctx.src(item.name.range);
        apply_item_flag(
            &mut ctx.emit,
            &mut attr_set,
            flag,
            attr_data,
            item_src,
            "globals",
        );
    }

    AttrFeedbackGlobal {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_import<'ast>(ctx: &mut HirCtx, item: &ast::ImportItem) -> AttrFeedbackImport {
    let cfg_state = check_attrs_expect_cfg(ctx, item.attrs, "imports");
    AttrFeedbackImport { cfg_state }
}

pub fn check_attrs_stmt<'ast>(
    ctx: &mut HirCtx,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackStmt {
    let cfg_state = check_attrs_expect_cfg(ctx, attrs, "statements");
    AttrFeedbackStmt { cfg_state }
}

pub fn check_attrs_enum_variant<'ast>(
    ctx: &mut HirCtx,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackEnumVariant {
    let cfg_state = check_attrs_expect_cfg(ctx, attrs, "enum variants");
    AttrFeedbackEnumVariant { cfg_state }
}

pub fn check_attrs_struct_field<'ast>(
    ctx: &mut HirCtx,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackStructField {
    let cfg_state = check_attrs_expect_cfg(ctx, attrs, "struct fields");
    AttrFeedbackStructField { cfg_state }
}

fn check_attrs_expect_cfg<'ast>(
    ctx: &mut HirCtx,
    attrs: &'ast [ast::Attr<'ast>],
    item_kinds: &'static str,
) -> CfgState {
    let mut cfg_state = CfgState::new_enabled();

    for attr in attrs {
        let resolved = match resolve_attr(ctx, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };

        match resolved.data {
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
            }
            _ => {
                let attr_src = ctx.src(attr.name.range);
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, item_kinds);
            }
        };
    }

    cfg_state
}

fn resolve_attr(ctx: &mut HirCtx, attr: &ast::Attr) -> Result<AttrResolvedData, ()> {
    let attr_name = ctx.name(attr.name.id);

    let kind = match AttrKind::from_str(attr_name) {
        Some(kind) => kind,
        None => {
            let attr_src = ctx.src(attr.name.range);
            err::attr_unknown(&mut ctx.emit, attr_src, attr_name);
            return Err(());
        }
    };

    let resolved = match kind {
        AttrKind::Cfg | AttrKind::CfgNot | AttrKind::CfgAny => {
            let op = match kind {
                AttrKind::Cfg => CfgOp::And,
                AttrKind::CfgNot => CfgOp::Not,
                AttrKind::CfgAny => CfgOp::Or,
                _ => unreachable!(),
            };
            let params = expect_multiple_params(ctx, attr, attr_name)?;
            let state = resolve_cfg_params(ctx, params, op)?;
            AttrResolved::Cfg(state)
        }
        AttrKind::Builtin => {
            let _ = expect_no_params(ctx, attr, attr_name)?;
            AttrResolved::Builtin
        }
        AttrKind::Inline => {
            let _ = expect_no_params(ctx, attr, attr_name)?;
            AttrResolved::Inline
        }
        AttrKind::ReprC => {
            let _ = expect_no_params(ctx, attr, attr_name)?;
            AttrResolved::ReprC
        }
        AttrKind::ThreadLocal => {
            let _ = expect_no_params(ctx, attr, attr_name)?;
            AttrResolved::ThreadLocal
        }
    };

    Ok(AttrResolvedData {
        kind,
        data: resolved,
    })
}

fn expect_no_params<'ast>(
    ctx: &mut HirCtx,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<(), ()> {
    if let Some((_, params_range)) = attr.params {
        let params_src = ctx.src(params_range);
        err::attr_param_list_unexpected(&mut ctx.emit, params_src, attr_name);
        Err(())
    } else {
        Ok(())
    }
}

fn expect_multiple_params<'ast>(
    ctx: &mut HirCtx,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<&'ast [ast::AttrParam], ()> {
    if let Some((params, params_range)) = attr.params {
        if params.is_empty() {
            let list_src = ctx.src(params_range);
            err::attr_param_list_required(&mut ctx.emit, list_src, attr_name, true);
            Err(())
        } else {
            Ok(params)
        }
    } else {
        let attr_src = ctx.src(attr.name.range);
        err::attr_param_list_required(&mut ctx.emit, attr_src, attr_name, false);
        Err(())
    }
}

fn resolve_cfg_params(
    ctx: &mut HirCtx,
    params: &[ast::AttrParam],
    op: CfgOp,
) -> Result<CfgState, ()> {
    let mut cfg_state = CfgState::new_enabled();

    for param in params {
        let param_name = ctx.name(param.name.id);

        let param_kind = match CfgParamKind::from_str(param_name) {
            Some(param_kind) => param_kind,
            None => {
                let param_src = ctx.src(param.name.range);
                err::attr_param_unknown(&mut ctx.emit, param_src, param_name);
                continue;
            }
        };

        let (value, value_range) = match param.value {
            Some((value, value_range)) => {
                let value = ctx.session.intern_lit.get(value);
                (value, value_range)
            }
            None => {
                let param_src = ctx.src(param.name.range);
                err::attr_param_value_required(&mut ctx.emit, param_src, param_name);
                continue;
            }
        };

        let state = match param_kind {
            CfgParamKind::Target => match config::TargetTriple::from_str(value) {
                Some(cfg_triple) => {
                    let triple = ctx.session.config.target;
                    Ok(CfgState(triple == cfg_triple))
                }
                None => Err(()),
            },
            CfgParamKind::TargetOS => match config::TargetOS::from_str(value) {
                Some(cfg_os) => {
                    let os = ctx.session.config.target_os;
                    Ok(CfgState(os == cfg_os))
                }
                None => Err(()),
            },
            CfgParamKind::TargetArch => match config::TargetArch::from_str(value) {
                Some(cfg_arch) => {
                    let arch = ctx.session.config.target_arch;
                    Ok(CfgState(arch == cfg_arch))
                }
                None => Err(()),
            },
            CfgParamKind::TargetPtrWidth => match config::TargetPtrWidth::from_str(value) {
                Some(cfg_ptr_width) => {
                    let ptr_width = ctx.session.config.target_ptr_width;
                    Ok(CfgState(ptr_width == cfg_ptr_width))
                }
                None => Err(()),
            },
            CfgParamKind::BuildKind => match config::BuildKind::from_str(value) {
                Some(cfg_build_kind) => {
                    let build_kind = ctx.session.config.build_kind;
                    Ok(CfgState(build_kind == cfg_build_kind))
                }
                None => Err(()),
            },
        };

        let state = match state {
            Ok(state) => state,
            Err(()) => {
                let value_src = ctx.src(value_range);
                err::attr_param_value_unknown(&mut ctx.emit, value_src, param_name, value);
                continue;
            }
        };
        cfg_state.apply_op(state, op);
    }

    Ok(cfg_state)
}

struct AttrResolvedData {
    kind: AttrKind,
    data: AttrResolved,
}

enum AttrResolved {
    Cfg(CfgState),
    Builtin,
    Inline,
    ReprC,
    ThreadLocal,
}

#[derive(Copy, Clone)]
pub struct CfgState(bool);

#[derive(Copy, Clone)]
enum CfgOp {
    And,
    Not,
    Or,
}

impl CfgState {
    #[inline]
    fn new_enabled() -> CfgState {
        CfgState(true)
    }
    #[inline]
    pub fn disabled(self) -> bool {
        !self.0
    }
    #[inline]
    fn combine(&mut self, state: CfgState) {
        self.0 = self.0 && state.0;
    }
    #[inline]
    fn apply_op(&mut self, state: CfgState, op: CfgOp) {
        match op {
            CfgOp::And => self.0 = self.0 && state.0,
            CfgOp::Not => self.0 = self.0 && !state.0,
            CfgOp::Or => self.0 = self.0 || state.0,
        }
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone)]
    pub enum AttrKind {
        Cfg "cfg",
        CfgNot "cfg_not",
        CfgAny "cfg_any",
        Builtin "builtin",
        Inline "inline",
        ReprC "repr_c",
        ThreadLocal "thread_local",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone)]
    enum CfgParamKind {
        Target "target",
        TargetOS "target_os",
        TargetArch "target_arch",
        TargetPtrWidth "target_ptr_width",
        BuildKind  "build_kind",
    }
}

pub fn apply_item_flag<T: hir::ItemFlag>(
    emit: &mut ErrorWarningBuffer,
    attr_set: &mut BitSet<T>,
    new_flag: T,
    attr_data: Option<(AttrKind, SourceRange)>,
    item_src: SourceRange,
    item_kinds: &'static str,
) where
    T: Copy + Clone + Into<u32> + AsStr,
{
    if attr_set.contains(new_flag) {
        if let Some((kind, attr_src)) = attr_data {
            err::attr_duplicate(emit, attr_src, kind.as_str());
            return;
        } else {
            unreachable!();
        }
    }

    for flag in T::ALL.iter().copied() {
        if !attr_set.contains(flag) {
            continue;
        }
        if new_flag.compatible(flag) {
            continue;
        }

        if let Some((kind, attr_src)) = attr_data {
            err::attr_not_compatible(emit, attr_src, kind.as_str(), flag.as_str(), item_kinds);
        } else {
            err::attr_flag_not_compatible(
                emit,
                item_src,
                new_flag.as_str(),
                flag.as_str(),
                item_kinds,
            );
        }
        return;
    }

    attr_set.set(new_flag);
}
