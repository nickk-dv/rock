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

int check(char* filepath)
{
	Timer timer;
	timer.start();
	Parser parser = {};
	if (!parser.init(filepath))
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

#define CLI_ON 0

#if CLI_ON == 0
int main(int argc, char** argv)
{
	check("../../test_tp.txt");
}
#else
int main(int argc, char** argv) //@Hack hardcoded for now, will add better argumnet processing layer when needed.
{
	for (int i = 1; i < argc; i++) 
	{
		char* arg = argv[i];

		if (strcmp("help", arg) == 0)
		{
			printf("Commands:\ncheck [filepath]\n");
			return 0;
		}
		else if (strcmp("check", arg) == 0)
		{
			if (i + 1 < argc)
			{
				char* filepath = argv[i + 1];
				return check(filepath);
			}
			else printf("Expected [filepath].\n");
			return 0;
		}
		else
		{
			printf("Unknown command. Use 'help' for list of commands.\n");
			return 0;
		}
	}
	return 0;
}
#endif
