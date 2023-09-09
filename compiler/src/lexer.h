#pragma once

#include "token.h"

#include <vector>

struct LineInfo
{
	u64 start_cursor = 0;
	u64 end_cursor = 0;
	u32 leading_spaces = 0;
	bool is_valid = true;
	bool is_empty = true;
};

struct Lexer
{
	bool set_input_from_file(const char* file_path);
	std::vector<Token> tokenize();

	LineInfo get_next_line();
	void print_debug_metrics(const std::vector<Token>& tokens);

	String input;
	u64 input_cursor = 0;
};
