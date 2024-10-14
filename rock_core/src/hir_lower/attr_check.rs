use super::context::HirCtx;
use crate::ast;
use crate::config;
use crate::error::{Error, ErrorSink, ErrorWarningBuffer, SourceRange, Warning, WarningSink};
use crate::errors as err;
use crate::hir::{self, EnumFlag, GlobalFlag, ProcFlag, StructFlag};
use crate::session::{Module, ModuleID};
use crate::support::{AsStr, BitSet};

pub struct AttrFeedbackProc {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<ProcFlag>,
}

pub struct AttrFeedbackEnum {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<EnumFlag>,
    pub tag_ty: Result<hir::BasicInt, ()>,
}

pub struct AttrFeedbackStruct {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<StructFlag>,
}

pub struct AttrFeedbackConst {
    pub cfg_state: CfgState,
}

pub struct AttrFeedbackGlobal {
    pub cfg_state: CfgState,
    pub attr_set: BitSet<GlobalFlag>,
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

pub fn check_attrs_proc<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    item: &ast::ProcItem,
) -> AttrFeedbackProc {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    if item.block.is_none() {
        attr_set.set(ProcFlag::External);
    }

    if item.is_variadic {
        if attr_set.contains(ProcFlag::External) {
            attr_set.set(ProcFlag::Variadic);
        } else {
            ctx.emit.error(Error::new(
                "`variadic` procedures must be `external`",
                SourceRange::new(origin_id, item.name.range),
                None,
            ));
        }
    }

    if attr_set.contains(hir::ProcFlag::Variadic) {
        if item.params.is_empty() {
            ctx.emit.error(Error::new(
                "variadic procedures must have at least one named parameter",
                SourceRange::new(origin_id, item.name.range),
                None,
            ));
        }
    }

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, origin_id, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = SourceRange::new(origin_id, attr.range);

        let flag = match resolved.data {
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            AttrResolved::Builtin => ProcFlag::Builtin,
            AttrResolved::Inline => ProcFlag::Inline,
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "procedures");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
        apply_item_flag(
            &mut ctx.emit,
            &mut attr_set,
            flag,
            attr_data,
            item_src,
            "procedures",
        );
    }

    AttrFeedbackProc {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_enum<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    item: &ast::EnumItem,
) -> AttrFeedbackEnum {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();
    let mut tag_ty = Err(());

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, origin_id, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = SourceRange::new(origin_id, attr.range);

        let flag = match resolved.data {
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            AttrResolved::Repr(repr_kind) => {
                tag_ty = match repr_kind {
                    ReprKind::ReprC => Ok(hir::BasicInt::S32),
                    ReprKind::ReprInt(int_ty) => Ok(int_ty),
                };
                hir::EnumFlag::HasRepr
            }
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "enums");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
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
        tag_ty,
    }
}

pub fn check_attrs_struct<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    item: &ast::StructItem,
) -> AttrFeedbackStruct {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, origin_id, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = SourceRange::new(origin_id, attr.range);

        let flag = match resolved.data {
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            AttrResolved::Repr(repr_kind) => match repr_kind {
                ReprKind::ReprC => hir::StructFlag::ReprC,
                ReprKind::ReprInt(int_ty) => {
                    //@add as_str for BasicInt?
                    let int_ty = int_ty.into_basic().as_str();
                    err::attr_struct_repr_int(&mut ctx.emit, attr_src, int_ty);
                    continue;
                }
            },
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "structs");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
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

pub fn check_attrs_const<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    item: &ast::ConstItem,
) -> AttrFeedbackConst {
    let cfg_state = check_attrs_expect_cfg(ctx, origin_id, item.attrs, "constants");
    AttrFeedbackConst { cfg_state }
}

pub fn check_attrs_global<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    item: &ast::GlobalItem,
) -> AttrFeedbackGlobal {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        let resolved = match resolve_attr(ctx, origin_id, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = SourceRange::new(origin_id, attr.range);

        let flag = match resolved.data {
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            AttrResolved::ThreadLocal => GlobalFlag::ThreadLocal,
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, "globals");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
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

pub fn check_attrs_import<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    item: &ast::ImportItem,
) -> AttrFeedbackImport {
    let cfg_state = check_attrs_expect_cfg(ctx, origin_id, item.attrs, "imports");
    AttrFeedbackImport { cfg_state }
}

pub fn check_attrs_stmt<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackStmt {
    let cfg_state = check_attrs_expect_cfg(ctx, origin_id, attrs, "statements");
    AttrFeedbackStmt { cfg_state }
}

pub fn check_attrs_enum_variant<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackEnumVariant {
    let cfg_state = check_attrs_expect_cfg(ctx, origin_id, attrs, "enum variants");
    AttrFeedbackEnumVariant { cfg_state }
}

pub fn check_attrs_struct_field<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackStructField {
    let cfg_state = check_attrs_expect_cfg(ctx, origin_id, attrs, "struct fields");
    AttrFeedbackStructField { cfg_state }
}

fn check_attrs_expect_cfg<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
    item_kinds: &'static str,
) -> CfgState {
    let mut cfg_state = CfgState::new_enabled();

    for attr in attrs {
        let resolved = match resolve_attr(ctx, origin_id, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = SourceRange::new(origin_id, attr.range);

        match resolved.data {
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
            }
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(&mut ctx.emit, attr_src, attr_name, item_kinds);
            }
        };
    }

    cfg_state
}

fn resolve_attr(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    attr: &ast::Attr,
) -> Result<AttrResolvedData, ()> {
    let module = ctx.session.module(origin_id);
    let file = ctx.session.vfs.file(module.file_id());
    let attr_name = &file.source[attr.name.range.as_usize()];

    let kind = match AttrKind::from_str(attr_name) {
        Some(kind) => kind,
        None => {
            let attr_src = SourceRange::new(origin_id, attr.name.range);
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
            let params = expect_multiple_params(ctx, origin_id, attr, attr_name)?;
            let state = resolve_cfg_params(ctx, origin_id, params, op)?;
            AttrResolved::Cfg(state)
        }
        AttrKind::Builtin => {
            let _ = expect_no_params(ctx, origin_id, attr, attr_name)?;
            AttrResolved::Builtin
        }
        AttrKind::Inline => {
            let _ = expect_no_params(ctx, origin_id, attr, attr_name)?;
            AttrResolved::Inline
        }
        AttrKind::Repr => {
            let param = expect_single_param(ctx, origin_id, attr, attr_name)?;
            let repr_kind = resolve_repr_param(ctx, origin_id, param)?;
            AttrResolved::Repr(repr_kind)
        }
        AttrKind::ThreadLocal => {
            let _ = expect_no_params(ctx, origin_id, attr, attr_name)?;
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
    origin_id: ModuleID,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<(), ()> {
    if let Some((_, params_range)) = attr.params {
        let params_src = SourceRange::new(origin_id, params_range);
        err::attr_param_list_unexpected(&mut ctx.emit, params_src, attr_name);
        Err(())
    } else {
        Ok(())
    }
}

fn expect_single_param<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<&'ast ast::AttrParam, ()> {
    if let Some((params, params_range)) = attr.params {
        if let Some(param) = params.get(0) {
            for param in params.iter().skip(1) {
                let param_src = SourceRange::new(origin_id, param.name.range);
                err::attr_expect_single_param(&mut ctx.emit, param_src, attr_name);
            }
            Ok(param)
        } else {
            let params_src = SourceRange::new(origin_id, params_range);
            err::attr_param_list_required(&mut ctx.emit, params_src, attr_name, true);
            Err(())
        }
    } else {
        let attr_src = SourceRange::new(origin_id, attr.name.range);
        err::attr_param_list_required(&mut ctx.emit, attr_src, attr_name, false);
        Err(())
    }
}

fn expect_multiple_params<'ast>(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<&'ast [ast::AttrParam], ()> {
    if let Some((params, params_range)) = attr.params {
        if params.is_empty() {
            let params_src = SourceRange::new(origin_id, params_range);
            err::attr_param_list_required(&mut ctx.emit, params_src, attr_name, true);
            Err(())
        } else {
            Ok(params)
        }
    } else {
        let attr_src = SourceRange::new(origin_id, attr.name.range);
        err::attr_param_list_required(&mut ctx.emit, attr_src, attr_name, false);
        Err(())
    }
}

fn resolve_cfg_params(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    params: &[ast::AttrParam],
    op: CfgOp,
) -> Result<CfgState, ()> {
    let mut cfg_state = CfgState::new_enabled();

    for param in params {
        let module = ctx.session.module(origin_id);
        let file = ctx.session.vfs.file(module.file_id());
        let param_name = &file.source[param.name.range.as_usize()];

        let param_kind = match CfgParamKind::from_str(param_name) {
            Some(param_kind) => param_kind,
            None => {
                let param_src = SourceRange::new(origin_id, param.name.range);
                err::attr_param_unknown(&mut ctx.emit, param_src, param_name);
                continue;
            }
        };

        let (value, value_range) = match param.value {
            Some((value, value_range)) => {
                //@change from using intern_lit
                let value = ctx.intern_lit().get(value);
                (value, value_range)
            }
            None => {
                let param_src = SourceRange::new(origin_id, param.name.range);
                err::attr_param_value_required(&mut ctx.emit, param_src, param_name);
                continue;
            }
        };

        let state = match param_kind {
            CfgParamKind::Target => match config::TargetTriple::from_str(value) {
                Some(cfg_triple) => {
                    let triple = ctx.target;
                    Ok(CfgState(triple == cfg_triple))
                }
                None => Err(()),
            },
            CfgParamKind::TargetArch => match config::TargetArch::from_str(value) {
                Some(cfg_arch) => {
                    let arch = ctx.target.arch();
                    Ok(CfgState(arch == cfg_arch))
                }
                None => Err(()),
            },
            CfgParamKind::TargetOS => match config::TargetOS::from_str(value) {
                Some(cfg_os) => {
                    let os = ctx.target.os();
                    Ok(CfgState(os == cfg_os))
                }
                None => Err(()),
            },
            CfgParamKind::TargetPtrWidth => match config::TargetPtrWidth::from_str(value) {
                Some(cfg_ptr_width) => {
                    let ptr_width = ctx.target.arch().ptr_width();
                    Ok(CfgState(ptr_width == cfg_ptr_width))
                }
                None => Err(()),
            },
            CfgParamKind::BuildKind => match config::BuildKind::from_str(value) {
                Some(cfg_build_kind) => {
                    //@current build_kind not available trough any context
                    let build_kind: config::BuildKind = todo!("build kind is not available");
                    Ok(CfgState(build_kind == cfg_build_kind))
                }
                None => Err(()),
            },
        };

        let state = match state {
            Ok(state) => state,
            Err(()) => {
                let value_src = SourceRange::new(origin_id, value_range);
                err::attr_param_value_unknown(&mut ctx.emit, value_src, param_name, value);
                continue;
            }
        };
        cfg_state.apply_op(state, op);
    }

    Ok(cfg_state)
}

fn resolve_repr_param(
    ctx: &mut HirCtx,
    origin_id: ModuleID,
    param: &ast::AttrParam,
) -> Result<ReprKind, ()> {
    let module = ctx.session.module(origin_id);
    let file = ctx.session.vfs.file(module.file_id());
    let param_name = &file.source[param.name.range.as_usize()];

    let mut repr_kind = if param_name == "C" {
        Ok(ReprKind::ReprC)
    } else if let Some(int_ty) = hir::BasicInt::from_str(param_name) {
        Ok(ReprKind::ReprInt(int_ty))
    } else {
        let param_src = SourceRange::new(origin_id, param.name.range);
        err::attr_param_unknown(&mut ctx.emit, param_src, param_name);
        Err(())
    };

    if let Some((_, value_range)) = param.value {
        let value_src = SourceRange::new(origin_id, value_range);
        err::attr_param_value_unexpected(&mut ctx.emit, value_src, param_name);
        repr_kind = Err(());
    }

    repr_kind
}

struct AttrResolvedData {
    kind: AttrKind,
    data: AttrResolved,
}

enum AttrResolved {
    Cfg(CfgState),
    Builtin,
    Inline,
    Repr(ReprKind),
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

#[derive(Copy, Clone)]
enum ReprKind {
    ReprC,
    ReprInt(hir::BasicInt),
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
    pub fn enabled(self) -> bool {
        self.0
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
        Repr "repr",
        ThreadLocal "thread_local",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone)]
    enum CfgParamKind {
        Target "target",
        TargetArch "target_arch",
        TargetOS "target_os",
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
            emit.warning(Warning::new(
                format!("duplicate attribute `{}`", kind.as_str()),
                attr_src,
                None,
            ));
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

        if let Some((_, attr_src)) = attr_data {
            emit.error(Error::new(
                format!(
                    "attribute `{}` cannot be applied to `{}` {item_kinds}",
                    new_flag.as_str(),
                    flag.as_str(),
                ),
                attr_src,
                None,
            ));
        } else {
            emit.error(Error::new(
                format!(
                    "`{}` {item_kinds} cannot be `{}`",
                    new_flag.as_str(),
                    flag.as_str(),
                ),
                item_src,
                None,
            ));
        }
        return;
    }

    attr_set.set(new_flag);
}
