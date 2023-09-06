#pragma once

#include "common.h"

enum TokenType
{
	TOKEN_ASSIGN              = '=',
	TOKEN_DOT                 = '.',
	TOKEN_COMA                = ',',
	TOKEN_COLON               = ':',
	TOKEN_SEMICOLON           = ';',

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
	TOKEN_KEYWORD_BREAK,      // break
	TOKEN_KEYWORD_RETURN,     // return
	TOKEN_KEYWORD_CONTINUE,   // continue

	TOKEN_KEYWORD_TRUE,       // true
	TOKEN_KEYWORD_FALSE,      // false

	TOKEN_KEYWORD_STRUCT,     // struct
	TOKEN_KEYWORD_ENUM,       // enum
	TOKEN_KEYWORD_FN,         // fn

	TOKEN_INPUT_END,          // input end

	TOKEN_ERROR,
};

struct Token //@Performance 32bytes is too much on large 1m line + files
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

TokenType token_get_keyword_token_type(const StringView& str);
TokenType token_get_1_symbol_token_type(u8 c);
TokenType token_get_2_symbol_token_type(u8 c, u8 c2);
void token_debug_print(const Token& token);
