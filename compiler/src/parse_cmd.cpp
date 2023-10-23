#include "parse_cmd.h"

#include "frontend/parser.h"
#include "frontend/checker.h"
#include "middlend/llvm_ir_builder.h"
#include "backend/llvm_backend.h"

i32 parse_cmd(i32 argc, char** argv)
{
	if (argc == 1)
	{
		printf("Usage: use 'help' command in your terminal.\n");
		return 0;
	}
	
	for (i32 i = 1; i < argc; i += 1)
	{
		char* arg = argv[i];

		if (match_arg(arg, "help"))
		{
			printf("Commands:\nbuild [filepath]\n");
			return 0;
		}
		else if (match_arg(arg, "build"))
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

	return 0;
}

bool match_arg(char* arg, const char* match)
{
	return strcmp(arg, match) == 0;
}

i32 cmd_build(char* path)
{
	Parser parser = {};
	Ast_Program* program = parse_program(&parser, path);
	if (program == NULL) return 1;
	
	bool check = check_program(program);
	if (!check) return 1;
	
	LLVMModuleRef mod = build_module(program);
	backend_build_module(mod);

	return 0;
}
