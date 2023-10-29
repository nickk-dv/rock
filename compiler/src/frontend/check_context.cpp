#include "check_context.h"

void check_context_init(Check_Context* cc, Ast* ast, Ast_Program* program)
{
	cc->ast = ast,
	cc->program = program;
}

void check_context_block_reset(Check_Context* cc, Ast_Proc_Decl* curr_proc)
{
	cc->curr_proc = curr_proc;
	cc->blocks.clear();
	cc->var_stack.clear();
}

void check_context_block_add(Check_Context* cc)
{
	cc->blocks.emplace_back(Block_Info{ 0 });
}

void check_context_block_pop_back(Check_Context* cc)
{
	Block_Info block_info = cc->blocks[cc->blocks.size() - 1];
	for (u32 i = 0; i < block_info.var_cout; i += 1) cc->var_stack.pop_back();
	cc->blocks.pop_back();
}

void check_context_block_add_var(Check_Context* cc, Ast_Ident ident, Ast_Type type)
{
	cc->blocks[cc->blocks.size() - 1].var_cout += 1;
	cc->var_stack.emplace_back(Var_Info{ ident, type });
}

bool check_context_block_contains_var(Check_Context* cc, Ast_Ident ident)
{
	for (Var_Info& var : cc->var_stack)
	{
		if (match_ident(var.ident, ident)) return true;
	}
	return false;
}

option<Ast_Type> check_context_block_find_var_type(Check_Context* cc, Ast_Ident ident)
{
	for (Var_Info& var : cc->var_stack)
	{
		if (match_ident(var.ident, ident)) return var.type;
	}
	return {};
}
