#include "lexer.h"
#include "parser.h"

int main()
{
	Lexer lexer = {};
	if (lexer.set_input_from_file("test.txt"))
	{
		lexer.tokenize();
		if (lexer.errors.size() == 0)
		{
			Parser parser = {};
			parser.parse(lexer);
		}
	}

	return 0;
}
