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
static void check_block(Ast* ast, Block_Stack* bc, Ast_Block* block);
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

/*
struct Checker;
struct Block_Info;
struct Block_Checker;

struct Checker
{
public:
	bool check_ast(Ast* ast);

private:
	bool check_types_and_proc_definitions(Ast* ast);
	bool is_proc_in_scope(Ast_Ident* proc_ident);
	Ast_Proc_Decl* get_proc_decl(Ast_Ident* proc_ident);
	bool check_struct_decl(Ast_Struct_Decl* struct_decl);
	bool check_enum_decl(Ast_Enum_Decl* enum_decl);
	bool check_proc_decl(Ast_Proc_Decl* proc_decl);
	bool check_proc_block(Ast_Proc_Decl* proc_decl);
	bool check_block(Ast_Block* block, Block_Checker* bc, bool is_entry, bool is_inside_loop);
	bool check_if(Ast_If* _if, Block_Checker* bc);
	bool check_else(Ast_Else* _else, Block_Checker* bc);
	bool check_for(Ast_For* _for, Block_Checker* bc);
	bool check_break(Ast_Break* _break, Block_Checker* bc);
	bool check_return(Ast_Return* _return, Block_Checker* bc);
	bool check_continue(Ast_Continue* _continue, Block_Checker* bc);
	std::optional<Type_Info> check_proc_call(Ast_Proc_Call* proc_call, Block_Checker* bc, bool& is_valid);
	bool check_var_decl(Ast_Var_Decl* var_decl, Block_Checker* bc);
	bool check_var_assign(Ast_Var_Assign* var_assign, Block_Checker* bc);
	std::optional<Type_Info> check_ident_chain(Ast_Ident_Chain* ident_chain, Block_Checker* bc);
	std::optional<Type_Info> check_expr(Ast_Expr* expr, Block_Checker* bc);

	HashMap<StringView, Ast_Proc_Decl*, u32, match_string_view> proc_table;
	Typer typer;
};

struct Block_Info
{
	Ast_Block* block;
	u32 var_count;
	bool is_inside_loop;
};

struct Block_Checker
{
	void block_enter(Ast_Block* block, bool is_inside_loop);
	void block_exit();
	void var_add(const Ast_Ident_Type_Pair& ident_type);
	void var_add(const Ast_Ident& ident, const Ast_Ident& type);
	bool is_var_declared(const Ast_Ident& ident);
	bool is_inside_a_loop();
	Ast_Ident var_get_type(const Ast_Ident& ident);

	std::vector<Block_Info> block_stack;
	std::vector<Ast_Ident_Type_Pair> var_stack; //@Perf this is basic linear search symbol table for the proc block
};
*/
#endif
