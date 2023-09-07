#include "token.h"

#include "common.h"

#include <unordered_map>

//@Performance experiment with unified hash map for keywords and symbols
const std::unordered_map<u64, TokenType> keyword_hash_to_token_type =
{
	{ hash_ascii_9("if"),       TOKEN_KEYWORD_IF },
	{ hash_ascii_9("else"),     TOKEN_KEYWORD_ELSE },
	{ hash_ascii_9("for"),      TOKEN_KEYWORD_FOR },
	{ hash_ascii_9("while"),    TOKEN_KEYWORD_WHILE },
	{ hash_ascii_9("break"),    TOKEN_KEYWORD_BREAK },
	{ hash_ascii_9("return"),   TOKEN_KEYWORD_RETURN },
	{ hash_ascii_9("continue"), TOKEN_KEYWORD_CONTINUE },

	{ hash_ascii_9("true"),     TOKEN_KEYWORD_TRUE },
	{ hash_ascii_9("false"),    TOKEN_KEYWORD_FALSE },

	{ hash_ascii_9("struct"),   TOKEN_KEYWORD_STRUCT },
	{ hash_ascii_9("enum"),     TOKEN_KEYWORD_ENUM},
	{ hash_ascii_9("fn"),       TOKEN_KEYWORD_FN},
};

const std::unordered_map<u64, TokenType> symbol_hash_to_token_type =
{
	{ hash_ascii_9("="), TOKEN_ASSIGN },
	{ hash_ascii_9("."), TOKEN_DOT },
	{ hash_ascii_9(","), TOKEN_COMA },
	{ hash_ascii_9(":"), TOKEN_COLON },
	{ hash_ascii_9(";"), TOKEN_SEMICOLON },

	{ hash_ascii_9("{"), TOKEN_SCOPE_START },
	{ hash_ascii_9("}"), TOKEN_SCOPE_END },
	{ hash_ascii_9("["), TOKEN_BRACKET_START },
	{ hash_ascii_9("]"), TOKEN_BRACKET_END },
	{ hash_ascii_9("("), TOKEN_PARENTHESIS_START },
	{ hash_ascii_9(")"), TOKEN_PARENTHESIS_END },

	{ hash_ascii_9("+"), TOKEN_PLUS },
	{ hash_ascii_9("-"), TOKEN_MINUS },
	{ hash_ascii_9("*"), TOKEN_TIMES },
	{ hash_ascii_9("/"), TOKEN_DIV },
	{ hash_ascii_9("%"), TOKEN_MOD },
	{ hash_ascii_9("&"), TOKEN_BITWISE_AND },
	{ hash_ascii_9("|"), TOKEN_BITWISE_OR },
	{ hash_ascii_9("^"), TOKEN_BITWISE_XOR },
	{ hash_ascii_9("~"), TOKEN_BITWISE_NOT },

	{ hash_ascii_9("!"), TOKEN_LOGIC_NOT },
	{ hash_ascii_9("<"), TOKEN_LESS },
	{ hash_ascii_9(">"), TOKEN_GREATER },

	{ hash_ascii_9("+="), TOKEN_PLUS_EQUALS },
	{ hash_ascii_9("-="), TOKEN_MINUS_EQUALS },
	{ hash_ascii_9("*="), TOKEN_TIMES_EQUALS },
	{ hash_ascii_9("/="), TOKEN_DIV_EQUALS },
	{ hash_ascii_9("%="), TOKEN_MOD_EQUALS },
	{ hash_ascii_9("&="), TOKEN_BITWISE_AND_EQUALS },
	{ hash_ascii_9("|="), TOKEN_BITWISE_OR_EQUALS },
	{ hash_ascii_9("^="), TOKEN_BITWISE_XOR_EQUALS },

	{ hash_ascii_9("=="), TOKEN_IS_EQUAL },
	{ hash_ascii_9("!="), TOKEN_NOT_EQUAL },
	{ hash_ascii_9("&&"), TOKEN_LOGIC_AND },
	{ hash_ascii_9("||"), TOKEN_LOGIC_OR },
	{ hash_ascii_9("<="), TOKEN_LESS_EQUALS },
	{ hash_ascii_9(">="), TOKEN_GREATER_EQUALS },

	{ hash_ascii_9("<<"), TOKEN_SHIFT_LEFT },
	{ hash_ascii_9(">>"), TOKEN_SHIFT_RIGHT },
	{ hash_ascii_9("::"), TOKEN_DOUBLE_COLON },
	{ hash_ascii_9("->"), TOKEN_ARROW },
};

TokenType token_get_keyword_token_type(const StringView& str)
{
	if (str.count > 9) return TOKEN_ERROR;
	u64 hash = string_hash_ascii_9(str);
	bool is_keyword = keyword_hash_to_token_type.find(hash) != keyword_hash_to_token_type.end();
	return is_keyword ? keyword_hash_to_token_type.at(hash) : TOKEN_ERROR;
}

TokenType token_get_1_symbol_token_type(u8 c)
{
	u64 hash = (u64)c;
	bool is_symbol = symbol_hash_to_token_type.find(hash) != symbol_hash_to_token_type.end();
	return is_symbol ? symbol_hash_to_token_type.at(hash) : TOKEN_ERROR;
}

TokenType token_get_2_symbol_token_type(u8 c, u8 c2)
{
	u64 hash = ((u64)c << 7) | (u64)c2;
	bool is_symbol = symbol_hash_to_token_type.find(hash) != symbol_hash_to_token_type.end();
	return is_symbol ? symbol_hash_to_token_type.at(hash) : TOKEN_ERROR;
}
