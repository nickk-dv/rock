#ifndef LLVM_BACKEND_H
#define LLVM_BACKEND_H

#include "llvm-c/Types.h"

void backend_build_module(LLVMModuleRef module);
void backend_run();

static void backend_verify_module(LLVMModuleRef module);
static void backend_run_clang();

#endif
