#include "llvm-c/Core.h"
#include "llvm-c/TargetMachine.h"

void llvm_convert_build(Ast* ast);

void llvm_convert_build(Ast* ast)
{
	LLVMContextRef context = LLVMContextCreate();
	LLVMModuleRef mod = LLVMModuleCreateWithNameInContext("main_module", context);
	LLVMBuilderRef builder = LLVMCreateBuilderInContext(context);
	
	// Create and add function prototype for sum
	LLVMTypeRef param_types[] = { LLVMInt32Type(), LLVMInt32Type() };
	LLVMTypeRef sum_proc_type = LLVMFunctionType(LLVMInt32Type(), param_types, 2, 0);
	LLVMValueRef sum_proc = LLVMAddFunction(mod, "sum", sum_proc_type);
	LLVMBasicBlockRef block = LLVMAppendBasicBlockInContext(context, sum_proc, "block");
	LLVMPositionBuilderAtEnd(builder, block);
	LLVMValueRef sum_value = LLVMBuildAdd(builder, LLVMGetParam(sum_proc, 0), LLVMGetParam(sum_proc, 1), "sum_value");
	LLVMBuildRet(builder, sum_value);

	// Create and add function prototype for main
	LLVMTypeRef main_ret_type = LLVMFunctionType(LLVMInt32Type(), NULL, 0, 0);
	LLVMValueRef main_func = LLVMAddFunction(mod, "main", main_ret_type);
	LLVMBasicBlockRef main_block = LLVMAppendBasicBlockInContext(context, main_func, "main_block");
	LLVMPositionBuilderAtEnd(builder, main_block);

	// Call the sum function with arguments
	LLVMValueRef args[] = { LLVMConstInt(LLVMInt32Type(), 39, 0), LLVMConstInt(LLVMInt32Type(), 30, 0) };
	LLVMValueRef sum_result = LLVMBuildCall2(builder, sum_proc_type, sum_proc, args, 2, "sum_result");
	LLVMBuildRet(builder, sum_result);

	//@Todo setup ErrorHandler from ErrorHandling.h to not crash with exit(1)
	char* message = LLVMPrintModuleToString(mod);
	printf("Module: %s", message);
	LLVMDisposeMessage(message);

	LLVMInitializeAllTargetInfos();
	LLVMInitializeAllTargets();
	LLVMInitializeAllTargetMCs();
	LLVMInitializeAllAsmParsers();
	LLVMInitializeAllAsmPrinters();
	//@Performance: any benefits of doing only init for one platform?
	//LLVMInitializeX86TargetInfo();
	//LLVMInitializeX86Target();
	//LLVMInitializeX86TargetMC();

	char* errors = 0;
	LLVMTargetRef target;
	LLVMGetTargetFromTriple(LLVMGetDefaultTargetTriple(), &target, &errors);
	char* triple = LLVMGetDefaultTargetTriple();
	char* cpu = LLVMGetHostCPUName();
	char* cpu_features = LLVMGetHostCPUFeatures();

	LLVMTargetMachineRef machine = LLVMCreateTargetMachine
	(target, triple, cpu, cpu_features, LLVMCodeGenLevelDefault, LLVMRelocDefault, LLVMCodeModelDefault);
	
	LLVMSetTarget(mod, LLVMGetDefaultTargetTriple());
	LLVMTargetDataRef datalayout = LLVMCreateTargetDataLayout(machine);
	char* datalayout_str = LLVMCopyStringRepOfTargetData(datalayout);
	printf("datalayout: %s\n", datalayout_str);
	LLVMSetDataLayout(mod, datalayout_str);
	LLVMDisposeMessage(datalayout_str);

	LLVMTargetMachineEmitToFile(machine, mod, "result.o", LLVMObjectFile, &errors);
	printf("error: %s\n", errors);
	LLVMDisposeMessage(errors);

	LLVMDisposeMessage(triple);
	LLVMDisposeMessage(cpu);
	LLVMDisposeMessage(cpu_features);
	LLVMDisposeMessage(errors);
	
	LLVMDisposeBuilder(builder);
	LLVMDisposeModule(mod);
	LLVMContextDispose(context);
}
