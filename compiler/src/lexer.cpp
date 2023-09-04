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
	u64 hash = (u64)c;
	hash = (hash << 7) & (u64)c2;
	bool is_symbol = symbol_hash_to_token_type.find(hash) != symbol_hash_to_token_type.end();
	return is_symbol ? symbol_hash_to_token_type.at(hash) : TOKEN_ERROR;
}

bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
bool is_number(u8 c) { return c >= '0' && c <= '9'; }
bool is_ident(u8 c)  { return is_letter(c) || (c == '_') || is_number(c); }
bool is_whitespace(u8 c) { return c == ' ' || c == '\t'; }

bool is_first_of_ident(u8 c)  { return is_letter(c) || (c == '_'); }
bool is_first_of_number(u8 c) { return is_number(c); }
bool is_first_of_string(u8 c) { return c == '"'; }
bool is_first_of_symbol(u8 c) { return get_single_symbol_token_type(c) != TOKEN_ERROR; }

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
	const size_t count = input.count;
	u64 i = input_cursor;

	LineInfo line = { i, i, 0 };

	// End of file
	if (i >= count)
	{
		line.is_valid = false;
		return line;
	}

	// Skip leading whitespaces
	while (i < count && is_whitespace(input.data[i]))
	{
		line.leading_spaces += 1;
		i++;
	}

	bool any_character_found = false;

	// Find the end of the current line
	while (i < count && input.data[i] != '\n' && input.data[i] != '\r')
	{
		any_character_found = true;
		line.end_cursor = i;
		i++;
	}

	u64 length = line.end_cursor - (line.start_cursor + line.leading_spaces);
	line.is_empty = length == 0 || !any_character_found;

	line.start_cursor += line.leading_spaces;

	// Handle CRLF and LF line endings
	if (i < count)
	{
		if (input.data[i] == '\r') 
		{
			i++;
			if (i < count && input.data[i] == '\n') i++;
		}
		else if (input.data[i] == '\n') i++;
	}

	// Update the input cursor to point to the start of the next line
	input_cursor = i;

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

		std::cout << "line: " << line.start_cursor << "-" << line.end_cursor << " spaces:" << line.leading_spaces << "\n";
		
		for (u64 i = line.start_cursor; i <= line.end_cursor; )
		{
			u8 c = input.data[i];

			if (is_whitespace(c))
			{
				i++;
				continue;
			}

			LexemeType type = get_lexeme_type(c);
			u64 lexeme_start = i;
			std::cout << "lexeme start char: " << c << "\n";
			u64 lexeme_end = i + 1;

			Token token = {};
			token.l0 = current_line_number;
			token.c0 = 1 + i - line.start_cursor - line.leading_spaces;

			switch (type)
			{
				case LEXEME_IDENT:
				{
					std::cout << "LEXEME_IDENT" << "\n";

					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];

						if (!is_ident(c))
						{
							break;
						}

						std::cout << c;
						lexeme_end += 1;
					}

					i = lexeme_end;
					lexeme_end -= 1;

					token.type = TOKEN_IDENT;
					token.string_value.data = input.data + lexeme_start;
					token.string_value.count = 1 + lexeme_end - lexeme_start;

					TokenType keyword = get_keyword_token_type(token.string_value);
					if (keyword != TOKEN_ERROR) token.type = keyword;

					std::cout << "\n";
					std::cout << "lexeme start-end: " << lexeme_start << " " << lexeme_end << "\n";
					//no error can exist ident just ends when its no longer ident chars
					break;
				}
				case LEXEME_NUMBER:
				{
					std::cout << "LEXEME_NUMBER" << "\n";
					bool error = false;

					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];

						if (!is_number(c))
						{
							break;
						}

						std::cout << (int)(c - '0');
						lexeme_end += 1;
					}

					i = lexeme_end;
					lexeme_end -= 1;

					token.type = TOKEN_NUMBER;

					std::cout << "\n";
					std::cout << "lexeme start-end: " << lexeme_start << " " << lexeme_end << "\n";
					if (error) std::cout << "Error: LexemeNumber some error.\n";
					break;
				}
				case LEXEME_STRING:
				{
					std::cout << "LEXEME_STRING" << "\n";
					bool error = true;

					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						std::cout << c;
						lexeme_end += 1;

						if (c == '"')
						{
							error = false;
							break;
						}
					}

					i = lexeme_end;
					lexeme_end -= 1;

					token.type = TOKEN_STRING;
					token.string_value.data = input.data + lexeme_start + 1; //@Hack skip first "
					token.string_value.count = 1 + lexeme_end - lexeme_start - 1; //@Hack skip last "

					std::cout << "\n";
					std::cout << "lexeme start-end: " << lexeme_start << " " << lexeme_end << "\n";
					if (error) std::cout << "Error: LexemeString wasnt terminated.\n";
					break;
				}
				case LEXEME_SYMBOL:
				{
					std::cout << "LEXEME_SYMBOL" << "\n";
					i++;

					break;
				}
				case LEXEME_ERROR:
				{
					std::cout << "Lexer: character is invalid.\n";
					i++;

					break;
				}
			}

			std::cout << "Token:" << token.type << "\n";

			tokens.emplace_back(token);
		}

		Token token_eof = {};
		token_eof.type = TOKEN_EOF;
		tokens.emplace_back(token_eof);

		std::cout << "\n";
	}
}
