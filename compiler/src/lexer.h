#pragma once

#include "common.h"
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

enum LexemeType
{
	LEXEME_IDENT,
	LEXEME_NUMBER,
	LEXEME_STRING,
	LEXEME_SYMBOL,
	LEXEME_ERROR
};

constexpr u32 MAX_LEXER_ERRORS = 100;

enum LexerErrorType
{
	LEXER_ERROR_STRING_NOT_TERMINATED,
	LEXER_ERROR_ILLEGAL_CHARACTER,
};

struct LexerError
{
	LexerErrorType type;
	Token token;
};

struct Lexer
{
	String input;
	u64 input_cursor = 0;
	std::vector<Token> tokens;
	std::vector<LexerError> errors;

	bool set_input_from_file(const char* file_path);
	LineInfo get_next_line();
	LexemeType get_lexeme_type(u8 c);
	void report_error(LexerErrorType error, Token token);
	void tokenize();
};
