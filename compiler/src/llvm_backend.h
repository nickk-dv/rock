#ifndef LLVM_BACKEND_H
#define LLVM_BACKEND_H

#include "llvm-c/Types.h"

void backend_build_module(LLVMModuleRef module);
void backend_print_module(LLVMModuleRef module);

#endif
