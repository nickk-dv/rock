use super::errors as err;
use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::bitset::BitSet;
use crate::config;
use crate::enum_str_convert;
use crate::error::{ErrorComp, SourceRange, WarningComp};
use crate::hir;
use crate::hir::{EnumFlag, GlobalFlag, ProcFlag, StructFlag};
use crate::session::{ModuleID, RockModule, Session};

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
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
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
            //@use err:: for this unique case
            emit.error(ErrorComp::new(
                "`variadic` procedures must be `external`",
                SourceRange::new(origin_id, item.name.range),
                None,
            ));
        }
    }

    for attr in item.attrs {
        let resolved = match resolve_attr(hir, emit, session, origin_id, attr) {
            Ok(resolved) => resolved,
            Err(()) => continue,
        };
        let attr_src = SourceRange::new(origin_id, attr.range);

        let flag = match resolved.data {
            AttrResolved::Cfg(state) => {
                cfg_state.combine(state);
                continue;
            }
            AttrResolved::Test => ProcFlag::Test,
            AttrResolved::Builtin => ProcFlag::Builtin,
            AttrResolved::Inline => ProcFlag::Inline,
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(emit, attr_src, attr_name, "procedures");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
        check_attr_flag(emit, flag, &mut attr_set, attr_data, item_src, "procedures");
    }

    AttrFeedbackProc {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_enum<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    item: &ast::EnumItem,
) -> AttrFeedbackEnum {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();
    let mut tag_ty = Err(());

    for attr in item.attrs {
        let resolved = match resolve_attr(hir, emit, session, origin_id, attr) {
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
                err::attr_cannot_apply(emit, attr_src, attr_name, "enums");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
        check_attr_flag(emit, flag, &mut attr_set, attr_data, item_src, "enums");
    }

    AttrFeedbackEnum {
        cfg_state,
        attr_set,
        tag_ty,
    }
}

pub fn check_attrs_struct<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    item: &ast::StructItem,
) -> AttrFeedbackStruct {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        let resolved = match resolve_attr(hir, emit, session, origin_id, attr) {
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
                    err::attr_struct_repr_int(emit, attr_src, int_ty);
                    continue;
                }
            },
            _ => {
                let attr_name = resolved.kind.as_str();
                err::attr_cannot_apply(emit, attr_src, attr_name, "structs");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
        check_attr_flag(emit, flag, &mut attr_set, attr_data, item_src, "structs");
    }

    AttrFeedbackStruct {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_const<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    item: &ast::ConstItem,
) -> AttrFeedbackConst {
    let cfg_state = check_attrs_expect_cfg(hir, emit, session, origin_id, item.attrs, "constants");
    AttrFeedbackConst { cfg_state }
}

pub fn check_attrs_global<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    item: &ast::GlobalItem,
) -> AttrFeedbackGlobal {
    let mut cfg_state = CfgState::new_enabled();
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        let resolved = match resolve_attr(hir, emit, session, origin_id, attr) {
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
                err::attr_cannot_apply(emit, attr_src, attr_name, "globals");
                continue;
            }
        };

        let item_src = SourceRange::new(origin_id, item.name.range);
        let attr_data = Some((resolved.kind, attr_src));
        check_attr_flag(emit, flag, &mut attr_set, attr_data, item_src, "globals");
    }

    AttrFeedbackGlobal {
        cfg_state,
        attr_set,
    }
}

pub fn check_attrs_import<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    item: &ast::ImportItem,
) -> AttrFeedbackImport {
    let cfg_state = check_attrs_expect_cfg(hir, emit, session, origin_id, item.attrs, "imports");
    AttrFeedbackImport { cfg_state }
}

pub fn check_attrs_stmt<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackStmt {
    let cfg_state = check_attrs_expect_cfg(hir, emit, session, origin_id, attrs, "statements");
    AttrFeedbackStmt { cfg_state }
}

pub fn check_attrs_enum_variant<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackEnumVariant {
    let cfg_state = check_attrs_expect_cfg(hir, emit, session, origin_id, attrs, "enum variants");
    AttrFeedbackEnumVariant { cfg_state }
}

pub fn check_attrs_struct_field<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
) -> AttrFeedbackStructField {
    let cfg_state = check_attrs_expect_cfg(hir, emit, session, origin_id, attrs, "struct fields");
    AttrFeedbackStructField { cfg_state }
}

fn check_attrs_expect_cfg<'ast>(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    attrs: &'ast [ast::Attr<'ast>],
    item_kinds: &'static str,
) -> CfgState {
    let mut cfg_state = CfgState::new_enabled();

    for attr in attrs {
        let resolved = match resolve_attr(hir, emit, session, origin_id, attr) {
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
                err::attr_cannot_apply(emit, attr_src, attr_name, item_kinds);
            }
        };
    }

    cfg_state
}

fn resolve_attr(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    attr: &ast::Attr,
) -> Result<AttrResolvedData, ()> {
    let module = session.module(origin_id);
    let attr_name = &module.source[attr.name.range.as_usize()];

    let kind = match AttrKind::from_str(attr_name) {
        Some(kind) => kind,
        None => {
            let attr_src = SourceRange::new(origin_id, attr.name.range);
            err::attr_unknown(emit, attr_src, attr_name);
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
            let params = expect_multiple_params(emit, origin_id, attr, attr_name)?;
            let state = resolve_cfg_params(hir, emit, origin_id, module, params, op)?;
            AttrResolved::Cfg(state)
        }
        AttrKind::Test => {
            let _ = expect_no_params(emit, origin_id, attr, attr_name)?;
            AttrResolved::Test
        }
        AttrKind::Builtin => {
            let _ = expect_no_params(emit, origin_id, attr, attr_name)?;
            AttrResolved::Builtin
        }
        AttrKind::Inline => {
            let _ = expect_no_params(emit, origin_id, attr, attr_name)?;
            AttrResolved::Inline
        }
        AttrKind::Repr => {
            let param = expect_single_param(emit, origin_id, attr, attr_name)?;
            let repr_kind = resolve_repr_param(emit, origin_id, module, param)?;
            AttrResolved::Repr(repr_kind)
        }
        AttrKind::ThreadLocal => {
            let _ = expect_no_params(emit, origin_id, attr, attr_name)?;
            AttrResolved::ThreadLocal
        }
    };

    Ok(AttrResolvedData {
        kind,
        data: resolved,
    })
}

fn expect_no_params<'ast>(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<(), ()> {
    if let Some((_, params_range)) = attr.params {
        let params_src = SourceRange::new(origin_id, params_range);
        err::attr_param_list_unexpected(emit, params_src, attr_name);
        Err(())
    } else {
        Ok(())
    }
}

fn expect_single_param<'ast>(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<&'ast ast::AttrParam, ()> {
    if let Some((params, params_range)) = attr.params {
        if let Some(param) = params.get(0) {
            for param in params.iter().skip(1) {
                let param_src = SourceRange::new(origin_id, param.name.range);
                err::attr_expect_single_param(emit, param_src, attr_name);
            }
            Ok(param)
        } else {
            let params_src = SourceRange::new(origin_id, params_range);
            err::attr_param_list_required(emit, params_src, attr_name, true);
            Err(())
        }
    } else {
        let attr_src = SourceRange::new(origin_id, attr.name.range);
        err::attr_param_list_required(emit, attr_src, attr_name, false);
        Err(())
    }
}

fn expect_multiple_params<'ast>(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    attr: &ast::Attr<'ast>,
    attr_name: &str,
) -> Result<&'ast [ast::AttrParam], ()> {
    if let Some((params, params_range)) = attr.params {
        if params.is_empty() {
            let params_src = SourceRange::new(origin_id, params_range);
            err::attr_param_list_required(emit, params_src, attr_name, true);
            Err(())
        } else {
            Ok(params)
        }
    } else {
        let attr_src = SourceRange::new(origin_id, attr.name.range);
        err::attr_param_list_required(emit, attr_src, attr_name, false);
        Err(())
    }
}

fn resolve_cfg_params(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    module: &RockModule,
    params: &[ast::AttrParam],
    op: CfgOp,
) -> Result<CfgState, ()> {
    let mut cfg_state = CfgState::new_enabled();

    for param in params {
        let param_name = &module.source[param.name.range.as_usize()];

        let param_kind = match CfgParamKind::from_str(param_name) {
            Some(param_kind) => param_kind,
            None => {
                let param_src = SourceRange::new(origin_id, param.name.range);
                err::attr_param_unknown(emit, param_src, param_name);
                continue;
            }
        };

        let (value, value_range) = match param.value {
            Some((value, value_range)) => {
                //@change from using intern_lit
                let value = hir.intern_lit().get(value);
                (value, value_range)
            }
            None => {
                let param_src = SourceRange::new(origin_id, param.name.range);
                err::attr_param_value_required(emit, param_src, param_name);
                continue;
            }
        };

        let state = match param_kind {
            CfgParamKind::Target => match config::TargetTriple::from_str(value) {
                Some(cfg_triple) => {
                    let triple = hir.target();
                    Ok(CfgState(triple == cfg_triple))
                }
                None => Err(()),
            },
            CfgParamKind::TargetArch => match config::TargetArch::from_str(value) {
                Some(cfg_arch) => {
                    let arch = hir.target().arch();
                    Ok(CfgState(arch == cfg_arch))
                }
                None => Err(()),
            },
            CfgParamKind::TargetOS => match config::TargetOS::from_str(value) {
                Some(cfg_os) => {
                    let os = hir.target().os();
                    Ok(CfgState(os == cfg_os))
                }
                None => Err(()),
            },
            CfgParamKind::TargetPtrWidth => match config::TargetPtrWidth::from_str(value) {
                Some(cfg_ptr_width) => {
                    let ptr_width = hir.target().arch().ptr_width();
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
                err::attr_param_value_unknown(emit, value_src, param_name, value);
                continue;
            }
        };
        cfg_state.apply_op(state, op);
    }

    Ok(cfg_state)
}

fn resolve_repr_param(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    module: &RockModule,
    param: &ast::AttrParam,
) -> Result<ReprKind, ()> {
    let param_name = &module.source[param.name.range.as_usize()];

    let mut repr_kind = if param_name == "C" {
        Ok(ReprKind::ReprC)
    } else if let Some(int_ty) = hir::BasicInt::from_str(param_name) {
        Ok(ReprKind::ReprInt(int_ty))
    } else {
        let param_src = SourceRange::new(origin_id, param.name.range);
        err::attr_param_unknown(emit, param_src, param_name);
        Err(())
    };

    if let Some((_, value_range)) = param.value {
        let value_src = SourceRange::new(origin_id, value_range);
        err::attr_param_value_unexpected(emit, value_src, param_name);
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
    Test,
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

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone)]
    pub enum AttrKind {
        Cfg => "cfg",
        CfgNot => "cfg_not",
        CfgAny => "cfg_any",
        Test => "test",
        Builtin => "builtin",
        Inline => "inline",
        Repr => "repr",
        ThreadLocal => "thread_local",
    }
);

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone)]
    enum CfgParamKind {
        Target => "target",
        TargetArch => "target_arch",
        TargetOS => "target_os",
        TargetPtrWidth => "target_ptr_width",
        BuildKind => "build_kind",
    }
);

pub fn check_attr_flag<T>(
    emit: &mut HirEmit,
    new_flag: T,
    attr_set: &mut BitSet<T>,
    attr_data: Option<(AttrKind, SourceRange)>,
    item_src: SourceRange,
    item_kinds: &'static str,
) where
    T: Copy + Clone + Into<u32> + DataFlag<T> + 'static,
{
    if attr_set.contains(new_flag) {
        if let Some((kind, attr_src)) = attr_data {
            emit.warning(WarningComp::new(
                format!("duplicate attribute `{}`", kind.as_str()),
                attr_src,
                None,
            ));
            return;
        } else {
            unreachable!();
        }
    }

    for flag in T::ALL_FLAGS {
        if !attr_set.contains(*flag) {
            continue;
        }
        if new_flag.compatible(*flag) {
            continue;
        }

        if let Some((_, attr_src)) = attr_data {
            emit.error(ErrorComp::new(
                format!(
                    "attribute `{}` cannot be applied to `{}` {item_kinds}",
                    new_flag.as_str(),
                    flag.as_str(),
                ),
                attr_src,
                None,
            ));
        } else {
            emit.error(ErrorComp::new(
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

pub trait DataFlag<T: PartialEq + Into<u32> + 'static>
where
    Self: Sized + PartialEq,
{
    const ALL_FLAGS: &'static [T];

    fn as_str(self) -> &'static str;
    fn compatible(self, other: T) -> bool;
}

impl DataFlag<ProcFlag> for ProcFlag {
    const ALL_FLAGS: &'static [ProcFlag] = &[
        ProcFlag::External,
        ProcFlag::Variadic,
        ProcFlag::Main,
        ProcFlag::Test,
        ProcFlag::Builtin,
        ProcFlag::Inline,
    ];

    fn as_str(self) -> &'static str {
        match self {
            ProcFlag::External => "external",
            ProcFlag::Variadic => "variadic",
            ProcFlag::Main => "main",
            ProcFlag::Test => "test",
            ProcFlag::Builtin => "builtin",
            ProcFlag::Inline => "inline",
        }
    }

    fn compatible(self, other: ProcFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            ProcFlag::External => matches!(other, ProcFlag::Variadic | ProcFlag::Inline),
            ProcFlag::Variadic => matches!(other, ProcFlag::External | ProcFlag::Inline),
            ProcFlag::Main => false,
            ProcFlag::Test => matches!(other, ProcFlag::Inline),
            ProcFlag::Builtin => matches!(other, ProcFlag::Inline),
            ProcFlag::Inline => !matches!(other, ProcFlag::Main),
        }
    }
}

impl DataFlag<EnumFlag> for EnumFlag {
    const ALL_FLAGS: &'static [EnumFlag] = &[EnumFlag::HasRepr];

    fn as_str(self) -> &'static str {
        match self {
            EnumFlag::HasRepr => "repr",
        }
    }

    fn compatible(self, other: EnumFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            EnumFlag::HasRepr => false,
        }
    }
}

impl DataFlag<StructFlag> for StructFlag {
    const ALL_FLAGS: &'static [StructFlag] = &[StructFlag::ReprC];

    fn as_str(self) -> &'static str {
        match self {
            StructFlag::ReprC => "repr(C)",
        }
    }

    fn compatible(self, other: StructFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            StructFlag::ReprC => false,
        }
    }
}

impl DataFlag<GlobalFlag> for GlobalFlag {
    const ALL_FLAGS: &'static [GlobalFlag] = &[GlobalFlag::ThreadLocal];

    fn as_str(self) -> &'static str {
        match self {
            GlobalFlag::ThreadLocal => "thread_local",
        }
    }

    fn compatible(self, other: GlobalFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            GlobalFlag::ThreadLocal => false,
        }
    }
}

impl Into<u32> for ProcFlag {
    fn into(self) -> u32 {
        self as u32
    }
}
impl Into<u32> for EnumFlag {
    fn into(self) -> u32 {
        self as u32
    }
}
impl Into<u32> for StructFlag {
    fn into(self) -> u32 {
        self as u32
    }
}
impl Into<u32> for GlobalFlag {
    fn into(self) -> u32 {
        self as u32
    }
}