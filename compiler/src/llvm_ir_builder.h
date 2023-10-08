#ifndef LLVM_IR_BUILDER_H
#define LLVM_IR_BUILDER_H

#include "ast.h"
#include "llvm-c/Types.h"
#include "llvm_ir_types.h"

struct LLVM_IR_Builder
{
public:
	LLVMModuleRef build_module(Ast* ast);

private:
	void build_enum_decl(Ast_Enum_Decl* enum_decl);
	void build_struct_decl(Ast_Struct_Decl* struct_decl);
	void build_proc_decl(Ast_Proc_Decl* proc_decl);
	void build_proc_body(Ast_Proc_Decl* proc_decl);

	Terminator_Type build_block(Ast_Block* block, Var_Block_Scope* bc, bool defer, std::optional<Loop_Meta> loop_meta = {}, bool entry = false);
	void build_defer(Ast_Block* block, Var_Block_Scope* bc, bool all_defers);
	void build_if(Ast_If* _if, LLVMBasicBlockRef cont_block, Var_Block_Scope* bc, bool defer, std::optional<Loop_Meta> loop_meta = {});
	void build_for(Ast_For* _for, Var_Block_Scope* bc, bool defer);
	LLVMValueRef build_proc_call(Ast_Proc_Call* _for, Var_Block_Scope* bc, bool is_statement);
	void build_var_decl(Ast_Var_Decl* var_decl, Var_Block_Scope* bc);
	void build_var_assign(Ast_Var_Assign* var_assign, Var_Block_Scope* bc);
	LLVMValueRef build_expr_value(Ast_Expr* expr, Var_Block_Scope* bc, bool adress_op = false);
	LLVMValueRef build_value_cast(LLVMValueRef value, LLVMTypeRef target_type);
	void build_binary_value_cast(LLVMValueRef& value_lhs, LLVMValueRef& value_rhs, LLVMTypeRef type_lhs, LLVMTypeRef type_rhs);

	LLVMTypeRef get_basic_type(BasicType type);
	Type_Meta get_type_meta(Ast_Type* type);
	LLVMValueRef get_enum_value(Ast_Enum* _enum);
	Field_Meta get_field_meta(Ast_Struct_Decl* struct_decl, StringView field_str);
	Var_Access_Meta get_var_access_meta(Ast_Var* var, Var_Block_Scope* bc);
	bool type_is_int(LLVMTypeRef type);
	bool type_is_bool(LLVMTypeRef type);
	bool type_is_float(LLVMTypeRef type);
	bool type_is_f32(LLVMTypeRef type);
	bool type_is_f64(LLVMTypeRef type);
	bool type_is_pointer(LLVMTypeRef type);
	u32 type_int_bit_witdh(LLVMTypeRef type);
	char* get_c_string(Ast_Ident& ident);
	void error_exit(const char* message);
	void debug_print_llvm_type(const char* message, LLVMTypeRef type);
	void set_curr_block(LLVMBasicBlockRef block);

	LLVMModuleRef module;
	LLVMBuilderRef builder;
	LLVMValueRef proc_value;
	HashMap<StringView, Enum_Meta, u32, match_string_view> enum_decl_map;
	HashMap<StringView, Proc_Meta, u32, match_string_view> proc_decl_map;
	HashMap<StringView, Struct_Meta, u32, match_string_view> struct_decl_map;
};

#endif
