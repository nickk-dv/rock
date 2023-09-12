#include <unordered_map>
#include <iostream>
#include <optional>
#include <chrono>

#include "common.cpp"
#include "tokenizer.cpp"
#include "error.cpp"
#include "ast.cpp"
#include "parser.cpp"
#include "semantic.cpp"

int main()
{
	Tokenizer lexer = {};

	if (!lexer.set_input_from_file("../../test.txt")) 
		exit(EXIT_FAILURE);

	typedef std::chrono::high_resolution_clock Clock;

	auto t0 = Clock::now();
	std::vector<Token> tokens = lexer.tokenize();
	auto t1 = Clock::now();
	std::chrono::nanoseconds ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0);
	std::cout << "Lexer ms: " << ns.count() / 1000000.0f << '\n';

	lexer.print_debug_metrics(tokens);
	
	auto p0 = Clock::now();
	Parser parser(std::move(tokens));
	std::optional<Ast> ast = parser.parse();
	auto p1 = Clock::now();
	std::chrono::nanoseconds pns = std::chrono::duration_cast<std::chrono::nanoseconds>(p1 - p0);
	std::cout << "Parser ms: " << pns.count() / 1000000.0f << '\n';
	
	if (ast.has_value())
	{
		printf("Parse result: Success\n");
		parser.debug_print_ast(&ast.value());
	}
	else printf("Parse result: FAILED\n");

	return 0;
}
