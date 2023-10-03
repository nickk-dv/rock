#include "parse_cmd.h"

#include "common.h"
#include "parser.h"
#include "llvm_ir_builder.h"
#include "llvm_backend.h"

int parse_cmd(int argc, char** argv)
{
	for (int i = 1; i < argc; i++)
	{
		char* arg = argv[i];

		if (arg_match(arg, "help"))
		{
			printf("Commands:\nbuild [filepath]\n");
			return 0;
		}
		else if (arg_match(arg, "build"))
		{
			if (i + 1 < argc)
			{
				char* filepath = argv[i + 1];
				return cmd_build(filepath);
			}
			else printf("Expected [filepath].\n");
			return 0;
		}
		else
		{
			printf("Unknown command. Use 'help' for a list of available commands.\n");
			return 0;
		}
	}

	printf("Usage: use 'help' command in your terminal.\n");
	return 0;
}

bool arg_match(char* arg, const char* match)
{
	return strcmp(arg, match) == 0;
}

int cmd_build(char* filepath)
{
	Timer timer;
	timer.start();
	Parser parser = {};
	if (!parser.init(filepath))
	{
		printf("Failed to open a file.\n");
		return 1;
	}
	timer.end("Parser init    ");

	timer.start();
	Ast* ast = parser.parse();
	if (ast == NULL)
	{
		printf("Failed to parse Ast.\n");
		return 1;
	}
	timer.end("Parse Ast      ");

	timer.start();
	LLVM_IR_Builder ir_builder = {};
	LLVMModuleRef mod = ir_builder.build_module(ast);
	timer.end("LLVM IR Builder");

	timer.start();
	LLVM_Backend backend = {};
	backend.build_binaries(mod);
	timer.end("LLVM Backend   ");

	return 0;
}
