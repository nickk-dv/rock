use super::hir_build::HirEmit;
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::session::{ModuleID, Session};

fn resolve_attr(emit: &mut HirEmit, session: &Session, origin_id: ModuleID, attr: &ast::Attr) {
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

    //@check if duplicate or can be applied at all

    let require_params = kind.requires_params();
    if let Some((params, params_range)) = attr.params {
        if require_params && params.is_empty() {
            emit.error(ErrorComp::new(
                format!("attribute `{attr_name}` requires non-empty parameter list"),
                SourceRange::new(origin_id, params_range),
                None,
            ));
            return;
        }
    } else {
        if require_params {
            emit.error(ErrorComp::new(
                format!("attribute `{attr_name}` requires parameter list"),
                SourceRange::new(origin_id, attr.range),
                None,
            ));
            return;
        }
    }
}

//@variants / fields / stmts 25.08.24
// cannot have attrs applied currently
enum AttrTargetKind {
    Proc,
    Enum,
    EnumVariant,
    Struct,
    StructField,
    Const,
    Global,
    Import,
    Statement,
}

enum ReprKind {
    ReprC,
    ReprInt(hir::BasicInt),
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq)]
pub enum AttrKind {
    Cfg,
    Cfg_Not,
    Cfg_Any,
    Test,
    Builtin,
    Inline,
    Repr,
    Thread_Local,
}

impl AttrKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AttrKind::Cfg => "cfg",
            AttrKind::Cfg_Not => "cfg_not",
            AttrKind::Cfg_Any => "cfg_any",
            AttrKind::Test => "test",
            AttrKind::Builtin => "builtin",
            AttrKind::Inline => "inline",
            AttrKind::Repr => "repr",
            AttrKind::Thread_Local => "thread_local",
        }
    }

    pub fn from_str(string: &str) -> Option<AttrKind> {
        match string {
            "cfg" => Some(AttrKind::Cfg),
            "cfg_not" => Some(AttrKind::Cfg_Not),
            "cfg_any" => Some(AttrKind::Cfg_Any),
            "test" => Some(AttrKind::Test),
            "builtin" => Some(AttrKind::Builtin),
            "inline" => Some(AttrKind::Inline),
            "repr" => Some(AttrKind::Repr),
            "thread_local" => Some(AttrKind::Thread_Local),
            _ => None,
        }
    }

    pub fn requires_params(self) -> bool {
        match self {
            AttrKind::Cfg | AttrKind::Cfg_Not | AttrKind::Cfg_Any => true,
            AttrKind::Test | AttrKind::Builtin | AttrKind::Inline => false,
            AttrKind::Repr => true,
            AttrKind::Thread_Local => false,
        }
    }
}
