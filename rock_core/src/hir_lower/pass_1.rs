use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::bitset::BitSet;
use crate::error::{ErrorComp, Info, SourceRange, WarningComp};
use crate::hir;
use crate::hir::{EnumFlag, GlobalFlag, ProcFlag, StructFlag};
use crate::session::{ModuleID, Session};

pub fn populate_scopes<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    session: &Session,
) {
    for origin_id in session.module_ids() {
        add_module_items(hir, emit, origin_id);
    }
}

fn add_module_items<'hir>(
    hir: &mut HirData<'hir, '_, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
) {
    let module_ast = hir.ast_module(origin_id);
    for item in module_ast.items.iter().copied() {
        match item {
            ast::Item::Proc(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_proc_item(hir, emit, origin_id, item),
            },
            ast::Item::Enum(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_enum_item(hir, emit, origin_id, item),
            },
            ast::Item::Struct(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_struct_item(hir, emit, origin_id, item),
            },
            ast::Item::Const(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_const_item(hir, emit, origin_id, item),
            },
            ast::Item::Global(item) => match hir.symbol_in_scope_source(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_global_item(hir, emit, origin_id, item),
            },
            ast::Item::Import(item) => check_import_item(emit, origin_id, item),
        }
    }
}

enum CfgKind {
    Target,
    TargetArch,
    TargetOs,
    TargetPtrWidth,
    BuildKind,
}

fn add_proc_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    item: &'ast ast::ProcItem<'ast>,
) {
    let mut attr_set = BitSet::empty();

    if item.block.is_none() {
        attr_set.set(ProcFlag::External);
    }

    if item.is_variadic {
        if attr_set.contains(ProcFlag::External) {
            attr_set.set(ProcFlag::Variadic);
        } else {
            emit.error(ErrorComp::new(
                "`variadic` procedures must be `external`",
                SourceRange::new(origin_id, item.name.range),
                None,
            ));
        }
    }

    for attr in item.attrs {
        let flag = match attr.kind {
            ast::AttributeKind::Cfg | ast::AttributeKind::Cfg_Not | ast::AttributeKind::Cfg_Any => {
                None
            }
            ast::AttributeKind::Test => Some(ProcFlag::Test),
            ast::AttributeKind::Builtin => Some(ProcFlag::Builtin),
            ast::AttributeKind::Inline => Some(ProcFlag::Inline),
            ast::AttributeKind::ReprC | ast::AttributeKind::Thread_Local => {
                error_attribute_cannot_apply(emit, origin_id, attr, "procedures");
                None
            }
            ast::AttributeKind::Unknown => {
                error_attribute_unknown(emit, origin_id, attr);
                None
            }
        };

        if let Some(new_flag) = flag {
            check_attribute_flag(
                emit,
                origin_id,
                item.name,
                "procedures",
                Some(attr),
                &mut attr_set,
                new_flag,
            );
        }
    }

    let data = hir::ProcData {
        origin_id,
        attr_set,
        vis: item.vis,
        name: item.name,
        params: &[],
        return_ty: hir::Type::Error,
        block: None,
        locals: &[],
    };

    let id = hir.registry_mut().add_proc(item, data);

    hir.add_symbol(
        origin_id,
        item.name.id,
        Symbol::Defined {
            kind: SymbolKind::Proc(id),
        },
    );
}

fn add_enum_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    item: &'ast ast::EnumItem<'ast>,
) {
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        match attr.kind {
            ast::AttributeKind::ReprC => attr_set.set(EnumFlag::ReprC),
            ast::AttributeKind::Unknown => error_attribute_unknown(emit, origin_id, attr),
            _ => error_attribute_cannot_apply(emit, origin_id, attr, "enums"),
        }
    }

    //@set resolved based on ast basic
    //@conflict with repr_c, instead use repr(`param`) instead?
    //@currently manual tag_ty cannot be set
    // repr int_ty is probably required else correct constant expr checking
    // cannot be done and it will default to i32 which isnt always correct
    let data = hir::EnumData {
        origin_id,
        attr_set,
        vis: item.vis,
        name: item.name,
        variants: &[],
        tag_ty: hir::TagTypeEval::Unresolved,
        layout: hir::LayoutEval::Unresolved,
    };

    let id = hir.registry_mut().add_enum(item, data);

    hir.add_symbol(
        origin_id,
        item.name.id,
        Symbol::Defined {
            kind: SymbolKind::Enum(id),
        },
    );
}

fn add_struct_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    item: &'ast ast::StructItem<'ast>,
) {
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        match attr.kind {
            ast::AttributeKind::ReprC => attr_set.set(StructFlag::ReprC),
            ast::AttributeKind::Unknown => error_attribute_unknown(emit, origin_id, attr),
            _ => error_attribute_cannot_apply(emit, origin_id, attr, "structs"),
        }
    }

    let data = hir::StructData {
        origin_id,
        attr_set,
        vis: item.vis,
        name: item.name,
        fields: &[],
        layout: hir::LayoutEval::Unresolved,
    };

    let id = hir.registry_mut().add_struct(item, data);

    hir.add_symbol(
        origin_id,
        item.name.id,
        Symbol::Defined {
            kind: SymbolKind::Struct(id),
        },
    );
}

fn add_const_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    item: &'ast ast::ConstItem<'ast>,
) {
    for attr in item.attrs {
        match attr.kind {
            ast::AttributeKind::Unknown => error_attribute_unknown(emit, origin_id, attr),
            _ => error_attribute_cannot_apply(emit, origin_id, attr, "constants"),
        }
    }

    let data = hir::ConstData {
        origin_id,
        vis: item.vis,
        name: item.name,
        ty: hir::Type::Error,
        value: hir.registry_mut().add_const_eval(item.value, origin_id),
    };

    let id = hir.registry_mut().add_const(item, data);

    hir.add_symbol(
        origin_id,
        item.name.id,
        Symbol::Defined {
            kind: SymbolKind::Const(id),
        },
    );
}

fn add_global_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    item: &'ast ast::GlobalItem<'ast>,
) {
    let mut attr_set = BitSet::empty();

    for attr in item.attrs {
        let flag = match attr.kind {
            ast::AttributeKind::Cfg | ast::AttributeKind::Cfg_Not | ast::AttributeKind::Cfg_Any => {
                None
            }
            ast::AttributeKind::Test
            | ast::AttributeKind::Builtin
            | ast::AttributeKind::Inline
            | ast::AttributeKind::ReprC => {
                error_attribute_cannot_apply(emit, origin_id, attr, "globals");
                None
            }
            ast::AttributeKind::Thread_Local => Some(GlobalFlag::ThreadLocal),
            ast::AttributeKind::Unknown => {
                error_attribute_unknown(emit, origin_id, attr);
                None
            }
        };

        if let Some(new_flag) = flag {
            check_attribute_flag(
                emit,
                origin_id,
                item.name,
                "globals",
                Some(attr),
                &mut attr_set,
                new_flag,
            );
        }
    }

    let data = hir::GlobalData {
        origin_id,
        attr_set,
        vis: item.vis,
        mutt: item.mutt,
        name: item.name,
        ty: hir::Type::Error,
        value: hir.registry_mut().add_const_eval(item.value, origin_id),
    };

    let id = hir.registry_mut().add_global(item, data);

    hir.add_symbol(
        origin_id,
        item.name.id,
        Symbol::Defined {
            kind: SymbolKind::Global(id),
        },
    );
}

fn check_import_item<'hir, 'ast>(
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    item: &'ast ast::ImportItem<'ast>,
) {
    for attr in item.attrs {
        match attr.kind {
            ast::AttributeKind::Unknown => error_attribute_unknown(emit, origin_id, attr),
            _ => error_attribute_cannot_apply(emit, origin_id, attr, "constants"),
        }
    }
}

pub fn error_name_already_defined(
    hir: &HirData,
    emit: &mut HirEmit,
    origin_id: ModuleID,
    name: ast::Name,
    existing: SourceRange,
) {
    emit.error(ErrorComp::new(
        format!("name `{}` is defined multiple times", hir.name_str(name.id)),
        SourceRange::new(origin_id, name.range),
        Info::new("existing definition", existing),
    ));
}

fn error_attribute_unknown(emit: &mut HirEmit, origin_id: ModuleID, attr: &ast::Attribute) {
    emit.error(ErrorComp::new(
        "attribute is unknown",
        SourceRange::new(origin_id, attr.range),
        None,
    ));
}

fn error_attribute_cannot_apply(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    attr: &ast::Attribute,
    item_kind: &'static str,
) {
    emit.error(ErrorComp::new(
        format!(
            "attribute #[{}] cannot be applied to {item_kind}",
            attr.kind.as_str()
        ),
        SourceRange::new(origin_id, attr.range),
        None,
    ));
}

pub fn check_attribute_flag<T>(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    item_name: ast::Name,
    item_kind: &'static str,
    attr: Option<&ast::Attribute>,
    attr_set: &mut BitSet<T>,
    new_flag: T,
) where
    T: Copy + Clone + Into<u32> + DataFlag<T> + 'static,
{
    if attr_set.contains(new_flag) {
        if let Some(attr) = attr {
            emit.warning(WarningComp::new(
                format!("duplicate attribute #[`{}`]", attr.kind.as_str()),
                SourceRange::new(origin_id, attr.range),
                None,
            ));
        } else {
            // properties cannot be set multiple times
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

        if let Some(attr) = attr {
            emit.error(ErrorComp::new(
                format!(
                    "attribute #[{}] cannot be applied to `{}` {item_kind}",
                    new_flag.as_str(),
                    flag.as_str(),
                ),
                SourceRange::new(origin_id, attr.range),
                None,
            ));
        } else {
            emit.error(ErrorComp::new(
                format!(
                    "`{}` {item_kind} cannot be `{}`",
                    new_flag.as_str(),
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
    const ALL_FLAGS: &'static [EnumFlag] = &[EnumFlag::ReprC];

    fn as_str(self) -> &'static str {
        match self {
            EnumFlag::ReprC => "repr_c",
        }
    }

    fn compatible(self, other: EnumFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            EnumFlag::ReprC => false,
        }
    }
}

impl DataFlag<StructFlag> for StructFlag {
    const ALL_FLAGS: &'static [StructFlag] = &[StructFlag::ReprC];

    fn as_str(self) -> &'static str {
        match self {
            StructFlag::ReprC => "repr_c",
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
