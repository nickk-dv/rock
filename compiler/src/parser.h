#pragma once

#include "ast.h"
#include "token.h"

#include <vector>

struct Parser
{
	Parser(std::vector<Token> tokens);

	Ast parse();

	std::optional<Ast_Struct_Declaration> parse_struct();
	std::optional<Ast_Enum_Declaration> parse_enum();
	std::optional<Ast_Procedure_Declaration> parse_procedure();

	Ast_Block* parse_block();
	Ast_Statement* parse_statement();
	Ast_If* parse_if();
	Ast_For* parse_for();
	Ast_While* parse_while();
	Ast_Break* parse_break();
	Ast_Return* parse_return();
	Ast_Continue* parse_continue();
	Ast_Procedure_Call* parse_proc_call();
	Ast_Variable_Assignment* parse_var_assignment();
	Ast_Variable_Declaration* parse_var_declaration();

	std::optional<Token> peek(u32 offset = 0);
	std::optional<Token> try_consume(TokenType token_type);
	void consume();

	const std::vector<Token> m_tokens;
	size_t m_index = 0;
	ArenaAllocator m_arena;
};
