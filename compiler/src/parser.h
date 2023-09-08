#pragma once

#include "token.h"

#include <vector>
#include <optional>

struct Parser
{
	Parser(std::vector<Token> tokens);

	void parse();
	void parse_struct();
	void parse_enum();
	void parse_fn();
	std::optional<Token> peek(u32 offset = 0);
	void seek(size_t index);
	void consume();
	void exit_error();

	const std::vector<Token> m_tokens;
	size_t m_index = 0;
	ArenaAllocator m_arena;
};
