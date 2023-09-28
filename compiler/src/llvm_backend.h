#ifndef LLVM_BACKEND_H
#define LLVM_BACKEND_H

#include "ast.h"
#include "llvm-c/Core.h"

struct Var_Meta
{
	StringView str;
	LLVMTypeRef var_type;
	LLVMValueRef var_value;
};

struct Backend_Block_Info
{
	u32 var_count;
};

//@Maybe finding by str might not work correctly
//due to ssa rules in more complex structures
//@Uniqueness of var names in scope should be checked by checker, which is disabled
struct Backend_Block_Scope
{
	void add_block();
	void pop_block();
	void add_var(const Var_Meta& var);
	std::optional<Var_Meta> find_var(StringView str);

	std::vector<Var_Meta> var_stack;
	std::vector<Backend_Block_Info> block_stack;
};

//@Todo nested struct decl type gen
struct Backend_LLVM
{
public:
	void backend_build(Ast* ast);

private:
	void build_ir(Ast* ast);
	void build_enum_decl(Ast_Enum_Decl* enum_decl);
	void build_struct_decl(Ast_Struct_Decl* struct_decl);
	void build_proc_decl(Ast_Proc_Decl* proc_decl);
	void build_proc_body(Ast_Proc_Decl* proc_decl);
	LLVMValueRef build_expr_value(Ast_Expr* expr, Backend_Block_Scope* bc);

	LLVMTypeRef basic_type_convert(BasicType basic_type);
	bool kind_is_ifd(LLVMTypeKind type_kind);
	bool kind_is_fd(LLVMTypeKind type_kind);
	bool kind_is_i(LLVMTypeKind type_kind);
	bool type_is_bool(LLVMTypeKind type_kind, LLVMTypeRef type_ref);
	char* get_c_string(Token& token);
	void error_exit(const char* message);

	void build_ir_example(Ast* ast);
	void build_binaries();
	void debug_print_module();

	LLVMContextRef context;
	LLVMModuleRef module;
	LLVMBuilderRef builder;

	struct Proc_Meta
	{
		LLVMTypeRef proc_type;
		LLVMValueRef proc_val;
	};

	struct Struct_Meta
	{
		LLVMTypeRef struct_type;
	};

	HashMap<StringView, Proc_Meta, u32, match_string_view> proc_decl_map;
	HashMap<StringView, Struct_Meta, u32, match_string_view> struct_decl_map;
};

#endif
