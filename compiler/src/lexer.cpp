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

void Lexer::tokenize()
{
	
}
