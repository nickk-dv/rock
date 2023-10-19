#include "checker_context.h"

void checker_context_init(Checker_Context* cc, Ast* ast, Ast_Program* program, Error_Handler* err)
{
	cc->ast = ast,
	cc->err = err;
	cc->program = program;
}

void checker_context_block_reset(Checker_Context* cc, Ast_Proc_Decl* curr_proc)
{
	cc->curr_proc = curr_proc;
	cc->blocks.clear();
	cc->var_stack.clear();
}

void checker_context_block_add(Checker_Context* cc)
{
	cc->blocks.emplace_back(Block_Info{ 0 });
}

void checker_context_block_pop_back(Checker_Context* cc)
{
	Block_Info block_info = cc->blocks[cc->blocks.size() - 1];
	for (u32 i = 0; i < block_info.var_cout; i += 1) cc->var_stack.pop_back();
	cc->blocks.pop_back();
}

void checker_context_block_add_var(Checker_Context* cc, Ast_Ident ident, Ast_Type type)
{
	cc->blocks[cc->blocks.size() - 1].var_cout += 1;
	cc->var_stack.emplace_back(Var_Info{ ident, type });
}

bool checker_context_block_contains_var(Checker_Context* cc, Ast_Ident ident)
{
	for (Var_Info& var : cc->var_stack)
	{
		if (match_ident(var.ident, ident)) return true;
	}
	return false;
}

option<Ast_Type> checker_context_block_find_var_type(Checker_Context* cc, Ast_Ident ident)
{
	for (Var_Info& var : cc->var_stack)
	{
		if (match_ident(var.ident, ident)) return var.type;
	}
	return {};
}
