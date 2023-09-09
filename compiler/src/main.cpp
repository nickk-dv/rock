#include "lexer.h"
#include "parser.h"

#include <iostream>
#include <chrono>

int main()
{
	Lexer lexer = {};

	if (!lexer.set_input_from_file("../../test_perf.txt")) 
		exit(EXIT_FAILURE);

	typedef std::chrono::high_resolution_clock Clock;
	auto t0 = Clock::now();
	std::vector<Token> tokens = lexer.tokenize();
	auto t1 = Clock::now();
	std::chrono::nanoseconds ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0);
	std::cout << "ms: " << ns.count() / 1000000.0f << '\n';
	lexer.print_debug_metrics(tokens);

	Timer parseTimer;
	Parser parser(std::move(tokens));
	std::optional<Ast> ast = parser.parse();

	printf("Parse time (ms): %f\n", parseTimer.Ms());
	if (ast.has_value())
		printf("Parse result: Success\n");
	else printf("Parse result: FAILED\n");

	return 0;
}
