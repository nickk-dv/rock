use super::hir_build::{HirData, HirEmit, Symbol, SymbolKind};
use crate::ast;
use crate::bitset::BitSet;
use crate::error::{ErrorComp, Info, SourceRange, WarningComp};
use crate::hir::{GlobalFlag, ProcFlag};
use crate::session::{ModuleID, Session};
use crate::text::TextRange;

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
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    if let Some(attr) = item.attr {
                        let flag = match attr.kind {
                            ast::AttributeKind::Test => Some(ProcFlag::Test),
                            ast::AttributeKind::Builtin => Some(ProcFlag::Builtin),
                            ast::AttributeKind::Inline => Some(ProcFlag::Inline),
                            ast::AttributeKind::Thread_Local => {
                                error_attribute_cannot_apply(emit, origin_id, attr, "procedure");
                                None
                            }
                            ast::AttributeKind::Unknown => {
                                error_attribute_unknown(emit, origin_id, attr);
                                None
                            }
                        };

                        //@check the flag
                    }

                    let id = hir.registry_mut().add_proc(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Proc(id),
                        },
                    );
                }
            },
            ast::Item::Enum(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    let id = hir.registry_mut().add_enum(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Enum(id),
                        },
                    );
                }
            },
            ast::Item::Struct(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing)
                }
                None => {
                    let id = hir.registry_mut().add_struct(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Struct(id),
                        },
                    );
                }
            },
            ast::Item::Const(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    let id = hir.registry_mut().add_const(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Const(id),
                        },
                    );
                }
            },
            ast::Item::Global(item) => match hir.scope_name_defined(origin_id, item.name.id) {
                Some(existing) => {
                    name_already_defined_error(hir, emit, origin_id, item.name, existing);
                }
                None => {
                    if let Some(attr) = item.attr {
                        let flag = match attr.kind {
                            ast::AttributeKind::Test
                            | ast::AttributeKind::Builtin
                            | ast::AttributeKind::Inline => {
                                error_attribute_cannot_apply(emit, origin_id, attr, "global");
                                None
                            }
                            ast::AttributeKind::Thread_Local => Some(GlobalFlag::ThreadLocal),
                            ast::AttributeKind::Unknown => {
                                error_attribute_unknown(emit, origin_id, attr);
                                None
                            }
                        };

                        //@check the flag
                    }

                    let id = hir.registry_mut().add_global(item, origin_id);
                    hir.add_symbol(
                        origin_id,
                        item.name.id,
                        Symbol::Defined {
                            kind: SymbolKind::Global(id),
                        },
                    );
                }
            },
            ast::Item::Import(..) => {}
        }
    }
}

pub fn name_already_defined_error(
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

fn error_attribute_unknown(emit: &mut HirEmit, origin_id: ModuleID, attr: ast::Attribute) {
    emit.error(ErrorComp::new(
        "unknown attribute",
        SourceRange::new(origin_id, attr.range),
        None,
    ));
}

fn error_attribute_cannot_apply(
    emit: &mut HirEmit,
    origin_id: ModuleID,
    attr: ast::Attribute,
    item_kind: &'static str,
) {
    emit.error(ErrorComp::new(
        format!(
            "cannot apply attribute #[{}] to {item_kind}",
            attr.kind.as_str()
        ),
        SourceRange::new(origin_id, attr.range),
        None,
    ));
}

const PROC_FLAG_ALL: [ProcFlag; 6] = [
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

trait AttributeFlag
where
    Self: Sized,
{
    fn into_u32(self) -> u32;
    fn as_str(self) -> &'static str;
    fn as_attribute(self) -> Option<ast::AttributeKind>;
    fn compatibility_set(self) -> BitSet;
    fn requirement_flag(self) -> Option<Self>;
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

    fn as_attribute(self) -> Option<ast::AttributeKind> {
        match self {
            ProcFlag::External => None,
            ProcFlag::Variadic => None,
            ProcFlag::Main => None,
            ProcFlag::Test => Some(ast::AttributeKind::Test),
            ProcFlag::Builtin => todo!(),
            ProcFlag::Inline => todo!(),
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

    fn requirement_flag(self) -> Option<ProcFlag> {
        match self {
            ProcFlag::External => None,
            ProcFlag::Variadic => Some(ProcFlag::External),
            ProcFlag::Main => None,
            ProcFlag::Test => None,
            ProcFlag::Builtin => None,
            ProcFlag::Inline => None,
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

    fn as_attribute(self) -> Option<ast::AttributeKind> {
        match self {
            GlobalFlag::ThreadLocal => Some(ast::AttributeKind::Thread_Local),
        }
    }

    fn compatibility_set(self) -> BitSet {
        match self {
            GlobalFlag::ThreadLocal => GLOBAL_FLAG_COMPAT_THREAD_LOCAL,
        }
    }

    fn requirement_flag(self) -> Option<GlobalFlag> {
        match self {
            GlobalFlag::ThreadLocal => None,
        }
    }
}

fn check_attribute_flag<FlagT: AttributeFlag + Copy + Clone>(
    emit: &mut HirEmit,
    attr_set: &mut BitSet,
    origin_id: ModuleID,
    attr_range: Option<TextRange>,
    new_flag: FlagT,
    all_flags: &[FlagT],
) {
    if attr_set.contains(new_flag.into_u32()) {
        if let Some(attr) = new_flag.as_attribute() {
            emit.warning(WarningComp::new(
                format!("duplicate attribute #[`{}`]", attr.as_str()),
                SourceRange::new(origin_id, attr_range.unwrap()),
                None,
            ));
        } else {
            unreachable!();
        }
        return;
    }

    if let Some(require_flag) = new_flag.requirement_flag() {
        if !attr_set.contains(require_flag.into_u32()) {
            // error: `flag` items must be `require_flag`
            return;
        }
    }

    let compat_set = new_flag.compatibility_set();

    for flag in all_flags {
        if attr_set.contains(flag.into_u32()) {
            if !compat_set.contains(flag.into_u32()) {
                eprintln!(
                    "attribute #[{}] cannot be applied to `{}` procedures",
                    new_flag.as_str(),
                    flag.as_str()
                );
                return;
            }
        }
    }
}

/*
#[test]
fn test_bitset() {
    let mut flag_set = BitSet::EMPTY;
    flag_set.set(ProcFlag::External);

    let set_flag = ProcFlag::Test;

    if flag_set.contains(set_flag) {
        eprintln!("#[test] already was set");
    } else {
        let require_flag = set_flag.requirement_flag();

        if let Some(require_flag) = require_flag {
            if !flag_set.contains(require_flag) {
                eprintln!(
                    "`{}` procedures must be `{}`",
                    set_flag.as_str(),
                    require_flag.as_str()
                );
                return;
            }
        }

        let compat_set = set_flag.compatibility_set();

        for flag in PROC_FLAG_ALL {
            if flag_set.contains(flag) {
                if !compat_set.contains(flag) {
                    eprintln!(
                        "attribute #[{}] cannot be applied to `{}` procedures",
                        set_flag.as_str(),
                        flag.as_str()
                    );
                    return;
                }
            }
        }

        flag_set.set(set_flag);
    }
}
*/
