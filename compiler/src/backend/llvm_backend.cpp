#include "llvm_backend.h"

#include "llvm-c/Core.h"
#include "llvm-c/Analysis.h"
#include "llvm-c/TargetMachine.h"
#include <stdio.h>

void backend_build_module(LLVMModuleRef mod)
{
	backend_verify_module(mod);

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
	
	LLVMTargetMachineEmitToFile(machine, mod, "result.o", LLVMObjectFile, &error);
	if (error != NULL) printf("error: %s\n", error);
	
	LLVMDisposeModule(mod);

	LLVMDisposeMessage(error);
	LLVMDisposeMessage(cpu);
	LLVMDisposeMessage(cpu_features);
	LLVMDisposeMessage(triple);

	backend_run_clang();
}

void backend_run()
{
	system("result");
}

void backend_verify_module(LLVMModuleRef mod)
{
	LLVMPrintModuleToFile(mod, "output.ll", NULL);
	LLVMVerifyModule(mod, LLVMPrintMessageAction, NULL);
}

void backend_run_clang()
{
	const char* cmd = "clang -o result.exe result.o";
	system(cmd);
}
