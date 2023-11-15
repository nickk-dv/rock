#ifndef CHECK_CONTEXT_H
#define CHECK_CONTEXT_H

#include "ast.h"

struct Check_Context;
struct Block_Info;
struct Var_Info;
enum class Terminator;
enum class Checker_Block_Flags;
enum class Checker_Proc_Call_Flags;

void check_context_init(Check_Context* cc, Ast* ast, Ast_Program* program);
void check_context_block_reset(Check_Context* cc, Ast_Decl_Proc* curr_proc);
void check_context_block_add(Check_Context* cc);
void check_context_block_pop_back(Check_Context* cc);
void check_context_block_add_var(Check_Context* cc, Ast_Ident ident, Ast_Type type);
bool check_context_block_contains_var(Check_Context* cc, Ast_Ident ident);
option<Ast_Type> check_context_block_find_var_type(Check_Context* cc, Ast_Ident ident);

struct Check_Context
{
	Ast* ast;
	Ast_Program* program;
	Ast_Decl_Proc* curr_proc;
	std::vector<Block_Info> blocks;
	std::vector<Var_Info> var_stack; //@Perf try hashmap with reset() on larger files
};

struct Block_Info
{
	u32 var_cout;
};

struct Var_Info
{
	Ast_Ident ident;
	Ast_Type type;
};

enum class Terminator
{
	None,
	Break,
	Return,
	Continue,
};

enum class Checker_Block_Flags
{
	None,
	Already_Added,
};

enum class Checker_Proc_Call_Flags
{
	In_Expr,
	In_Statement,
};

#endif
