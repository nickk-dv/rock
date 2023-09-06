#include "lexer.h"
#include "parser.h"

int main()
{
	Lexer lexer = {};

	if (!lexer.set_input_from_file("test.txt")) 
		exit(EXIT_FAILURE);

	Timer lexTimer;
	lexer.tokenize();
	printf("Lexer time (ms): %f\n", lexTimer.Ms());
	lexer.print_debug_metrics();

	Parser parser = {};

	Timer parseTimer;
	parser.parse(lexer);
	printf("Parse time (ms): %f\n", parseTimer.Ms());

	return 0;
}
