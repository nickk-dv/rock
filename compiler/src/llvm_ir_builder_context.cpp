#include "llvm_ir_builder_context.h"

#include "llvm-c/Core.h"

void builder_context_init(IR_Builder_Context* bc, Ast_Program* program)
{
	bc->program = program;
	bc->module = LLVMModuleCreateWithName("program_module");
	bc->builder = LLVMCreateBuilder();
}

void builder_context_deinit(IR_Builder_Context* bc)
{
	LLVMDisposeBuilder(bc->builder);
}

void builder_context_block_reset(IR_Builder_Context* bc, Value curr_proc)
{
	bc->curr_proc = curr_proc;
	bc->blocks.clear();
	bc->defer_stack.clear();
	bc->loop_stack.clear();
}

void builder_context_block_add(IR_Builder_Context* bc)
{
	bc->blocks.emplace_back(IR_Block_Info { 0, 0, 0 });
}

void builder_context_block_pop_back(IR_Builder_Context* bc)
{
	IR_Block_Info block_info = bc->blocks[bc->blocks.size() - 1];
	for (u32 i = 0; i < block_info.defer_count; i += 1) bc->defer_stack.pop_back();
	for (u32 i = 0; i < block_info.loop_count; i += 1) bc->loop_stack.pop_back();
	for (u32 i = 0; i < block_info.var_count; i += 1) bc->var_stack.pop_back();
	bc->blocks.pop_back();
}

void builder_context_block_add_defer(IR_Builder_Context* bc, Ast_Defer* defer)
{
	bc->blocks[bc->blocks.size() - 1].defer_count += 1;
	bc->defer_stack.emplace_back(defer);
}

void builder_context_block_add_loop(IR_Builder_Context* bc, IR_Loop_Info loop_info)
{
	bc->blocks[bc->blocks.size() - 1].loop_count += 1;
	bc->loop_stack.emplace_back(loop_info);
}

void builder_context_block_add_var(IR_Builder_Context* bc, IR_Var_Info var_info)
{
	bc->blocks[bc->blocks.size() - 1].var_count += 1;
	bc->var_stack.emplace_back(var_info);
}

IR_Var_Info builder_context_block_find_var(IR_Builder_Context* bc, Ast_Ident var_ident)
{
	for (IR_Var_Info& var : bc->var_stack)
	{
		if (match_string_view(var.str, var_ident.str)) return var;
	}
	return {};
}

IR_Loop_Info builder_context_block_get_loop(IR_Builder_Context* bc)
{
	return bc->loop_stack[bc->loop_stack.size() - 1];
}

Basic_Block builder_context_add_bb(IR_Builder_Context* bc, const char* name)
{
	return LLVMAppendBasicBlock(bc->curr_proc, name);
}

Basic_Block builder_context_get_bb(IR_Builder_Context* bc)
{
	return LLVMGetInsertBlock(bc->builder);
}

void builder_context_set_bb(IR_Builder_Context* bc, Basic_Block basic_block)
{
	LLVMPositionBuilderAtEnd(bc->builder, basic_block);
}
