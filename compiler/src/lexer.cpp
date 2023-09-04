#include "lexer.h"

#include "common.h"

#include <iostream> //@Debug for debug printing only
#include <unordered_map>

//@Sync with keyword tokens
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
};

//@Sync with symbol tokens
const std::unordered_map<u64, TokenType> symbol_hash_to_token_type =
{
	{ string_hash_ascii_count_9("="), TOKEN_ASSIGN },
	{ string_hash_ascii_count_9(";"), TOKEN_SEMICOLON },
	{ string_hash_ascii_count_9("."), TOKEN_DOT },
	{ string_hash_ascii_count_9(","), TOKEN_COMA },
	{ string_hash_ascii_count_9(":"), TOKEN_COLON },

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

TokenType get_keyword_token_type(const StringView& str)
{
	if (str.count > 9) return TOKEN_ERROR;
	u64 hash = string_hash_ascii_count_9(str);
	bool is_keyword = keyword_hash_to_token_type.find(hash) != keyword_hash_to_token_type.end();
	return is_keyword ? keyword_hash_to_token_type.at(hash) : TOKEN_ERROR;
}

TokenType get_single_symbol_token_type(u8 c)
{
	u64 hash = (u64)c;
	bool is_symbol = symbol_hash_to_token_type.find(hash) != symbol_hash_to_token_type.end();
	return is_symbol ? symbol_hash_to_token_type.at(hash) : TOKEN_ERROR;
}

TokenType get_double_symbol_token_type(u8 c, u8 c2)
{
	u64 hash = ((u64)c << 7) | (u64)c2;
	bool is_symbol = symbol_hash_to_token_type.find(hash) != symbol_hash_to_token_type.end();
	return is_symbol ? symbol_hash_to_token_type.at(hash) : TOKEN_ERROR;
}

bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
bool is_number(u8 c) { return c >= '0' && c <= '9'; }
bool is_ident(u8 c)  { return is_letter(c) || (c == '_') || is_number(c); }
bool is_whitespace(u8 c) { return c == ' ' || c == '\t'; }
bool is_line_break(u8 c) { return c == '\r' || c == '\n'; }

bool is_first_of_ident(u8 c)  { return is_letter(c) || (c == '_'); }
bool is_first_of_number(u8 c) { return is_number(c); }
bool is_first_of_string(u8 c) { return c == '"'; }
bool is_first_of_symbol(u8 c) { return get_single_symbol_token_type(c) != TOKEN_ERROR; }

enum LexemeType
{
	LEXEME_IDENT,
	LEXEME_NUMBER,
	LEXEME_STRING,
	LEXEME_SYMBOL,

	LEXEME_ERROR,
};

LexemeType get_lexeme_type(u8 c)
{
	if (is_first_of_ident(c))  return LEXEME_IDENT;
	if (is_first_of_number(c)) return LEXEME_NUMBER;
	if (is_first_of_string(c)) return LEXEME_STRING;
	if (is_first_of_symbol(c)) return LEXEME_SYMBOL;
	return LEXEME_ERROR;
}

bool Lexer::set_input_from_file(const char* file_path)
{
	input_cursor = 0;
	return os_file_read_all(file_path, &input);
}

LineInfo Lexer::get_next_line()
{
	u64 count = input.count;
	u64 i = input_cursor;

	LineInfo line = { i, i, 0 };

	if (i >= count) 
	{
		line.is_valid = false;
		return line;
	}

	while (i < count && is_whitespace(input.data[i]))
	{
		line.leading_spaces += 1;
		i++;
	}

	line.end_cursor = i;

	while (i < count && !is_line_break(input.data[i]))
	{
		line.end_cursor = i;
		line.is_empty = false;
		i++;
	}

	if (i < count && input.data[i] == '\r') i++;
	if (i < count && input.data[i] == '\n') i++;
	input_cursor = i;

	line.start_cursor += line.leading_spaces;

	return line;
}

void Lexer::tokenize()
{
	LineInfo line = {};
	u32 current_line_number = 0;

	while (line.is_valid)
	{
		line = get_next_line();
		current_line_number += 1;
		if (line.is_empty) continue;

		for (u64 i = line.start_cursor; i <= line.end_cursor; )
		{
			u8 fc = input.data[i];

			if (is_whitespace(fc))
			{
				i++;
				continue;
			}

			LexemeType type = get_lexeme_type(fc);
			u64 lexeme_start = i;
			u64 lexeme_end = i + 1;

			Token token = {};
			token.l0 = current_line_number;
			token.c0 = (u32)(1 + i - (line.start_cursor - line.leading_spaces));

			switch (type)
			{
				case LEXEME_IDENT:
				{
					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						if (!is_ident(c)) break;
						lexeme_end += 1;
					}

					token.type = TOKEN_IDENT;
					token.string_value.data = input.data + lexeme_start;
					token.string_value.count = lexeme_end - lexeme_start;

					TokenType keyword = get_keyword_token_type(token.string_value);
					if (keyword != TOKEN_ERROR) token.type = keyword;

					i = lexeme_end;
					lexeme_end -= 1;
				} break;
				case LEXEME_NUMBER:
				{
					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						if (!is_number(c)) break;
						lexeme_end += 1;
					}

					token.type = TOKEN_NUMBER;
					//@Incompelete support double and float
					//@Incomplete support hex integers
					//@Incomplete report number lex errors
					//@Incomplete store numeric value in token

					i = lexeme_end;
					lexeme_end -= 1;
				} break;
				case LEXEME_STRING:
				{
					bool terminated = false;

					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						lexeme_end += 1;
						if (c == '"') { terminated = true; break; }
					}

					token.type = TOKEN_STRING;
					token.string_value.data = input.data + lexeme_start;
					token.string_value.count = lexeme_end - lexeme_start;

					if (!terminated) std::cout << "ERROR: Lexer: string literal was not terminated. line:" << current_line_number << "\n";
					
					i = lexeme_end;
					lexeme_end -= 1;
				} break;
				case LEXEME_SYMBOL:
				{
					token.type = get_single_symbol_token_type(fc);

					if (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];

						TokenType symbol = get_double_symbol_token_type(fc, c);
						if (symbol != TOKEN_ERROR) 
						{
							token.type = symbol;
							lexeme_end += 1;
						}
					}

					i = lexeme_end;
					lexeme_end -= 1;
				} break;
				case LEXEME_ERROR:
				{
					i++;
					//@Incomplete create centralized error reporting functionality
					std::cout << "ERROR: Lexer: character is invalid." << (int)fc << "\n";
				} break;
			}

			//@Debug printing token info
			std::cout << "Token:" << token.type << " line: " << token.l0 << " col: " << token.c0 << " ";
			if (token.type == TOKEN_STRING || token.type == TOKEN_IDENT)
			{
				for (u64 k = 0; k < token.string_value.count; k++)
				std::cout << (char)token.string_value.data[k];
			}
			std::cout << "\n";

			if (type != LEXEME_ERROR)
			tokens.emplace_back(token);
		}

		Token token_eof = {};
		token_eof.type = TOKEN_EOF;
		tokens.emplace_back(token_eof);
	}
}
