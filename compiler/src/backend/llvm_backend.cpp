#include "llvm_backend.h"

#include "llvm-c/Core.h"
#include "llvm-c/Analysis.h"
#include "llvm-c/TargetMachine.h"
#include <stdio.h>

void backend_build_module(LLVMModuleRef mod)
{
	//@Todo setup ErrorHandler from ErrorHandling.h to not crash with exit(1)
	//even during IR building for dev period
	//@Performance: any benefits of doing only init for one platform?
	//LLVMInitializeX86TargetInfo() ...
	LLVMInitializeAllTargetInfos();
	LLVMInitializeAllTargets();
	LLVMInitializeAllTargetMCs();
	LLVMInitializeAllAsmParsers();
	LLVMInitializeAllAsmPrinters();
	
	LLVMTargetRef target;
	char* error = 0;
	char* cpu = LLVMGetHostCPUName();
	char* cpu_features = LLVMGetHostCPUFeatures();
	char* triple = LLVMGetDefaultTargetTriple();
	LLVMGetTargetFromTriple(triple, &target, &error);
	LLVMSetTarget(mod, triple);

	LLVMTargetMachineRef machine = LLVMCreateTargetMachine
	(target, triple, cpu, cpu_features, LLVMCodeGenLevelDefault, LLVMRelocDefault, LLVMCodeModelDefault);
	
	LLVMTargetDataRef datalayout = LLVMCreateTargetDataLayout(machine);
	char* datalayout_str = LLVMCopyStringRepOfTargetData(datalayout);
	LLVMSetDataLayout(mod, datalayout_str);
	LLVMDisposeMessage(datalayout_str);
	backend_print_module(mod);
	
	LLVMTargetMachineEmitToFile(machine, mod, "result.o", LLVMObjectFile, &error);
	if (error != NULL) printf("error: %s\n", error);
	
	LLVMDisposeModule(mod);

	LLVMDisposeMessage(error);
	LLVMDisposeMessage(cpu);
	LLVMDisposeMessage(cpu_features);
	LLVMDisposeMessage(triple);
}

void backend_print_module(LLVMModuleRef mod)
{
	LLVMPrintModuleToFile(mod, "output.ll", NULL);
	char* message = LLVMPrintModuleToString(mod);
	printf("Module: %s", message);
	LLVMDisposeMessage(message);
	LLVMVerifyModule(mod, LLVMPrintMessageAction, NULL);
}
