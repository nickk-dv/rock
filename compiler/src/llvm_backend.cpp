#include "llvm_backend.h"

#include "llvm-c/TargetMachine.h"

void llvm_build(Ast* ast)
{
	LLVMModuleRef mod = llvm_build_ir(ast);
	llvm_build_binaries(mod);
}

LLVMModuleRef llvm_build_ir(Ast* ast)
{
	LLVMContextRef context = LLVMContextCreate();
	LLVMModuleRef mod = LLVMModuleCreateWithNameInContext("module", context);
	LLVMBuilderRef builder = LLVMCreateBuilderInContext(context);

	for (const Ast_Struct_Decl& struct_decl : ast->structs)
	{
		//@Hack temporary member types
		LLVMTypeRef members[] = { LLVMInt32TypeInContext(context), LLVMDoubleTypeInContext(context) };
		
		Token t = struct_decl.type.token;
		t.string_value.data[t.string_value.count] = 0;
		LLVMTypeRef struct_type = LLVMStructCreateNamed(context, (char*)t.string_value.data);
		LLVMStructSetBody(struct_type, members, 2, 0);

		//@Hack adding global var to see the struct declaration in the ir
		LLVMValueRef globalVar = LLVMAddGlobal(mod, struct_type, "global_to_see_struct");
	}

	for (const Ast_Enum_Decl& enum_decl : ast->enums)
	{
		for (u32 i = 0; i < enum_decl.variants.size(); i++)
		{
			Token t = enum_decl.variants[i].ident.token;
			t.string_value.data[t.string_value.count] = 0;
			LLVMValueRef enum_constant = LLVMAddGlobal(mod, LLVMInt32TypeInContext(context), (char*)t.string_value.data);
			LLVMSetInitializer(enum_constant, LLVMConstInt(LLVMInt32TypeInContext(context), enum_decl.constants[i], 0));
			LLVMSetGlobalConstant(enum_constant, 1);
		}
	}

	for (const Ast_Proc_Decl& proc_decl : ast->procs)
	{
		LLVMTypeRef proc_type = LLVMFunctionType(LLVMVoidType(), NULL, 0, 0);
		Token t = proc_decl.ident.token; //@Hack inserting null terminator to source data (should never override other string or identifier)
		t.string_value.data[t.string_value.count] = 0;
		LLVMValueRef proc = LLVMAddFunction(mod, (char*)t.string_value.data, proc_type);
		LLVMBasicBlockRef proc_block = LLVMAppendBasicBlockInContext(context, proc, "block");
		LLVMPositionBuilderAtEnd(builder, proc_block);
		LLVMBuildRet(builder, NULL);
	}

	//LLVMTypeRef main_func_type = LLVMFunctionType(LLVMInt32Type(), NULL, 0, 0);
	//LLVMValueRef main_func = LLVMAddFunction(mod, "main", main_func_type);
	//LLVMBasicBlockRef main_block = LLVMAppendBasicBlockInContext(context, main_func, "block");
	//LLVMPositionBuilderAtEnd(builder, main_block);
	//LLVMBuildRet(builder, LLVMConstInt(LLVMInt32Type(), 0, 0));

	LLVMDisposeBuilder(builder);
	return mod;
}

LLVMModuleRef llvm_build_ir_example(Ast* ast)
{
	LLVMContextRef context = LLVMContextCreate();
	LLVMModuleRef mod = LLVMModuleCreateWithNameInContext("module", context);
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

	LLVMDisposeBuilder(builder);
	return mod;
}

void llvm_build_binaries(LLVMModuleRef mod)
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
	llvm_debug_print_module(mod);

	LLVMTargetMachineEmitToFile(machine, mod, "result.o", LLVMObjectFile, &error);
	if (error != NULL) printf("error: %s\n", error);
	
	LLVMContextRef context = LLVMGetModuleContext(mod);
	LLVMDisposeModule(mod);
	LLVMContextDispose(context);

	LLVMDisposeMessage(error);
	LLVMDisposeMessage(cpu);
	LLVMDisposeMessage(cpu_features);
	LLVMDisposeMessage(triple);
}

void llvm_debug_print_module(LLVMModuleRef mod)
{
	LLVMPrintModuleToFile(mod, "output.ll", NULL);
	char* message = LLVMPrintModuleToString(mod);
	printf("Module: %s", message);
	LLVMDisposeMessage(message);
}
