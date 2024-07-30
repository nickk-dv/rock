use super::context::{Codegen, ProcCodegen};
use crate::llvm;
use rock_core::ast;
use rock_core::hir;

pub fn codegen_const_value(cg: &Codegen, value: hir::ConstValue) -> llvm::Value {
    //@temp
    llvm::const_float(cg.basic_type(ast::BasicType::F64), 0.0)
}

pub fn codegen_expr_value(
    cg: &Codegen,
    proc_cg: &mut ProcCodegen,
    expr: &hir::Expr,
) -> llvm::Value {
    //@temp
    llvm::const_float(cg.basic_type(ast::BasicType::F64), 0.0)
}
