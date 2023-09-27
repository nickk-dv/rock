#ifndef LLVM_BACKEND_H
#define LLVM_BACKEND_H

#include "ast.h"
#include "llvm-c/Core.h"

struct Backend_LLVM
{
public:
	void backend_build(Ast* ast);

private:
	void backend_build_ir(Ast* ast);
	void backend_build_enum_decl(Ast_Enum_Decl* enum_decl);
	void backend_build_struct_decl(Ast_Struct_Decl* struct_decl);
	void backend_build_proc_decl(Ast_Proc_Decl* proc_decl);
	void backend_build_proc_body(Ast_Proc_Decl* proc_decl);

	LLVMTypeRef basic_type_convert(BasicType basic_type);
	char* get_c_string(Token& token);
	void error_exit(const char* message);

	void backend_build_ir_example(Ast* ast);
	void backend_build_binaries();
	void backend_debug_print_module();

	LLVMContextRef context;
	LLVMModuleRef module;
	LLVMBuilderRef builder;

	struct Proc_Meta
	{
		LLVMTypeRef proc_type;
		LLVMValueRef proc_val;
	};

	HashMap<StringView, Proc_Meta, u32, match_string_view> proc_decl_map;
};

#endif
