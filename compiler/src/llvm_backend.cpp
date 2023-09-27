#include "llvm_backend.h"

#include "llvm-c/TargetMachine.h"

void Backend_LLVM::backend_build(Ast* ast)
{
	context = LLVMContextCreate();
	module = LLVMModuleCreateWithNameInContext("module", context);
	builder = LLVMCreateBuilderInContext(context);
	backend_build_ir(ast);
	backend_build_binaries();
}

void Backend_LLVM::backend_build_ir(Ast* ast)
{
	for (Ast_Enum_Decl* enum_decl : ast->enums) { backend_build_enum_decl(enum_decl); }
	for (Ast_Struct_Decl* struct_decl : ast->structs) { backend_build_struct_decl(struct_decl); }
	proc_decl_map.init(32);
	for (Ast_Proc_Decl* proc_decl : ast->procs) { backend_build_proc_decl(proc_decl); }
	for (Ast_Proc_Decl* proc_decl : ast->procs) { backend_build_proc_body(proc_decl); }
	LLVMDisposeBuilder(builder);
}

void Backend_LLVM::backend_build_enum_decl(Ast_Enum_Decl* enum_decl)
{
	for (u32 i = 0; i < enum_decl->variants.size(); i++)
	{
		LLVMValueRef enum_constant = LLVMAddGlobal(module, LLVMInt32TypeInContext(context), get_c_string(enum_decl->variants[i].token));
		LLVMSetInitializer(enum_constant, LLVMConstInt(LLVMInt32TypeInContext(context), enum_decl->constants[i], 0));
		LLVMSetGlobalConstant(enum_constant, 1);
	}
}

void Backend_LLVM::backend_build_struct_decl(Ast_Struct_Decl* struct_decl)
{
	//@Hack temporary member types
	LLVMTypeRef members[] = { basic_type_convert(BASIC_TYPE_F32), basic_type_convert(BASIC_TYPE_F64) };
	
	LLVMTypeRef struct_type = LLVMStructCreateNamed(context, get_c_string(struct_decl->type.token));
	LLVMStructSetBody(struct_type, members, 2, 0);

	//@Hack adding global var to see the struct declaration in the ir
	LLVMValueRef globalVar = LLVMAddGlobal(module, struct_type, "global_to_see_struct");
}

// @Usefull functions:
// LLVMTypeRef LLVMTypeOf(LLVMValueRef Val);
// LLVMGetAllocatedType - get type of value on a stack

void Backend_LLVM::backend_build_proc_decl(Ast_Proc_Decl* proc_decl)
{
	LLVMTypeRef proc_type = LLVMFunctionType(LLVMVoidType(), NULL, 0, 0);
	LLVMValueRef proc_val = LLVMAddFunction(module, get_c_string(proc_decl->ident.token), proc_type);
	Proc_Meta meta = { proc_type, proc_val };
	proc_decl_map.add(proc_decl->ident.token.string_value, meta, hash_fnv1a_32(proc_decl->ident.token.string_value));
}

void Backend_LLVM::backend_build_proc_body(Ast_Proc_Decl* proc_decl)
{
	auto proc_meta = proc_decl_map.find(proc_decl->ident.token.string_value, hash_fnv1a_32(proc_decl->ident.token.string_value));
	if (!proc_meta) { error_exit("failed to find proc declaration while building its body"); return; }
	LLVMBasicBlockRef entry_block = LLVMAppendBasicBlockInContext(context, proc_meta->proc_val, "entry");
	LLVMPositionBuilderAtEnd(builder, entry_block);
	
	Ast_Block* block = proc_decl->block;
	for (Ast_Statement* statement : block->statements)
	{
		switch (statement->tag)
		{
			case Ast_Statement::Tag::If:
			{

			} break;
			case Ast_Statement::Tag::For:
			{

			} break;
			case Ast_Statement::Tag::Break:
			{

			} break;
			case Ast_Statement::Tag::Return:
			{
				Ast_Return* _return = statement->as_return;
				if (_return->expr.has_value()) error_exit("return with expr is not supported");
				LLVMBuildRet(builder, NULL); //ret void
			} break;
			case Ast_Statement::Tag::Continue:
			{

			} break;
			case Ast_Statement::Tag::Proc_Call:
			{
				Ast_Proc_Call* proc_call = statement->as_proc_call;
				if (!proc_call->input_exprs.empty()) error_exit("proc call with input exprs is not supported");

				auto proc_meta = proc_decl_map.find(proc_call->ident.token.string_value, hash_fnv1a_32(proc_call->ident.token.string_value));
				if (!proc_meta) { error_exit("failed to find proc declaration while trying to call it"); return; }
				//@Notice usage of return values must be enforced on checking stage, statement proc call should return nothing
				LLVMValueRef ret_val = LLVMBuildCall2(builder, proc_meta.value().proc_type, proc_meta.value().proc_val, NULL, 0, "ret_val");
			} break;
			case Ast_Statement::Tag::Var_Decl:
			{
				Ast_Var_Decl* var_decl = statement->as_var_decl;
				if (var_decl->expr.has_value()) error_exit("var decl is only supported with default init");
				if (!var_decl->type.has_value()) error_exit("var decl expected type to be known");
				
				Ast_Type* type = var_decl->type.value();
				if (type->tag != Ast_Type::Tag::Basic) error_exit("var decl is only supported with basic types");

				LLVMTypeRef var_type = basic_type_convert(type->as_basic);
				LLVMValueRef var_ptr = LLVMBuildAlloca(builder, var_type, get_c_string(var_decl->ident.token));
				LLVMBuildStore(builder, LLVMConstNull(var_type), var_ptr); //LLVMMemset might be better for 0-ing structs or arrays
			} break;
			case Ast_Statement::Tag::Var_Assign:
			{
				Ast_Var_Assign* var_assign = statement->as_var_assign;
			} break;
			default: break;
		}
	}
	
	LLVMBuildRet(builder, NULL); //entry block needs to have ret void
}

LLVMTypeRef Backend_LLVM::basic_type_convert(BasicType basic_type) //@Uints shouild use different bitwith?
{
	switch (basic_type)
	{
		case BASIC_TYPE_I8: return LLVMInt8Type();
		case BASIC_TYPE_U8: return LLVMInt8Type();
		case BASIC_TYPE_I16: return LLVMInt16Type();
		case BASIC_TYPE_U16: return LLVMInt16Type();
		case BASIC_TYPE_I32: return LLVMInt32Type();
		case BASIC_TYPE_U32: return LLVMInt32Type();
		case BASIC_TYPE_I64: return LLVMInt64Type();
		case BASIC_TYPE_U64: return LLVMInt64Type();
		case BASIC_TYPE_F32: return LLVMFloatType();
		case BASIC_TYPE_F64: return LLVMDoubleType();
		case BASIC_TYPE_BOOL: return LLVMInt1Type();
		//@Undefined for now, might use a struct: case BASIC_TYPE_STRING:
		default: error_exit("basic type not found (basic_type_convert)"); break;
	}
	return LLVMVoidType();
}

char* Backend_LLVM::get_c_string(Token& token) //@Unsafe hack to get c string from string view of source file string, need to do smth better
{
	token.string_value.data[token.string_value.count] = 0;
	return (char*)token.string_value.data;
}

void Backend_LLVM::error_exit(const char* message)
{
	printf("backend error: %s.\n", message);
	exit(EXIT_FAILURE);
}

void Backend_LLVM::backend_build_ir_example(Ast* ast)
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
}

void Backend_LLVM::backend_build_binaries()
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
	LLVMSetTarget(module, triple);

	LLVMTargetMachineRef machine = LLVMCreateTargetMachine
	(target, triple, cpu, cpu_features, LLVMCodeGenLevelDefault, LLVMRelocDefault, LLVMCodeModelDefault);
	
	LLVMTargetDataRef datalayout = LLVMCreateTargetDataLayout(machine);
	char* datalayout_str = LLVMCopyStringRepOfTargetData(datalayout);
	LLVMSetDataLayout(module, datalayout_str);
	LLVMDisposeMessage(datalayout_str);
	backend_debug_print_module();

	LLVMTargetMachineEmitToFile(machine, module, "result.o", LLVMObjectFile, &error);
	if (error != NULL) printf("error: %s\n", error);
	
	LLVMDisposeModule(module);
	LLVMContextDispose(context);

	LLVMDisposeMessage(error);
	LLVMDisposeMessage(cpu);
	LLVMDisposeMessage(cpu_features);
	LLVMDisposeMessage(triple);
}

void Backend_LLVM::backend_debug_print_module()
{
	LLVMPrintModuleToFile(module, "output.ll", NULL);
	char* message = LLVMPrintModuleToString(module);
	printf("Module: %s", message);
	LLVMDisposeMessage(message);
}
