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

bool is_letter(u8 c)
{
	return (c >= 65 && c <= 90) || (c >= 97 && c <= 122);
}

bool is_number(u8 c)
{
	return c >= 48 && c <= 57;
}

bool is_first_of_ident(u8 c)
{
	return is_letter(c) || (c == '_');
}

bool is_ident(u8 c)
{
	return is_letter(c) || (c == '_') || is_number(c);
}

bool is_first_of_string(u8 c)
{
	return c == '"';
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

	// Find the end of the current line
	while (i < count && input.data[i] != '\n' && input.data[i] != '\r')
	{
		line.end_cursor = i;
		i++;
	}

	u64 length = line.end_cursor - (line.start_cursor + line.leading_spaces);
	line.is_empty = length == 0;

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

u8 Lexer::peek_next_character()
{
	return input.data[input_cursor];
}

void Lexer::eat_character()
{
	input_cursor += 1;
}

void Lexer::tokenize()
{
	//token logic:
	//1. identifier => check with keywords => emit correct token
	//2. number (12 12.5 12,5 500.000.333, hex) 
	///@TODO specify supported number notation convering negative and positive int / floats / doubles
	//3. string ("str") starts and ends with ""
	//4. special character sets (usually 1-2 characters) eg. (= & != *= += | ||) cannot be part of the indentifier

	std::vector<Token> tokens;

	LineInfo line = {};
	while (line.is_valid)
	{
		line = get_next_line();
		if (line.is_empty)
			std::cout << "line empty" << "\n";
		else std::cout << "line: " << line.start_cursor << "-" << line.end_cursor << " spaces:" << line.leading_spaces << "\n";
	}

	input_cursor = 0;

	for (size_t i = 0; i < input.count; i += 1)
	{
		u8 c = peek_next_character();
		eat_character();

		if (c == ' ') continue;

		std::cout << "char: " << (char)c << " " << (int)c << "\n";
	}

	return;
}
