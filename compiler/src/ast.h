#pragma once

#include "token.h"

#include <vector>
#include <optional>

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

struct Ast_Literal;
struct Ast_Unary_Expression;
struct Ast_Binary_Expression;

struct Ast
{
	std::vector<Ast_Struct_Declaration> structs;
	std::vector<Ast_Enum_Declaration> enums;
	std::vector<Ast_Procedure_Declaration> procedures;
};

struct IdentTypePair
{
	Token ident;
	Token type;
};

struct Ast_Struct_Declaration
{
	Token type;
	std::vector<IdentTypePair> fields;
};

struct Ast_Enum_Declaration
{
	Token type;
	std::vector<IdentTypePair> variants;
};

struct Ast_Procedure_Declaration
{
	Token ident;
	std::vector<IdentTypePair> input_parameters;
	std::optional<Token> return_type;
	Ast_Block* block;
};

struct Ast_Block
{
	std::vector<Ast_Statement*> statements;
};

enum class BlockStatement
{
	If, For, While, Break, Return, Continue,
	ProcedureCall, VariableAssignment, VariableDeclaration,
};

struct Ast_Statement
{
	enum class Tag
	{
		If, For, While, Break, Return, Continue,
		ProcedureCall, VariableAssignment, VariableDeclaration,
	};

	Tag tag;

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

struct Ast_If
{
	//@Incomplete
};

struct Ast_For
{
	//@Incomplete
};

struct Ast_While
{
	//@Incomplete
};

struct Ast_Break
{
	Token token;
};

struct Ast_Return
{
	//@Incomplete
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
	//@Incomplete
};

struct Ast_Variable_Declaration
{
	//@Incomplete
};

struct Ast_Literal
{
	Token token;
};

struct Ast_Unary_Expression
{
	//@Incomplete
};

struct Ast_Binary_Expression
{
	//@Incomplete
};
