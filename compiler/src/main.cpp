#include <unordered_map>
#include <iostream>
#include <optional>
#include <chrono>

#include "common.cpp"
#include "tokenizer.cpp"
#include "ast.cpp"
#include "debug_printer.cpp"
#include "parser.cpp"
#include "checker.cpp"
#include "llvm_convert.cpp"

int main()
{
	Tokenizer lexer = {};

	Timer timer;
	timer.start();
	const char* file_path = "../../test.txt";
	if (!lexer.set_input_from_file(file_path))
	{
		printf("Failed to open a file.\n");
		return 1;
	}
	timer.end("Lexer init");

	timer.start();
	std::vector<Token> tokens = lexer.tokenize();
	timer.end("Lexer");
	lexer.print_debug_metrics(tokens);

	timer.start();
	Parser parser(std::move(tokens));
	timer.end("Parser init");

	timer.start();
	std::optional<Ast> ast = parser.parse();
	timer.end("Parser");

	if (!ast.has_value()) return 1;
	//debug_print_ast(&ast.value());
	printf("Parse result: Success\n\n");

	timer.start();
	bool check = check_ast(&ast.value());
	if (!check) return 1;
	timer.end("Check");
	printf("Check result: Success\n");

	timer.start();
	llvm_convert_build(&ast.value());
	timer.end("LLVM IR build");

	return 0;
}
