#ifndef LLVM_IR_TYPES_H
#define LLVM_IR_TYPES_H

#include "ast.h"
#include "llvm-c/Types.h"

enum class Terminator_Type;
struct Proc_Meta;
struct Loop_Meta;
struct Type_Meta;
struct Struct_Meta;
struct Field_Meta;
struct Var_Meta;
struct Var_Access_Meta;
struct Var_Block_Info;
struct Var_Block_Scope;

enum class Terminator_Type
{
	None,
	Return,
	Break,
	Continue,
};

struct Proc_Meta
{
	LLVMTypeRef proc_type;
	LLVMValueRef proc_val;
};

struct Loop_Meta
{
	LLVMBasicBlockRef break_target;
	LLVMBasicBlockRef continue_target;
	std::optional<Ast_Var_Assign*> continue_action;
};

struct Type_Meta
{
	LLVMTypeRef type;
	bool is_struct;
	Ast_Struct_Decl* struct_decl;
	bool is_pointer;
	LLVMTypeRef pointer_type;
};

struct Struct_Meta
{
	Ast_Struct_Decl* struct_decl;
	LLVMTypeRef struct_type;
};

struct Field_Meta
{
	u32 id;
	Type_Meta type_meta;
};

struct Var_Meta
{
	StringView str;
	LLVMValueRef var_value;
	Type_Meta type_meta;
};

struct Var_Access_Meta
{
	LLVMValueRef ptr;
	LLVMTypeRef type;
};

struct Var_Block_Info
{
	u32 var_count;
};

struct Var_Block_Scope
{
	void add_block();
	void pop_block();
	void add_var(const Var_Meta& var);
	std::optional<Var_Meta> find_var(StringView str);

	std::vector<Var_Meta> var_stack;
	std::vector<Var_Block_Info> block_stack;
};

#endif
