#ifndef AST_H
#define AST_H

#include "common.h"
#include "token.h"

struct Ast;
struct Ast_Struct_Decl;
struct Ast_Enum_Decl;
struct Ast_Proc_Decl;

struct Ast_Ident { Token token; };
struct Ast_Literal { Token token; };
struct Ast_Ident_Type_Pair;
struct Ast_Ident_Literal_Pair;
struct Ast_Type;
struct Ast_Array_Type;
struct Ast_Var;
struct Ast_Access;
struct Ast_Var_Access;
struct Ast_Array_Access;
struct Ast_Enum;
struct Ast_Term;
struct Ast_Expr;
struct Ast_Unary_Expr;
struct Ast_Binary_Expr;

struct Ast_Block;
struct Ast_Statement;
struct Ast_If;
struct Ast_Else;
struct Ast_For;
struct Ast_Defer;
struct Ast_Break;
struct Ast_Return;
struct Ast_Continue;
struct Ast_Proc_Call;
struct Ast_Var_Decl;
struct Ast_Var_Assign;

struct Ast
{
	std::vector<Ast_Struct_Decl*> structs;
	std::vector<Ast_Enum_Decl*> enums;
	std::vector<Ast_Proc_Decl*> procs;
};

struct Ast_Struct_Decl
{
	Ast_Ident type;
	std::vector<Ast_Ident_Type_Pair> fields;
};

struct Ast_Enum_Decl
{
	Ast_Ident type;
	std::optional<BasicType> basic_type;
	std::vector<Ast_Ident_Literal_Pair> variants;
};

struct Ast_Proc_Decl
{
	Ast_Ident ident;
	std::vector<Ast_Ident_Type_Pair> input_params;
	std::optional<Ast_Type*> return_type;
	Ast_Block* block;
	bool external;
};

struct Ast_Ident_Type_Pair
{
	Ast_Ident ident;
	Ast_Type* type;
};

struct Ast_Ident_Literal_Pair
{
	Ast_Ident ident;
	Ast_Literal literal;
	bool is_negative;
};

struct Ast_Type
{
	enum class Tag
	{
		Basic, Custom, Pointer, Array
	} tag;

	union
	{
		BasicType as_basic;
		Ast_Ident as_custom; //@Perf making this into pointer will reduce it from 40 to 16 bytes
		Ast_Type* as_pointer;
		Ast_Array_Type* as_array;
	};
};

struct Ast_Array_Type
{
	Ast_Type* element_type;
	bool is_dynamic;
	u64 fixed_size;
};

struct Ast_Var
{
	Ast_Ident ident;
	std::optional<Ast_Access*> access;
};

struct Ast_Access
{
	enum class Tag
	{
		Var, Array 
	} tag;

	union
	{
		Ast_Var_Access* as_var;
		Ast_Array_Access* as_array;
	};
};

struct Ast_Var_Access
{
	Ast_Ident ident;
	std::optional<Ast_Access*> next;
};

struct Ast_Array_Access
{
	Ast_Expr* index_expr;
	std::optional<Ast_Access*> next;
};

struct Ast_Enum
{
	Ast_Ident type;
	Ast_Ident variant;
};

struct Ast_Term
{
	enum class Tag
	{
		Var, Enum, Literal, Proc_Call,
	} tag;

	union
	{
		Ast_Var* as_var;
		Ast_Enum* as_enum;
		Ast_Literal as_literal;
		Ast_Proc_Call* as_proc_call;
	};
};

struct Ast_Expr
{
	enum class Tag
	{
		Term, Unary_Expr, Binary_Expr,
	} tag;

	union
	{
		Ast_Term* as_term;
		Ast_Unary_Expr* as_unary_expr;
		Ast_Binary_Expr* as_binary_expr;
	};
};

struct Ast_Unary_Expr
{
	UnaryOp op;
	Ast_Expr* right;
};

struct Ast_Binary_Expr
{
	BinaryOp op;
	Ast_Expr* left;
	Ast_Expr* right;
};

struct Ast_Block
{
	std::vector<Ast_Statement*> statements;
};

struct Ast_Statement
{
	enum class Tag
	{
		If, For, Defer, Break, Return, 
		Continue, Proc_Call, Var_Decl, Var_Assign,
	} tag;

	union
	{
		Ast_If* as_if;
		Ast_For* as_for;
		Ast_Defer* as_defer;
		Ast_Break* as_break;
		Ast_Return* as_return;
		Ast_Continue* as_continue;
		Ast_Proc_Call* as_proc_call;
		Ast_Var_Decl* as_var_decl;
		Ast_Var_Assign* as_var_assign;
	};
};

struct Ast_If
{
	Token token;
	Ast_Expr* condition_expr;
	Ast_Block* block;
	std::optional<Ast_Else*> _else;
};

struct Ast_Else
{
	Token token;

	enum class Tag
	{
		If, Block,
	} tag;

	union
	{
		Ast_If* as_if;
		Ast_Block* as_block;
	};
};

struct Ast_For
{
	Token token;
	std::optional<Ast_Var_Decl*> var_decl;
	std::optional<Ast_Expr*> condition_expr;
	std::optional<Ast_Var_Assign*> var_assign;
	Ast_Block* block;
};

struct Ast_Defer
{
	Token token;
	Ast_Block* block;
};

struct Ast_Break
{
	Token token;
};

struct Ast_Return
{
	Token token;
	std::optional<Ast_Expr*> expr;
};

struct Ast_Continue
{
	Token token;
};

struct Ast_Proc_Call
{
	Ast_Ident ident;
	std::vector<Ast_Expr*> input_exprs;
	std::optional<Ast_Access*> access;
};

struct Ast_Var_Decl
{
	Ast_Ident ident;
	std::optional<Ast_Type*> type;
	std::optional<Ast_Expr*> expr;
};

struct Ast_Var_Assign
{
	Ast_Var* var;
	AssignOp op;
	Ast_Expr* expr;
};

#endif
