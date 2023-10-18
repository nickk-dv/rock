#include "parse_cmd.h"

#include "common.h"
#include "parser.h"
#include "checker.h"
#include "llvm_ir_builder.h"
#include "llvm_backend.h"

#include <filesystem>
#include <unordered_map>
#include "debug_printer.h"

namespace fs = std::filesystem;

int parse_cmd(int argc, char** argv)
{
	if (argc == 1)
	{
		printf("Usage: use 'help' command in your terminal.\n");
		return 0;
	}
	
	for (int i = 1; i < argc; i += 1)
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

int cmd_build(char* filepath)
{
	tokenizer_init();

	if (!fs::exists(filepath)) 
	{
		printf("Filepath is not found: %s\n", filepath);
		return 1;
	}
	if (!fs::is_directory(filepath)) 
	{
		printf("Filepath is not a directory: %s\n", filepath);
		return 1;
	}
	
	Arena parser_arena(128 * 1024);
	std::vector<Parser*> parsers = {};
	std::unordered_map<std::string, Ast*> modules;
	bool parse_error = false;

	Timer timer;
	timer.start();
	fs::path root = fs::path(filepath);
	for (const auto& entry : fs::recursive_directory_iterator(root))
	{
		if (fs::is_regular_file(entry.path()) && entry.path().extension() == ".txt")
		{
			std::string file = entry.path().string();
			
			Parser* parser = parser_arena.alloc<Parser>();
			parsers.emplace_back(parser);
			if (!parser_init(parser, file.c_str()))
			{
				printf("Failed to open a file, or file empty: %s\n", file.c_str());
				parse_error = true;
				continue;
			}
			
			Ast* ast = parser_parse(parser);
			if (ast == NULL)
			{
				printf("Failed to parse Ast: %s\n", file.c_str());
				parse_error = true;
				continue;
			}

			std::string trim = entry.path().lexically_relative(root).replace_extension("").string();
			modules.emplace(trim, ast);
		}
	}
	if (parse_error) return 1;
	timer.end("Parse init & Parse Ast");

	for (const auto& [module, ast] : modules)
	{
		//debug_print_ast(ast);
	}

	Ast_Program program = {};
	Error_Handler err = {};
	Ast* main_ast = NULL;
	Checker_Context cc = {};

	timer.start();
	for (const auto& [module, ast] : modules)
	{
		checker_context_init(&cc, ast, &program, &err);
		check_decl_uniqueness(&cc, modules);
		if (module == "main") main_ast = ast;
	}
	if (main_ast == NULL)
	{
		printf("Entry not found. Make sure to have src/main file.\n\n");
		err.has_err = true;
	}

	if (err.has_err)
	{
		timer.end("Check Ast Error");
		return 1;
	}

	checker_context_init(&cc, main_ast, &program, &err);
	check_main_proc(&cc);
	for (const auto& [module, ast] : modules) 
	{
		checker_context_init(&cc, ast, &program, &err);
		check_decls(&cc);
	}
	if (err.has_err)
	{
		timer.end("Check Ast Error");
		return 1;
	}

	checker_context_init(&cc, NULL, &program, &err);
	check_program(&cc);
	if (err.has_err)
	{
		timer.end("Check Ast Error");
		return 1;
	}
	
	for (const auto& [module, ast] : modules)
	{
		checker_context_init(&cc, ast, &program, &err);
		check_ast(&cc);
	}
	if (err.has_err) 
	{
		timer.end("Check Ast Error");
		return 1;
	}
	timer.end("Check Ast");

	printf("LLVMModule Build (new ir builder): \n\n");
	timer.start();
	LLVMModuleRef mod = build_module(&program);
	timer.end("LLVM Build IR");
	
	timer.start();
	LLVM_Backend backend = {};
	backend.build_binaries(mod);
	timer.end("LLVM Backend ");

	return 0;
}
