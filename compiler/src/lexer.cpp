#include "lexer.h"

#include "common.h"

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

void Lexer::tokenize()
{
	
}
