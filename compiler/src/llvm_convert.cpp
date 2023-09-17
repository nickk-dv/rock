#include "llvm-c/Core.h"

void llvm_convert_build(Ast* ast)
{
	LLVMModuleRef mod = LLVMModuleCreateWithName("main_module");
	
	char* message = LLVMPrintModuleToString(mod);
	printf("Module: %s", message);
	LLVMDisposeMessage(message);
	
	LLVMDisposeModule(mod);
}
