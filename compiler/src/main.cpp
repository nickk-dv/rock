#include <unordered_map>
#include <unordered_set>
#include <optional>
#include <vector>

#include "common.cpp"
#include "tokenizer.cpp"
#include "ast.cpp"
#include "debug_printer.cpp"
#include "parser.cpp"
#include "typer.cpp"
#include "checker.cpp"
#include "llvm_backend.cpp"

int main()
{
	const char* file_path = "../../test_tp.txt";

	Timer timer;
	timer.start();
	Parser parser = {};
	if (!parser.tokenizer.set_input_from_file(file_path))
	{
		printf("Failed to open a file.\n");
		return 1;
	}
	timer.end("Tokenizer & Parser init");

	timer.start();
	Ast* ast = parser.parse();
	timer.end("Parse Ast");

	if (ast == NULL)
	{
		printf("Parse result: Failed to parse Ast.\n");
		return 1;
	}
	//debug_print_ast(ast);
	printf("Parse result: Success\n\n");

	//Vector pre alloc lex + parse
	//0.75ms
	//0.8ms

	//
	//timer.start();
	//Checker checker = {};
	//bool check = checker.check_ast(ast);
	//timer.end("Check");
	//if (!check)
	//{
	//	printf("Check result: Failed\n");
	//	return 1;
	//}
	//printf("Check result: Success\n\n");

	//timer.start();
	//llvm_build(ast);
	//timer.end("LLVM IR build");

	return 0;
}

//@Incomplete basic CLI. Build this up when there are multiple options required
int main_cli(int argc, char **argv)
{
	for (int i = 1; i < argc; i++) 
	{
		printf("Arg: %s\n", argv[i]);
	}
	return 0;
}
