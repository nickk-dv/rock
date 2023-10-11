#ifndef CHECKER_H
#define CHECKER_H

#include "common.h"
//#include "typer.h"
#include "ast.h"

typedef std::unordered_map<std::string, Ast*> Module_Map;
enum class Terminator;
struct Block_Stack;

bool check_declarations(Ast* ast, Ast_Program* program, Module_Map& modules);
bool check_ast(Ast* ast, Ast_Program* program);

static Ast* try_import(Ast* ast, std::optional<Ast_Ident> import);
static Terminator check_block_cfg(Ast_Block* block, bool is_loop, bool is_defer, bool is_entry = false);
static void check_if_cfg(Ast_If* _if, bool is_loop, bool is_defer);
static void check_block(Ast* ast, Block_Stack* bc, Ast_Block* block, bool add_block = true);
static void check_if(Ast* ast, Block_Stack* bc, Ast_If* _if);
static void check_for(Ast* ast, Block_Stack* bc, Ast_For* _for);
static void check_proc_call(Ast* ast, Block_Stack* bc, Ast_Proc_Call* proc_call);
static void check_var_decl(Ast* ast, Block_Stack* bc, Ast_Var_Decl* var_decl);
static void check_var_assign(Ast* ast, Block_Stack* bc, Ast_Var_Assign* var_assign);
static void block_stack_reset(Block_Stack* bc);
static void block_stack_add_block(Block_Stack* bc);
static void block_stack_remove_block(Block_Stack* bc);
static void block_stack_add_var(Block_Stack* bc, Ast_Ident ident);
static bool block_stack_contains_var(Block_Stack* bc, Ast_Ident ident);
static void error_pair(const char* message, const char* labelA, Ast_Ident identA, const char* labelB, Ast_Ident identB);
static void error(const char* message, Ast_Ident ident);

enum class Terminator
{
	None,
	Break,
	Return,
	Continue,
};

struct Block_Stack
{
	u32 block_count = 0;
	std::vector<u32> var_count_stack;
	std::vector<Ast_Ident> var_stack;
};

#endif
