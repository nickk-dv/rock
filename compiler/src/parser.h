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
	std::optional<Token> peek(u32 offset = 0);
	void seek(size_t index);
	void consume();
	void exit_error();

	const std::vector<Token> m_tokens;
	size_t m_index = 0;
	ArenaAllocator m_arena;
};
