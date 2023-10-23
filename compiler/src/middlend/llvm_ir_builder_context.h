#ifndef LLVM_IR_BUILDER_CONTEXT_H
#define LLVM_IR_BUILDER_CONTEXT_H

#include "llvm-c/Types.h"
#include "frontend/ast.h"

typedef LLVMTypeRef Type;
typedef LLVMValueRef Value;
typedef LLVMBuilderRef Builder;
typedef LLVMBasicBlockRef Basic_Block;

struct IR_Builder_Context;
struct IR_Block_Info;
struct IR_Var_Info;
struct IR_Loop_Info;
struct IR_Access_Info;
enum class IR_Terminator;
enum class IR_Block_Flags;
enum class IR_Proc_Call_Flags;

void builder_context_init(IR_Builder_Context* bc, Ast_Program* program);
void builder_context_deinit(IR_Builder_Context* bc);
void builder_context_block_reset(IR_Builder_Context* bc, Value curr_proc);
void builder_context_block_add(IR_Builder_Context* bc);
void builder_context_block_pop_back(IR_Builder_Context* bc);
void builder_context_block_add_defer(IR_Builder_Context* bc, Ast_Defer* defer);
void builder_context_block_add_loop(IR_Builder_Context* bc, IR_Loop_Info loop_info);
void builder_context_block_add_var(IR_Builder_Context* bc, IR_Var_Info var_info);
IR_Var_Info builder_context_block_find_var(IR_Builder_Context* bc, Ast_Ident var_ident);
IR_Loop_Info builder_context_block_get_loop(IR_Builder_Context* bc);
Basic_Block builder_context_add_bb(IR_Builder_Context* bc, const char* name);
Basic_Block builder_context_get_bb(IR_Builder_Context* bc);
void builder_context_set_bb(IR_Builder_Context* bc, Basic_Block basic_block);

struct IR_Builder_Context
{
	Ast_Program* program;
	LLVMModuleRef module;
	Builder builder;
	Value curr_proc;
	std::vector<IR_Block_Info> blocks;
	std::vector<Ast_Defer*> defer_stack;
	std::vector<IR_Loop_Info> loop_stack;
	std::vector<IR_Var_Info> var_stack;
};

struct IR_Block_Info
{
	u32 defer_count;
	u32 loop_count;
	u32 var_count;
};

struct IR_Loop_Info
{
	Basic_Block break_block;
	Basic_Block continue_block;
	option<Ast_Var_Assign*> var_assign;
};

struct IR_Var_Info
{
	StringView str;
	Value ptr;
	Ast_Type ast_type;
};

struct IR_Access_Info
{
	Value ptr;
	Type type;
};

enum class IR_Terminator
{
	None,
	Break,
	Return,
	Continue,
};

enum class IR_Block_Flags
{
	None,
	Already_Added,
};

enum class IR_Proc_Call_Flags
{
	In_Expr,
	In_Statement,
};

#endif
