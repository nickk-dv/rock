use super::context::{Codegen, ProcCodegen};
use crate::llvm;
use rock_core::ast;
use rock_core::hir;

pub fn codegen_const_value(cg: &Codegen, value: hir::ConstValue) -> llvm::Value {
    llvm::const_float(cg.basic_type(ast::BasicType::F64), 0.0)
}
