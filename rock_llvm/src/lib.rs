mod emit;
mod llvm;
mod sys;

use emit::emit_mod;
use rock_core::hir;

pub fn codegen(hir: hir::Hir) {
    emit_mod::codegen_module(hir);
}
