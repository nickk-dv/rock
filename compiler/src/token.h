#pragma once

#include "common.h"

enum TokenType
{
	//[Lexemes]
	TOKEN_IDENT,              // name
	TOKEN_NUMBER,             // 10
	TOKEN_STRING,             // "string"

	//[Keywords]
	TOKEN_KEYWORD_STRUCT,     // struct
	TOKEN_KEYWORD_ENUM,       // enum
	TOKEN_KEYWORD_FN,         // fn
	TOKEN_KEYWORD_IF,         // if
	TOKEN_KEYWORD_ELSE,       // else
	TOKEN_KEYWORD_TRUE,       // true
	TOKEN_KEYWORD_FALSE,      // false
	TOKEN_KEYWORD_FOR,        // for
	TOKEN_KEYWORD_WHILE,      // while
	TOKEN_KEYWORD_BREAK,      // break
	TOKEN_KEYWORD_RETURN,     // return
	TOKEN_KEYWORD_CONTINUE,   // continue

	//[Punctuation]
	TOKEN_DOT,                // .
	TOKEN_COMA,               // ,
	TOKEN_COLON,              // :
	TOKEN_SEMICOLON,          // ;
	TOKEN_BLOCK_START,        // {
	TOKEN_BLOCK_END,          // }
	TOKEN_BRACKET_START,      // [
	TOKEN_BRACKET_END,        // ]
	TOKEN_PARENTHESIS_START,  // (
	TOKEN_PARENTHESIS_END,    // )
	TOKEN_ARROW,              // ->
	TOKEN_DOUBLE_COLON,       // ::

	//[Binary Operators]
	TOKEN_ASSIGN,             // =
	TOKEN_PLUS,               // +
	TOKEN_MINUS,              // -
	TOKEN_TIMES,              // *
	TOKEN_DIV,                // /
	TOKEN_MOD,                // %
	TOKEN_BITWISE_AND,        // &
	TOKEN_BITWISE_OR,         // |
	TOKEN_BITWISE_XOR,        // ^
	TOKEN_PLUS_EQUALS,        // +=
	TOKEN_MINUS_EQUALS,       // -=
	TOKEN_TIMES_EQUALS,       // *=
	TOKEN_DIV_EQUALS,         // /=
	TOKEN_MOD_EQUALS,         // %=
	TOKEN_BITWISE_AND_EQUALS, // &=
	TOKEN_BITWISE_OR_EQUALS,  // |=
	TOKEN_BITWISE_XOR_EQUALS, // ^=
	TOKEN_BITSHIFT_LEFT,      // <<
	TOKEN_BITSHIFT_RIGHT,     // >>
	TOKEN_LESS,               // <
	TOKEN_GREATER,            // >
	TOKEN_LESS_EQUALS,        // <=
	TOKEN_GREATER_EQUALS,     // >=
	TOKEN_IS_EQUAL,           // ==
	TOKEN_NOT_EQUAL,          // !=
	TOKEN_LOGIC_AND,          // &&
	TOKEN_LOGIC_OR,           // ||

	//[Unary Operators]
	TOKEN_BITWISE_NOT,        // ~
	TOKEN_LOGIC_NOT,          // !

	TOKEN_INPUT_END,
	TOKEN_ERROR,
};

struct Token
{
	TokenType type = TOKEN_ERROR;
	u32 l0 = 0;
	u32 c0 = 0;

	union
	{
		double float64_value;
		u64 integer_value;
		StringView string_value;
	};
};

TokenType token_get_keyword_token_type(const StringView& str);
TokenType token_get_1_symbol_token_type(u8 c);
TokenType token_get_2_symbol_token_type(u8 c, u8 c2);
