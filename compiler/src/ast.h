#pragma once

#include <vector>

struct Ast;

struct Ast_Block;
struct Ast_Block_Statement;

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

struct Ast_Block
{
	std::vector<Ast_Block_Statement*> statements;
};

enum class BlockStatement
{
	If, For, While, Break, Return, Continue,
	ProcedureCall, VariableAssignment, VariableDeclaration,
};

struct Ast_Block_Statement
{
	BlockStatement tag;
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
	} statement;
};

struct Ast_If
{

};

struct Ast_For
{

};

struct Ast_While
{

};

struct Ast_Break
{

};

struct Ast_Return
{

};

struct Ast_Continue
{

};

struct Ast_Procedure_Call
{

};

struct Ast_Variable_Assignment
{

};

struct Ast_Variable_Declaration
{

};

struct Ast_Literal
{

};

struct Ast_Unary_Expression
{

};

struct Ast_Binary_Expression
{

};
