#pragma once

#include "lexer.h"

struct Parser
{
	Parser(std::vector<Token> tokens);

	void parse();

	const std::vector<Token> m_tokens;
	size_t m_index = 0;
	ArenaAllocator m_arena;
};
