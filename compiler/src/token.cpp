#include "token.h"

#include <unordered_map>

const std::unordered_map<u64, TokenType> keyword_hash_to_token_type =
{
	{ string_hash_ascii_count_9("if"),       TOKEN_KEYWORD_IF },
	{ string_hash_ascii_count_9("else"),     TOKEN_KEYWORD_ELSE },
	{ string_hash_ascii_count_9("for"),      TOKEN_KEYWORD_FOR },
	{ string_hash_ascii_count_9("while"),    TOKEN_KEYWORD_WHILE },
	{ string_hash_ascii_count_9("break"),    TOKEN_KEYWORD_BREAK },
	{ string_hash_ascii_count_9("return"),   TOKEN_KEYWORD_RETURN },
	{ string_hash_ascii_count_9("continue"), TOKEN_KEYWORD_CONTINUE },

	{ string_hash_ascii_count_9("true"),     TOKEN_KEYWORD_TRUE },
	{ string_hash_ascii_count_9("false"),    TOKEN_KEYWORD_FALSE },

	{ string_hash_ascii_count_9("struct"),   TOKEN_KEYWORD_STRUCT },
	{ string_hash_ascii_count_9("enum"),     TOKEN_KEYWORD_ENUM},
	{ string_hash_ascii_count_9("fn"),       TOKEN_KEYWORD_FN},
};

const std::unordered_map<u64, TokenType> symbol_hash_to_token_type =
{
	{ string_hash_ascii_count_9("="), TOKEN_ASSIGN },
	{ string_hash_ascii_count_9("."), TOKEN_DOT },
	{ string_hash_ascii_count_9(","), TOKEN_COMA },
	{ string_hash_ascii_count_9(":"), TOKEN_COLON },
	{ string_hash_ascii_count_9(";"), TOKEN_SEMICOLON },

	{ string_hash_ascii_count_9("{"), TOKEN_SCOPE_START },
	{ string_hash_ascii_count_9("}"), TOKEN_SCOPE_END },
	{ string_hash_ascii_count_9("["), TOKEN_BRACKET_START },
	{ string_hash_ascii_count_9("]"), TOKEN_BRACKET_END },
	{ string_hash_ascii_count_9("("), TOKEN_PARENTHESIS_START },
	{ string_hash_ascii_count_9(")"), TOKEN_PARENTHESIS_END },

	{ string_hash_ascii_count_9("+"), TOKEN_PLUS },
	{ string_hash_ascii_count_9("-"), TOKEN_MINUS },
	{ string_hash_ascii_count_9("*"), TOKEN_TIMES },
	{ string_hash_ascii_count_9("/"), TOKEN_DIV },
	{ string_hash_ascii_count_9("%"), TOKEN_MOD },
	{ string_hash_ascii_count_9("&"), TOKEN_BITWISE_AND },
	{ string_hash_ascii_count_9("|"), TOKEN_BITWISE_OR },
	{ string_hash_ascii_count_9("^"), TOKEN_BITWISE_XOR },
	{ string_hash_ascii_count_9("~"), TOKEN_BITWISE_NOT },

	{ string_hash_ascii_count_9("!"), TOKEN_LOGIC_NOT },
	{ string_hash_ascii_count_9("<"), TOKEN_LESS },
	{ string_hash_ascii_count_9(">"), TOKEN_GREATER },

	{ string_hash_ascii_count_9("+="), TOKEN_PLUS_EQUALS },
	{ string_hash_ascii_count_9("-="), TOKEN_MINUS_EQUALS },
	{ string_hash_ascii_count_9("*="), TOKEN_TIMES_EQUALS },
	{ string_hash_ascii_count_9("/="), TOKEN_DIV_EQUALS },
	{ string_hash_ascii_count_9("%="), TOKEN_MOD_EQUALS },
	{ string_hash_ascii_count_9("&="), TOKEN_BITWISE_AND_EQUALS },
	{ string_hash_ascii_count_9("|="), TOKEN_BITWISE_OR_EQUALS },
	{ string_hash_ascii_count_9("^="), TOKEN_BITWISE_XOR_EQUALS },

	{ string_hash_ascii_count_9("=="), TOKEN_IS_EQUAL },
	{ string_hash_ascii_count_9("!="), TOKEN_NOT_EQUAL },
	{ string_hash_ascii_count_9("&&"), TOKEN_LOGIC_AND },
	{ string_hash_ascii_count_9("||"), TOKEN_LOGIC_OR },
	{ string_hash_ascii_count_9("<="), TOKEN_LESS_EQUALS },
	{ string_hash_ascii_count_9(">="), TOKEN_GREATER_EQUALS },

	{ string_hash_ascii_count_9("<<"), TOKEN_SHIFT_LEFT },
	{ string_hash_ascii_count_9(">>"), TOKEN_SHIFT_RIGHT },
	{ string_hash_ascii_count_9("::"), TOKEN_DOUBLE_COLON },
};

TokenType token_get_keyword_token_type(const StringView& str)
{
	if (str.count > 9) return TOKEN_ERROR;
	u64 hash = string_hash_ascii_count_9(str);
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
