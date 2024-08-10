mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;

use rock_core::hir;

#[derive(Copy, Clone)]
pub enum BuildKind {
    Debug,
    Release,
}

impl BuildKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildKind::Debug => "debug",
            BuildKind::Release => "release",
        }
    }
}

pub fn build(hir: hir::Hir) {
    emit_mod::codegen_module(hir);
}
