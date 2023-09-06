#include "lexer.h"
#include "parser.h"

int main()
{
	Lexer lexer = {};

	if (!lexer.set_input_from_file("test.txt")) 
		exit(EXIT_FAILURE);

	lexer.tokenize();
	Parser parser = {};
	parser.parse(lexer);

	return 0;
}
