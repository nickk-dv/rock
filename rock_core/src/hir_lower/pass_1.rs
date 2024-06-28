use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::bitset::BitSet;
use crate::error::{ErrorComp, Info, SourceRange, WarningComp};
use crate::hir;
use crate::hir::{GlobalFlag, ProcFlag};
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
            ast::Item::Proc(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_proc_item(hir, emit, origin_id, item),
            },
            ast::Item::Enum(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_enum_item(hir, emit, origin_id, item),
            },
            ast::Item::Struct(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_struct_item(hir, emit, origin_id, item),
            },
            ast::Item::Const(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_const_item(hir, emit, origin_id, item),
            },
            ast::Item::Global(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(src) => error_name_already_defined(hir, emit, origin_id, item.name, src),
                None => add_global_item(hir, emit, origin_id, item),
            },
            ast::Item::Import(item) => check_import_item(emit, origin_id, item),
        }
    }
}

fn add_proc_item<'hir, 'ast>(
    hir: &mut HirData<'hir, 'ast, '_>,
    emit: &mut HirEmit<'hir>,
    origin_id: ModuleID,
    item: &'ast ast::ProcItem<'ast>,
) {
    let mut attr_set = BitSet::EMPTY;

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
            ast::AttributeKind::Test => Some(ProcFlag::Test),
            ast::AttributeKind::Builtin => Some(ProcFlag::Builtin),
            ast::AttributeKind::Inline => Some(ProcFlag::Inline),
            ast::AttributeKind::Thread_Local => {
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
                &PROC_FLAG_ALL,
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
    for attr in item.attrs {
        match attr.kind {
            ast::AttributeKind::Unknown => error_attribute_unknown(emit, origin_id, attr),
            _ => error_attribute_cannot_apply(emit, origin_id, attr, "enums"),
        }
    }

    let data = hir::EnumData {
        origin_id,
        vis: item.vis,
        name: item.name,
        basic: item.basic.unwrap_or(ast::BasicType::S32),
        variants: &[],
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
    for attr in item.attrs {
        match attr.kind {
            ast::AttributeKind::Unknown => error_attribute_unknown(emit, origin_id, attr),
            _ => error_attribute_cannot_apply(emit, origin_id, attr, "structs"),
        }
    }

    let data = hir::StructData {
        origin_id,
        vis: item.vis,
        name: item.name,
        fields: &[],
        size_eval: hir::SizeEval::Unresolved,
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
    let mut attr_set = BitSet::EMPTY;

    for attr in item.attrs {
        let flag = match attr.kind {
            ast::AttributeKind::Test | ast::AttributeKind::Builtin | ast::AttributeKind::Inline => {
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
                &GLOBAL_FLAG_ALL,
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
        format!("attribute is unknown"),
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

pub fn check_attribute_flag<FlagT: AttributeFlag + Copy + Clone>(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    item_name: ast::Name,
    item_kind: &'static str,
    attr: Option<&ast::Attribute>,
    attr_set: &mut BitSet,
    new_flag: FlagT,
    all_flags: &[FlagT],
) {
    if attr_set.contains(new_flag.into_u32()) {
        if let Some(attr) = attr {
            emit.warning(WarningComp::new(
                format!("duplicate attribute #[`{}`]", attr.kind.as_str()),
                SourceRange::new(origin_id, attr.range),
                None,
            ));
        } else {
            // properties like `external`, `variadic`
            // cannot be set multiple times, unlike attributes
            unreachable!();
        }
        return;
    }

    let compat_set = new_flag.compatibility_set();

    for flag in all_flags {
        if attr_set.contains(flag.into_u32()) {
            if !compat_set.contains(flag.into_u32()) {
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
        }
    }

    attr_set.set(new_flag.into_u32());
}

pub trait AttributeFlag
where
    Self: Sized,
{
    fn into_u32(self) -> u32;
    fn as_str(self) -> &'static str;
    fn compatibility_set(self) -> BitSet;
}

impl AttributeFlag for ProcFlag {
    fn into_u32(self) -> u32 {
        self as u32
    }

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

    fn compatibility_set(self) -> BitSet {
        match self {
            ProcFlag::External => PROC_FLAG_COMPAT_EXTERNAL,
            ProcFlag::Variadic => PROC_FLAG_COMPAT_VARIADIC,
            ProcFlag::Main => PROC_FLAG_COMPAT_MAIN,
            ProcFlag::Test => PROC_FLAG_COMPAT_TEST,
            ProcFlag::Builtin => PROC_FLAG_COMPAT_BUILTIN,
            ProcFlag::Inline => PROC_FLAG_COMPAT_INLINE,
        }
    }
}

impl AttributeFlag for GlobalFlag {
    fn into_u32(self) -> u32 {
        self as u32
    }

    fn as_str(self) -> &'static str {
        match self {
            GlobalFlag::ThreadLocal => "thread_local",
        }
    }

    fn compatibility_set(self) -> BitSet {
        match self {
            GlobalFlag::ThreadLocal => GLOBAL_FLAG_COMPAT_THREAD_LOCAL,
        }
    }
}

impl Into<u32> for ProcFlag {
    fn into(self) -> u32 {
        self as u32
    }
}

impl Into<u32> for GlobalFlag {
    fn into(self) -> u32 {
        self as u32
    }
}

pub const PROC_FLAG_ALL: [ProcFlag; 6] = [
    ProcFlag::External,
    ProcFlag::Variadic,
    ProcFlag::Main,
    ProcFlag::Test,
    ProcFlag::Builtin,
    ProcFlag::Inline,
];

const PROC_FLAG_COMPAT_EXTERNAL: BitSet =
    BitSet::new(&[ProcFlag::Variadic as u32, ProcFlag::Inline as u32]);
const PROC_FLAG_COMPAT_VARIADIC: BitSet =
    BitSet::new(&[ProcFlag::External as u32, ProcFlag::Inline as u32]);
const PROC_FLAG_COMPAT_MAIN: BitSet = BitSet::new(&[]);
const PROC_FLAG_COMPAT_TEST: BitSet = BitSet::new(&[ProcFlag::Inline as u32]);
const PROC_FLAG_COMPAT_BUILTIN: BitSet = BitSet::new(&[ProcFlag::Inline as u32]);
const PROC_FLAG_COMPAT_INLINE: BitSet = BitSet::new(&[
    ProcFlag::External as u32,
    ProcFlag::Variadic as u32,
    ProcFlag::Test as u32,
    ProcFlag::Builtin as u32,
]);

const GLOBAL_FLAG_ALL: [GlobalFlag; 1] = [GlobalFlag::ThreadLocal];

const GLOBAL_FLAG_COMPAT_THREAD_LOCAL: BitSet = BitSet::new(&[]);
