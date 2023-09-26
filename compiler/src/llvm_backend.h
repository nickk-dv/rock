#ifndef LLVM_BACKEND_H
#define LLVM_BACKEND_H

#include "ast.h"
#include "llvm-c/Core.h"

void llvm_build(Ast* ast);

LLVMModuleRef llvm_build_ir(Ast* ast);
LLVMModuleRef llvm_build_ir_example(Ast* ast);
void llvm_build_binaries(LLVMModuleRef mod);
void llvm_debug_print_module(LLVMModuleRef mod);

#endif
