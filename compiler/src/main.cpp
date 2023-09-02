#include "lexer.h"

int main()
{
	Lexer lexer = {};
	if (lexer.set_input_from_file("test.txt")) 
		lexer.tokenize();

	return 0;
}
