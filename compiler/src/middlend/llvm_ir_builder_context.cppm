module;
#include "llvm-c/Core.h"

export module llmv_ir_builder_context;

import ast;

export typedef LLVMTypeRef Type;
export typedef LLVMValueRef Value;
export typedef LLVMBuilderRef Builder;
export typedef LLVMBasicBlockRef Basic_Block;

struct IR_Builder_Context;
struct IR_Block_Info;
struct IR_Loop_Info;
struct IR_Var_Info;

export void builder_context_init(IR_Builder_Context* bc, Ast_Program* program);
export void builder_context_deinit(IR_Builder_Context* bc);
export void builder_context_block_reset(IR_Builder_Context* bc, Value curr_proc);
export void builder_context_block_add(IR_Builder_Context* bc);
export void builder_context_block_pop_back(IR_Builder_Context* bc);
export void builder_context_block_add_defer(IR_Builder_Context* bc, Ast_Stmt_Defer* defer);
export void builder_context_block_add_loop(IR_Builder_Context* bc, IR_Loop_Info loop_info);
export void builder_context_block_add_var(IR_Builder_Context* bc, IR_Var_Info var_info);
export IR_Var_Info builder_context_block_find_var(IR_Builder_Context* bc, Ast_Ident var_ident);
export IR_Loop_Info builder_context_block_get_loop(IR_Builder_Context* bc);
export Basic_Block builder_context_add_bb(IR_Builder_Context* bc, const char* name);
export Basic_Block builder_context_get_bb(IR_Builder_Context* bc);
export void builder_context_set_bb(IR_Builder_Context* bc, Basic_Block basic_block);

export struct IR_Builder_Context
{
	Ast_Program* program;
	LLVMModuleRef module;
	Builder builder;
	Value curr_proc;
	std::vector<IR_Block_Info> blocks;
	std::vector<Ast_Stmt_Defer*> defer_stack;
	std::vector<IR_Loop_Info> loop_stack;
	std::vector<IR_Var_Info> var_stack;
};

export struct IR_Block_Info
{
	u32 defer_count;
	u32 loop_count;
	u32 var_count;
};

export struct IR_Loop_Info
{
	Basic_Block break_block;
	Basic_Block continue_block;
	option<Ast_Stmt_Var_Assign*> var_assign;
};

export struct IR_Var_Info
{
	StringView str;
	Value ptr;
	Ast_Type ast_type;
};

export struct IR_Access_Info
{
	Value ptr;
	Type type;
};

export enum class IR_Terminator
{
	None,
	Break,
	Return,
	Continue,
};

export enum class IR_Block_Flags
{
	None,
	Already_Added,
};

export enum class IR_Proc_Call_Flags
{
	In_Expr,
	In_Statement,
};

export void builder_context_init(IR_Builder_Context* bc, Ast_Program* program)
{
	bc->program = program;
	bc->module = LLVMModuleCreateWithName("program_module");
	bc->builder = LLVMCreateBuilder();
}

export void builder_context_deinit(IR_Builder_Context* bc)
{
	LLVMDisposeBuilder(bc->builder);
}

export void builder_context_block_reset(IR_Builder_Context* bc, Value curr_proc)
{
	bc->curr_proc = curr_proc;
	bc->blocks.clear();
	bc->defer_stack.clear();
	bc->loop_stack.clear();
}

export void builder_context_block_add(IR_Builder_Context* bc)
{
	bc->blocks.emplace_back(IR_Block_Info { 0, 0, 0 });
}

export void builder_context_block_pop_back(IR_Builder_Context* bc)
{
	IR_Block_Info block_info = bc->blocks[bc->blocks.size() - 1];
	for (u32 i = 0; i < block_info.defer_count; i += 1) bc->defer_stack.pop_back();
	for (u32 i = 0; i < block_info.loop_count; i += 1) bc->loop_stack.pop_back();
	for (u32 i = 0; i < block_info.var_count; i += 1) bc->var_stack.pop_back();
	bc->blocks.pop_back();
}

export void builder_context_block_add_defer(IR_Builder_Context* bc, Ast_Stmt_Defer* defer)
{
	bc->blocks[bc->blocks.size() - 1].defer_count += 1;
	bc->defer_stack.emplace_back(defer);
}

export void builder_context_block_add_loop(IR_Builder_Context* bc, IR_Loop_Info loop_info)
{
	bc->blocks[bc->blocks.size() - 1].loop_count += 1;
	bc->loop_stack.emplace_back(loop_info);
}

export void builder_context_block_add_var(IR_Builder_Context* bc, IR_Var_Info var_info)
{
	bc->blocks[bc->blocks.size() - 1].var_count += 1;
	bc->var_stack.emplace_back(var_info);
}

export IR_Var_Info builder_context_block_find_var(IR_Builder_Context* bc, Ast_Ident var_ident)
{
	for (IR_Var_Info& var : bc->var_stack)
	{
		if (var.str == var_ident.str) return var;
	}
	return {};
}

export IR_Loop_Info builder_context_block_get_loop(IR_Builder_Context* bc)
{
	return bc->loop_stack[bc->loop_stack.size() - 1];
}

export Basic_Block builder_context_add_bb(IR_Builder_Context* bc, const char* name)
{
	return LLVMAppendBasicBlock(bc->curr_proc, name);
}

export Basic_Block builder_context_get_bb(IR_Builder_Context* bc)
{
	return LLVMGetInsertBlock(bc->builder);
}

export void builder_context_set_bb(IR_Builder_Context* bc, Basic_Block basic_block)
{
	LLVMPositionBuilderAtEnd(bc->builder, basic_block);
}
