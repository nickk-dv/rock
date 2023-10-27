#ifndef CHECKER_CONTEXT_H
#define CHECKER_CONTEXT_H

#include "ast.h"
#include "general/error_handler.h"

struct Checker_Context;
struct Block_Info;
struct Var_Info;
struct Type_Context;
struct Literal;
enum class Type_Kind;
enum class Literal_Kind;
enum class Terminator;
enum class Checker_Block_Flags;
enum class Checker_Proc_Call_Flags;

void checker_context_init(Checker_Context* cc, Ast* ast, Ast_Program* program, Error_Handler* err);
void checker_context_block_reset(Checker_Context* cc, Ast_Proc_Decl* curr_proc);
void checker_context_block_add(Checker_Context* cc);
void checker_context_block_pop_back(Checker_Context* cc);
void checker_context_block_add_var(Checker_Context* cc, Ast_Ident ident, Ast_Type type);
bool checker_context_block_contains_var(Checker_Context* cc, Ast_Ident ident);
option<Ast_Type> checker_context_block_find_var_type(Checker_Context* cc, Ast_Ident ident);

struct Checker_Context
{
	Ast* ast;
	Ast_Program* program;
	Error_Handler* err;
	Ast_Proc_Decl* curr_proc;
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

struct Type_Context
{
	option<Ast_Type> expect_type;
	bool expect_constant;
};

Type_Context type_context_make(option<Ast_Type> expect_type, bool expect_constant);

struct Literal
{
	Literal_Kind kind;

	union
	{
		bool as_bool;
		f64 as_f64;
		i64 as_i64;
		u64 as_u64;
	};
};

enum class Type_Kind
{
	Bool,
	Float,
	Integer,
	String,
	Pointer,
	Array,
	Struct,
	Enum,
};

enum class Literal_Kind
{
	Bool, 
	Float, 
	Int, 
	UInt
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
