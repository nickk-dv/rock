#include "lexer.h"

#include "common.h"

#include <vector> //@TODO maybe use custom grovable array with some heuristic for the initial size of token stream
#include <iostream> //@DEBUG for debug printing only

bool Lexer::set_input_from_file(const char* file_path)
{
	bool result = os_file_read_all(file_path, &input);
	if (!result) return false;

	input_cursor = 0;
	current_line_number = 1;
	current_char_number = 1;

	return true;
}

bool is_whitespace(u8 c)
{
	return c == ' ' || c == '\t';
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
			// Handle CRLF
			i++;
			if (i < count && input.data[i] == '\n')
			{
				i++;
			}
		}
		else if (input.data[i] == '\n') 
		{
			// Handle LF
			i++;
		}
	}

	// Update the input cursor to point to the start of the next line
	input_cursor = i;

	return line;
}

bool is_letter(u8 c)
{
	return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z');
}

bool is_number(u8 c)
{
	return c >= '0' && c <= '9';
}

bool is_first_of_ident(u8 c)
{
	return is_letter(c) || (c == '_');
}

bool is_ident(u8 c)
{
	return is_letter(c) || (c == '_') || is_number(c);
}

#include <unordered_set>

//@SYNC with single character tokens

const std::unordered_set<u8> valid_symbol_lexeme_characters = 
{
	TOKEN_ASSIGN, TOKEN_SEMICOLON, TOKEN_DOT, TOKEN_COMA,
	TOKEN_SCOPE_START, TOKEN_SCOPE_END, 
	TOKEN_BRACKET_START, TOKEN_BRACKET_END,
	TOKEN_PARENTHESIS_START, TOKEN_PARENTHESIS_END,
	TOKEN_PLUS, TOKEN_MINUS, TOKEN_TIMES, TOKEN_DIV, TOKEN_MOD,
	TOKEN_BITWISE_AND, TOKEN_BITWISE_OR, TOKEN_BITWISE_XOR, TOKEN_BITWISE_NOT,
	TOKEN_LOGIC_NOT, TOKEN_LESS, TOKEN_GREATER,
	':', //for :: support
};

LexemeType get_lexeme_type(u8 c)
{
	if (is_letter(c) || (c == '_')) 
		return LEXEME_IDENT;

	if (is_number(c)) 
		return LEXEME_NUMBER;

	if (c == '"') 
		return LEXEME_STRING;

	if (valid_symbol_lexeme_characters.find(c) != valid_symbol_lexeme_characters.end()) 
		return LEXEME_SYMBOL;

	return LEXEME_ERROR;
}

void Lexer::tokenize()
{
	/*

	indentifier:     => can be also be a keyword
		start:       letter or underscore
		followed_by: letter or underscore or number
		until:       not (letter or underscore or number) or line_end
		until_error: no error

	string:
		start:       "
		followed_by: any u8 character
		until:       "
		until_error: no closing " before line_end

		@TODO string literal escape sequences \\ \" \n \t \r etc,
		might need to store string literals separately from file text memory

	number:
		supported notation: -1 1 -2.0 2.0 0xF0

		@TODO specify

	special_char:
		1 or 2 special_chars: = *= && ||

		@TODO specify

	*/

	std::vector<Token> tokens;

	LineInfo line = {};

	while (line.is_valid)
	{
		line = get_next_line();
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

					std::cout << "\n";
					std::cout << "lexeme start-end: " << lexeme_start << " " << lexeme_end << "\n";
					//no error can exist ident just ends when its no longer ident chars
					break;
				}
				case LEXEME_NUMBER:
				{
					std::cout << "LEXEME_NUMBER" << "\n";
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

					std::cout << "\n";
					std::cout << "lexeme start-end: " << lexeme_start << " " << lexeme_end << "\n";
					if (error) std::cout << "Error: LexemeString wasnt terminated.\n";
					break;
				}
				case LEXEME_SYMBOL:
				{
					std::cout << "LEXEME_SYMBOL" << "\n";
					break;
				}
				case LEXEME_ERROR:
				{
					std::cout << "Lexer: LEXEME_ERROR" << "\n";
					break;
				}
			}
		}

		std::cout << "\n";
	}
}
