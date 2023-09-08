#include "lexer.h"
#include "parser.h"

int main()
{
	Lexer lexer = {};

	if (!lexer.set_input_from_file("test_perf.txt")) 
		exit(EXIT_FAILURE);

	Timer lexTimer;
	std::vector<Token> tokens = lexer.tokenize();
	printf("Lexer time (ms): %f\n", lexTimer.Ms());
	lexer.print_debug_metrics(tokens);

	//Timer parseTimer;
	//Parser parser(std::move(tokens));
	//parser.parse();
	//printf("Parse time (ms): %f\n", parseTimer.Ms());

	return 0;
}
