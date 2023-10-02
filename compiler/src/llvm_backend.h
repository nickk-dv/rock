#ifndef LLVM_BACKEND_H
#define LLVM_BACKEND_H

#include "llvm-c/Types.h"

struct LLVM_Backend
{
	void build_binaries(LLVMModuleRef module);
	void debug_print_module(LLVMModuleRef module);
};

#endif
