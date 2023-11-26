export module check_context;

import general;
import ast;

struct Check_Context;
struct Block_Info;
struct Var_Info;
enum class Terminator;
enum class Checker_Block_Flags;
enum class Checker_Proc_Call_Flags;

export void check_context_init(Check_Context* cc, Ast* ast, Ast_Program* program);
export void check_context_block_reset(Check_Context* cc, Ast_Decl_Proc* curr_proc);
export void check_context_block_add(Check_Context* cc);
export void check_context_block_pop_back(Check_Context* cc);
export void check_context_block_add_var(Check_Context* cc, Ast_Ident ident, Ast_Type type);
export bool check_context_block_contains_var(Check_Context* cc, Ast_Ident ident);
export option<Ast_Type> check_context_block_find_var_type(Check_Context* cc, Ast_Ident ident);

export struct Check_Context
{
	Ast* ast;
	Ast_Program* program;
	Ast_Decl_Proc* curr_proc;
	std::vector<Block_Info> blocks;
	std::vector<Var_Info> var_stack; //@Perf try hashmap with reset() on larger files
};

export struct Block_Info
{
	u32 var_cout;
};

export struct Var_Info
{
	Ast_Ident ident;
	Ast_Type type;
};

export enum class Terminator
{
	None,
	Break,
	Return,
	Continue,
};

export enum class Checker_Block_Flags
{
	None,
	Already_Added,
};

export enum class Checker_Proc_Call_Flags
{
	In_Expr,
	In_Statement,
};

module : private;

void check_context_init(Check_Context* cc, Ast* ast, Ast_Program* program)
{
	cc->ast = ast,
	cc->program = program;
}

void check_context_block_reset(Check_Context* cc, Ast_Decl_Proc* curr_proc)
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
		if (var.ident == ident) return true;
	}
	return false;
}

option<Ast_Type> check_context_block_find_var_type(Check_Context* cc, Ast_Ident ident)
{
	for (Var_Info& var : cc->var_stack)
	{
		if (var.ident == ident) return var.type;
	}
	return {};
}
