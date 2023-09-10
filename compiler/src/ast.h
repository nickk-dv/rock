#pragma once

#include "token.h"

#include <vector>
#include <optional>

struct Ast_Literal;
struct Ast_Identifier;
struct Ast_Term;
struct Ast_Expression;
struct Ast_Binary_Expression;

struct Ast;
struct Ast_Struct_Declaration;
struct Ast_Enum_Declaration;
struct Ast_Procedure_Declaration;

struct Ast_Block;
struct Ast_Statement;
struct Ast_If;
struct Ast_For;
struct Ast_While;
struct Ast_Break;
struct Ast_Return;
struct Ast_Continue;
struct Ast_Procedure_Call;
struct Ast_Variable_Assignment;
struct Ast_Variable_Declaration;

struct Ast_Literal
{
	Token token;
};

struct Ast_Identifier
{
	Token token;
};

struct Ast_Term
{
	enum class Tag
	{
		Literal, Identifier, ProcedureCall,
	} tag;

	union
	{
		Ast_Literal _literal;
		Ast_Identifier _ident;
		Ast_Procedure_Call* _proc_call;
	};
};

struct Ast_Expression
{
	enum class Tag
	{
		Term, BinaryExpression,
	} tag;

	union
	{
		Ast_Term* _term;
		Ast_Binary_Expression* _bin_expr;
	};
};

enum BinaryOp
{
	//@Incomplete find mapping from tokens
};

struct Ast_Binary_Expression
{
	BinaryOp op;
	Ast_Expression* left;
	Ast_Expression* right;
};

struct Ast
{
	std::vector<Ast_Struct_Declaration> structs;
	std::vector<Ast_Enum_Declaration> enums;
	std::vector<Ast_Procedure_Declaration> procedures;
};

struct IdentTypePair
{
	Ast_Identifier ident;
	Ast_Identifier type;
};

struct Ast_Struct_Declaration
{
	Ast_Identifier type;
	std::vector<IdentTypePair> fields;
};

struct Ast_Enum_Declaration
{
	Ast_Identifier type;
	std::vector<IdentTypePair> variants;
};

struct Ast_Procedure_Declaration
{
	Ast_Identifier ident;
	std::vector<IdentTypePair> input_parameters;
	std::optional<Ast_Identifier> return_type;
	Ast_Block* block;
};

struct Ast_Block
{
	std::vector<Ast_Statement*> statements;
};

struct Ast_Statement
{
	enum class Tag
	{
		If, For, While, Break, Return, Continue,
		ProcedureCall, VariableAssignment, VariableDeclaration,
	} tag;

	union
	{
		Ast_If* _if;
		Ast_For* _for;
		Ast_While* _while;
		Ast_Break* _break;
		Ast_Return* _return;
		Ast_Continue* _continue;
		Ast_Procedure_Call* _proc_call;
		Ast_Variable_Assignment* _var_assignment;
		Ast_Variable_Declaration* _var_declaration;
	};
};

struct Ast_If //@Incomplete
{
	//Some conditional expr
	Ast_Block* block;
	std::optional<Ast_Block*> else_block;
};

struct Ast_For //@Incomplete
{
	//3 expressions
	Ast_Block* block;
};

struct Ast_While //@Incomplete
{
	//Some conditional expr
	Ast_Block* block;
};

struct Ast_Break
{
	Token token;
};

struct Ast_Return
{
	Token token;
	std::optional<Ast_Expression*> expr;
};

struct Ast_Continue
{
	Token token;
};

struct Ast_Procedure_Call
{
	//@Incomplete
};

struct Ast_Variable_Assignment
{
	Ast_Identifier ident;
	Ast_Expression* expr;
};

struct Ast_Variable_Declaration
{
	Ast_Identifier ident;
	Ast_Identifier type;
	std::optional<Ast_Expression*> expr;
};
