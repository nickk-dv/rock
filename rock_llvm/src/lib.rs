pub mod build;
mod llvm;
mod sys;

use build::emit_mod;
use rock_core::hir;

pub fn codegen(hir: hir::Hir) {
    emit_mod::codegen_module(hir);
}
