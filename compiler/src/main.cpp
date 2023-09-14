#include <unordered_map>
#include <iostream>
#include <optional>
#include <chrono>

#include "common.cpp"
#include "tokenizer.cpp"
#include "error.cpp"
#include "ast.cpp"
#include "debug_printer.cpp"
#include "parser.cpp"
#include "checker.cpp"

int main()
{
	Tokenizer lexer = {};

	const char* file_path = "../../test.txt";
	if (!lexer.set_input_from_file(file_path))
	{
		printf("Failed to open a file.\n");
		exit(EXIT_FAILURE);
	}

	typedef std::chrono::high_resolution_clock Clock;

	auto t0 = Clock::now();
	std::vector<Token> tokens = lexer.tokenize();
	auto t1 = Clock::now();
	std::chrono::nanoseconds ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0);
	std::cout << "Lexer ms: " << ns.count() / 1000000.0f << '\n';

	lexer.print_debug_metrics(tokens);
	Parser parser(std::move(tokens)); //@Performance allocation not measured in parser time

	auto p0 = Clock::now();
	std::optional<Ast> ast = parser.parse();
	auto p1 = Clock::now();
	std::chrono::nanoseconds pns = std::chrono::duration_cast<std::chrono::nanoseconds>(p1 - p0);
	std::cout << "Parser ms: " << pns.count() / 1000000.0f << "\n\n";
	
	if (ast.has_value())
	{
		//debug_print_ast(&ast.value());

		printf("Parse result: Success\n\n");

		bool correct = check_ast(&ast.value());
		if (correct) printf("Check result: Success\n");
		else printf("Check result: FAILED\n");
	}
	else printf("Parse result: FAILED\n");

	return 0;
}
