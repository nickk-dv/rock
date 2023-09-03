#pragma once

#include "common.h"

// !#$%&'()*+,-./:;<=>?@[\]^_`{|}~  all not numeric & letter characters
// #$&':?@\_` unused characters

enum TokenType
{
	TOKEN_ASSIGN              = '=',
	TOKEN_SEMICOLON           = ';',
	TOKEN_DOT                 = '.',
	TOKEN_COMA                = ',',

	TOKEN_SCOPE_START         = '{',
	TOKEN_SCOPE_END           = '}',
	TOKEN_BRACKET_START       = '[',
	TOKEN_BRACKET_END         = ']',
	TOKEN_PARENTHESIS_START   = '(',
	TOKEN_PARENTHESIS_END     = ')',

	TOKEN_PLUS                = '+',
	TOKEN_MINUS               = '-',
	TOKEN_TIMES               = '*',
	TOKEN_DIV                 = '/',
	TOKEN_MOD                 = '%',
	TOKEN_BITWISE_AND         = '&',
	TOKEN_BITWISE_OR          = '|',
	TOKEN_BITWISE_XOR         = '^',
	TOKEN_BITWISE_NOT         = '~',

	TOKEN_LOGIC_NOT           = '!',
	TOKEN_LESS                = '<',
	TOKEN_GREATER             = '>',

	TOKEN_IDENT = 128,        // name
	TOKEN_NUMBER,             // 10
	TOKEN_STRING,             // "string"

	TOKEN_PLUS_EQUALS,        // +=
	TOKEN_MINUS_EQUALS,       // -=
	TOKEN_TIMES_EQUALS,       // *=
	TOKEN_DIV_EQUALS,         // /=
	TOKEN_MOD_EQUALS,         // %=
	TOKEN_BITWISE_AND_EQUALS, // &=
	TOKEN_BITWISE_OR_EQUALS,  // |=
	TOKEN_BITWISE_XOR_EQUALS, // ^=

	TOKEN_IS_EQUAL,           // ==
	TOKEN_NOT_EQUAL,          // !=
	TOKEN_LOGIC_AND,          // &&
	TOKEN_LOGIC_OR,           // ||
	TOKEN_LESS_EQUALS,        // <=
	TOKEN_GREATER_EQUALS,     // >=

	TOKEN_SHIFT_LEFT,         // <<
	TOKEN_SHIFT_RIGHT,        // >>
	TOKEN_DOUBLE_COLON,       // ::

	TOKEN_KEYWORD_IF,         // if
	TOKEN_KEYWORD_ELSE,       // else
	TOKEN_KEYWORD_FOR,        // for
	TOKEN_KEYWORD_WHILE,      // while
	TOKEN_KEYWORD_SWITCH,     // switch
	TOKEN_KEYWORD_CASE,       // case
	TOKEN_KEYWORD_BREAK,      // break
	TOKEN_KEYWORD_RETURN,     // return
	TOKEN_KEYWORD_CONTINUE,   // continue

	TOKEN_KEYWORD_TRUE,       // true
	TOKEN_KEYWORD_FALSE,      // false

	TOKEN_KEYWORD_STRUCT,     // struct
	TOKEN_KEYWORD_ENUM,       // enum

	TOKEN_EOF,                // end of file
	
	TOKEN_ERROR,
};

enum LexemeType
{
	LEXEME_IDENT,
	LEXEME_NUMBER,
	LEXEME_STRING,
	LEXEME_SYMBOL,
	
	LEXEME_ERROR,
};

struct Token
{
	TokenType type = TOKEN_ERROR;
	u32 l0 = 0;
	u32 c0 = 0;

	union 
	{
		float float32_value;
		double float64_value;
		u64 integer_value;
		StringView string_value;
	};
};

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
	String input;
	u64 input_cursor = 0;
	u32 current_line_number = 0;
	u32 current_char_number = 0;

	bool set_input_from_file(const char* file_path);
	LineInfo get_next_line();
	void tokenize();
};
