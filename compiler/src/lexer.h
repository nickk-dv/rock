#pragma once

enum TokenType
{
	TOKEN_BITWISE_XOR = '^',
	TOKEN_BITWISE_NOT = '~',
	TOKEN_BITWISE_AND = '&',
	TOKEN_BITWISE_OR  = '|',

	//ASCII types here 
	//@TODO maybe add named tokens for used characters like .,{}[]...

	TOKEN_IDENT = 256,      // name
	TOKEN_NUMBER,           // 10
	TOKEN_STRING,           // "string"

	TOKEN_PLUS_EQUALS,      // +=
	TOKEN_MINUS_EQUALS,     // -=
	TOKEN_TIMES_EQUALS,     // *=
	TOKEN_DIV_EQUALS,       // /=

	TOKEN_IS_EQUAL,         // ==
	TOKEN_NOT_EQUAL,        // !=
	TOKEN_LOGIC_AND,        // &&
	TOKEN_LOGIC_OR,         // ||

	TOKEN_LESS_EQUALS,      // <=
	TOKEN_GREATER_EQUALS,   // >=
	TOKEN_SHIFT_LEFT,       // <<
	TOKEN_SHIFT_RIGHT,      // >>

	TOKEN_DOUBLE_COLON,     // ::

	TOKEN_KEYWORD_IF,       // if
	TOKEN_KEYWORD_ELSE,     // else
	TOKEN_KEYWORD_FOR,      // for
	TOKEN_KEYWORD_WHILE,    // while
	TOKEN_KEYWORD_SWITCH,   // switch
	TOKEN_KEYWORD_CASE,     // case
	TOKEN_KEYWORD_BREAK,    // break
	TOKEN_KEYWORD_RETURN,   // return
	TOKEN_KEYWORD_CONTINUE, // continue

	TOKEN_KEYWORD_TRUE,     // true
	TOKEN_KEYWORD_FALSE,    // false

	TOKEN_KEYWORD_STRUCT,   // struct
	TOKEN_KEYWORD_ENUM,     // enum

	TOKEN_EOF,              // end of file

	TOKEN_ERROR,
};
