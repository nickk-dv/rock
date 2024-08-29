use super::hir_build::{HirData, HirEmit};
use crate::ast;
use crate::bitset::BitSet;
use crate::config;
use crate::enum_str_convert;
use crate::error::{ErrorComp, SourceRange, WarningComp};
use crate::hir;
use crate::hir::{EnumFlag, GlobalFlag, ProcFlag, StructFlag};
use crate::session::{ModuleID, Session};
use crate::text::TextRange;

enum CfgOp {
    And,
    Not,
    Or,
}

impl CfgOp {
    fn from_attr(kind: AttrKind) -> Option<CfgOp> {
        match kind {
            AttrKind::Cfg => Some(CfgOp::And),
            AttrKind::CfgNot => Some(CfgOp::Not),
            AttrKind::CfgAny => Some(CfgOp::Or),
            _ => None,
        }
    }
}

//@incomplete prototype
fn check_attr(
    hir: &HirData,
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    attr: &ast::Attr,
) {
    let module = session.module(origin_id);
    let attr_name = &module.source[attr.name.range.as_usize()];

    let kind = match AttrKind::from_str(attr_name) {
        Some(kind) => kind,
        None => {
            emit.error(ErrorComp::new(
                format!("attribute `{attr_name}` is unknown"),
                SourceRange::new(origin_id, attr.name.range),
                None,
            ));
            return;
        }
    };

    if let Some(cfg_op) = CfgOp::from_attr(kind) {
        if let Some((params, params_range)) = attr.params {
            if params.is_empty() {
                emit.error(ErrorComp::new(
                    format!("attribute `{attr_name}` requires non-empty parameter list"),
                    SourceRange::new(origin_id, params_range),
                    None,
                ));
                return;
            } else {
                for param in params {
                    let param_name = &module.source[param.name.range.as_usize()];

                    let param_kind = match CfgParamKind::from_str(param_name) {
                        Some(param_kind) => param_kind,
                        None => {
                            emit.error(ErrorComp::new(
                                format!("config parameter `{param_name}` is unknown"),
                                SourceRange::new(origin_id, param.name.range),
                                None,
                            ));
                            continue;
                        }
                    };

                    let (value, value_range) = match param.value {
                        Some((value, value_range)) => {
                            let value = hir.intern_string().get_str(value);
                            (value, value_range)
                        }
                        None => {
                            emit.error(ErrorComp::new(
                                format!("config parameter `{param_name}` requires an assigned string value"),
                                SourceRange::new(origin_id, param.name.range),
                                None,
                            ));
                            continue;
                        }
                    };

                    let cfg_state = match param_kind {
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
                        CfgParamKind::TargetPtrWidth => {
                            match config::TargetPtrWidth::from_str(value) {
                                Some(cfg_ptr_width) => {
                                    let ptr_width = hir.target().arch().ptr_width();
                                    Ok(CfgState(ptr_width == cfg_ptr_width))
                                }
                                None => Err(()),
                            }
                        }
                        CfgParamKind::BuildKind => match config::BuildKind::from_str(value) {
                            Some(cfg_build_kind) => {
                                //@current build_kind not available trough any context
                                let build_kind: config::BuildKind =
                                    todo!("build kind is not available");
                                Ok(CfgState(build_kind == cfg_build_kind))
                            }
                            None => Err(()),
                        },
                    };

                    let cfg_state = match cfg_state {
                        Ok(cfg_state) => cfg_state,
                        Err(()) => {
                            emit.error(ErrorComp::new(
                                format!("unknown `{param_name}` value `{value}`"),
                                SourceRange::new(origin_id, value_range),
                                None,
                            ));
                            continue;
                        }
                    };

                    //@correctly evaluate state according to cfg_op
                    // across multiple config parameters
                    // currently cfg_state is true on matching parameter
                }
            }
        } else {
            emit.error(ErrorComp::new(
                format!("attribute `{attr_name}` requires parameter list"),
                SourceRange::new(origin_id, attr.range),
                None,
            ));
            return;
        }
    } else {
        //@todo non cfg attributes
        // divide AttrKind into subcategories?
        return;
    }
}

pub fn process_attrs<T>(
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    item_name: ast::Name,
    target: AttrTarget,
    attrs: &[ast::Attr],
    attr_set: &mut BitSet<T>,
) -> CfgState
where
    T: Copy + Clone + Into<u32> + DataFlag<T> + 'static,
{
    let mut cfg_state = CfgState(true);
    for attr in attrs {
        let cfg = process_attr(emit, session, origin_id, item_name, target, attr, attr_set);
        cfg_state.combine(cfg);
    }
    cfg_state
}

//@set feedback like repr int_ty for enums
fn process_attr<T>(
    emit: &mut HirEmit,
    session: &Session,
    origin_id: ModuleID,
    item_name: ast::Name,
    target: AttrTarget,
    attr: &ast::Attr,
    attr_set: &mut BitSet<T>,
) -> CfgState
where
    T: Copy + Clone + Into<u32> + DataFlag<T> + 'static,
{
    let module = session.module(origin_id);
    let attr_name = &module.source[attr.name.range.as_usize()];

    let kind = match AttrKind::from_str(attr_name) {
        Some(kind) => kind,
        None => {
            emit.error(ErrorComp::new(
                format!("attribute `{attr_name}` is unknown"),
                SourceRange::new(origin_id, attr.name.range),
                None,
            ));
            return CfgState(true);
        }
    };

    if let Some(new_flag) = T::from_attr(kind) {
        check_attr_flag(
            emit,
            origin_id,
            item_name,
            target,
            Some((kind, attr.name.range)),
            attr_set,
            new_flag,
        );
    } else if let Some(cfg_op) = CfgOp::from_attr(kind) {
        //@check and evaluate #cfg attribute
    } else {
        emit.error(ErrorComp::new(
            format!(
                "attribute `{attr_name}` cannot be applied to {}",
                target.as_str(),
            ),
            SourceRange::new(origin_id, attr.range),
            None,
        ));
        return CfgState(true);
    }

    let require_params = kind.requires_params();
    if let Some((params, params_range)) = attr.params {
        if require_params && params.is_empty() {
            emit.error(ErrorComp::new(
                format!("attribute `{attr_name}` requires non-empty parameter list"),
                SourceRange::new(origin_id, params_range),
                None,
            ));
        }
        return CfgState(true);
    } else {
        if require_params {
            emit.error(ErrorComp::new(
                format!("attribute `{attr_name}` requires parameter list"),
                SourceRange::new(origin_id, attr.range),
                None,
            ));
        }
        return CfgState(true);
    }
}

#[derive(Copy, Clone)]
pub struct CfgState(bool);

impl CfgState {
    #[inline]
    pub fn disabled(self) -> bool {
        !self.0
    }
    #[inline]
    pub fn enabled(self) -> bool {
        self.0
    }
    #[inline]
    fn combine(&mut self, other: CfgState) {
        self.0 = self.0 && other.0
    }
}

#[derive(Copy, Clone)]
enum ReprKind {
    ReprC,
    ReprInt(hir::BasicInt),
}

//@variants / fields / stmts grammar 25.08.24
// cannot have attrs applied currently
enum_str_convert!(
    fn as_str,
    #[derive(Copy, Clone)]
    pub enum AttrTarget {
        Proc => "procedures",
        Enum => "enums",
        Struct => "structs",
        Const => "constants",
        Global => "globals",
        Import => "imports",
        Statement => "statements",
        EnumVariant => "enum variants",
        StructField => "struct fields",
    }
);

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone)]
    enum AttrKind {
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

impl AttrKind {
    fn requires_params(self) -> bool {
        match self {
            AttrKind::Cfg | AttrKind::CfgNot | AttrKind::CfgAny => true,
            AttrKind::Test | AttrKind::Builtin | AttrKind::Inline => false,
            AttrKind::Repr => true,
            AttrKind::ThreadLocal => false,
        }
    }
}

impl AttrTarget {
    fn can_apply(self, kind: AttrKind) -> bool {
        if matches!(kind, AttrKind::Cfg | AttrKind::CfgNot | AttrKind::CfgAny) {
            return true;
        }
        match self {
            AttrTarget::Proc => {
                matches!(kind, AttrKind::Test | AttrKind::Builtin | AttrKind::Inline)
            }
            AttrTarget::Enum => matches!(kind, AttrKind::Repr),
            AttrTarget::Struct => matches!(kind, AttrKind::Repr),
            AttrTarget::Const => false,
            AttrTarget::Global => matches!(kind, AttrKind::ThreadLocal),
            AttrTarget::Import => false,
            AttrTarget::Statement => false,
            AttrTarget::EnumVariant => false,
            AttrTarget::StructField => false,
        }
    }
}

pub fn check_attr_flag<T>(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    item_name: ast::Name,
    target: AttrTarget,
    attr: Option<(AttrKind, TextRange)>,
    attr_set: &mut BitSet<T>,
    new_flag: T,
) where
    T: Copy + Clone + Into<u32> + DataFlag<T> + 'static,
{
    if attr_set.contains(new_flag) {
        if let Some((attr, range)) = attr {
            emit.warning(WarningComp::new(
                format!("duplicate attribute `{}`", attr.as_str()),
                SourceRange::new(origin_id, range),
                None,
            ));
        } else {
            unreachable!();
        }
        return;
    }

    for flag in T::ALL_FLAGS {
        if !attr_set.contains(*flag) {
            continue;
        }
        if new_flag.compatible(*flag) {
            continue;
        }

        if let Some((_, range)) = attr {
            emit.error(ErrorComp::new(
                format!(
                    "attribute `{}` cannot be applied to `{}` {}",
                    new_flag.as_str(),
                    flag.as_str(),
                    target.as_str(),
                ),
                SourceRange::new(origin_id, range),
                None,
            ));
        } else {
            emit.error(ErrorComp::new(
                format!(
                    "`{}` {} cannot be `{}`",
                    new_flag.as_str(),
                    target.as_str(),
                    flag.as_str(),
                ),
                SourceRange::new(origin_id, item_name.range),
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
    fn from_attr(kind: AttrKind) -> Option<Self>;
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

    fn from_attr(kind: AttrKind) -> Option<ProcFlag> {
        match kind {
            AttrKind::Test => Some(ProcFlag::Test),
            AttrKind::Builtin => Some(ProcFlag::Builtin),
            AttrKind::Inline => Some(ProcFlag::Inline),
            _ => None,
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

    fn from_attr(kind: AttrKind) -> Option<EnumFlag> {
        match kind {
            AttrKind::Repr => Some(EnumFlag::HasRepr),
            _ => None,
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

    fn from_attr(kind: AttrKind) -> Option<StructFlag> {
        match kind {
            AttrKind::Repr => Some(StructFlag::ReprC),
            _ => None,
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

    fn from_attr(kind: AttrKind) -> Option<GlobalFlag> {
        match kind {
            AttrKind::ThreadLocal => Some(GlobalFlag::ThreadLocal),
            _ => None,
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
