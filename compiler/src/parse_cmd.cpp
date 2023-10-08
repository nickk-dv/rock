#include "parse_cmd.h"

#include "common.h"
#include "parser.h"
#include "checker.h"
#include "llvm_ir_builder.h"
#include "llvm_backend.h"
#include "debug_printer.h"

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

#include <filesystem>
namespace fs = std::filesystem;
#include <unordered_map>

int cmd_build(char* filepath)
{
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
			if (!parser->init(file.c_str()))
			{
				printf("Failed to open a file, or file empty: %s\n", file.c_str());
				parse_error = true;
				continue;
			}
			
			Ast* ast = parser->parse();
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

	bool check = true;

	timer.start();
	for (const auto& [module, ast] : modules)
	{
		if(!check_declarations(ast, modules)) check = false;
	}
	if (!check) return 1;

	for (const auto& [module, ast] : modules)
	{
		if (!check_ast(ast)) check = false;
	}
	if (!check) return 1;
	timer.end("Check Ast");

	return 0;

	/*
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
	bool res = check_ast(ast);
	if (!res)
	{
		printf("Ast check failed.\n");
		return 1;
	}
	timer.end("Check Ast      ");

	debug_print_ast(ast);
	
	timer.start();
	LLVM_IR_Builder ir_builder = {};
	LLVMModuleRef mod = ir_builder.build_module(ast);
	timer.end("LLVM IR Builder");

	timer.start();
	LLVM_Backend backend = {};
	backend.build_binaries(mod);
	timer.end("LLVM Backend   ");

	return 0;
	*/
}
