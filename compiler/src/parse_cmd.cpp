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

		if (match_arg(arg, "help") || match_arg(arg, "h"))
		{
			printf("Commands:\n");
			printf("  help  h\n");
			printf("  check c [path]\n");
			printf("  build b [path]\n");
			printf("  run   r [path]\n");
			return 0;
		}
		else if (match_arg(arg, "check") || match_arg(arg, "c"))
		{
			if (i + 1 < argc)
			{
				char* path = argv[i + 1];
				return cmd_check(path);
			}
			else printf("Expected [path]\n");
			return 0;
		}
		else if (match_arg(arg, "build") || match_arg(arg, "b"))
		{
			if (i + 1 < argc)
			{
				char* path = argv[i + 1];
				return cmd_build(path);
			}
			else printf("Expected [path]\n");
			return 0;
		}
		else if (match_arg(arg, "run") || match_arg(arg, "r"))
		{
			if (i + 1 < argc)
			{
				char* path = argv[i + 1];
				return cmd_run(path);
			}
			else printf("Expected [path]\n");
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

i32 cmd_check(char* path)
{
	Parser parser = {};
	Ast_Program* program = parse_program(&parser, path);
	if (program == NULL) return 1;
	
	bool check = check_program(program);
	if (!check) return 1;

	printf("Check success\n");
	return 0;
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

	printf("Build success\n");
	return 0;
}

i32 cmd_run(char* path)
{
	Parser parser = {};
	Ast_Program* program = parse_program(&parser, path);
	if (program == NULL) return 1;

	bool check = check_program(program);
	if (!check) return 1;

	LLVMModuleRef mod = build_module(program);
	backend_build_module(mod);

	backend_run();

	printf("Run success\n");
	return 0;
}
