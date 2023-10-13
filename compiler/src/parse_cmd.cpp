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

		if (match_arg(arg, "help"))
		{
			printf("Commands:\nbuild [filepath]\n");
			return 0;
		}
		else if (match_arg(arg, "buildfile"))
		{
			if (i + 1 < argc)
			{
				char* filepath = argv[i + 1];
				return cmd_build_file(filepath);
			}
			else printf("Expected [filepath].\n");
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

	printf("Usage: use 'help' command in your terminal.\n");
	return 0;
}

bool match_arg(char* arg, const char* match)
{
	return strcmp(arg, match) == 0;
}

int cmd_build_file(char* filepath)
{
	tokenizer_init();

	Timer timer;
	timer.start();
	Parser parser = {};
	if (!parser_init(&parser, filepath))
	{
		printf("Failed to open a file.\n");
		return 1;
	}
	timer.end("Parser init    ");
	
	timer.start();
	Ast* ast = parser_parse(&parser);
	if (ast == NULL)
	{
		printf("Failed to parse Ast.\n");
		return 1;
	}
	timer.end("Parse Ast      ");
	
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
}

#include <filesystem>
namespace fs = std::filesystem;
#include <unordered_map>

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

	bool check = true;
	u64 struct_id_start = 0;
	u64 enum_id_start = 0;
	u64 proc_id_start = 0;
	Ast_Program program = {};

	timer.start();
	for (const auto& [module, ast] : modules)
	{
		ast->struct_id_start = struct_id_start;
		ast->enum_id_start = enum_id_start;
		ast->proc_id_start = proc_id_start;
		
		printf("file: %s\n", module.c_str());
		if(!check_declarations(ast, &program, modules)) check = false;
		
		struct_id_start += ast->structs.size();
		enum_id_start += ast->enums.size();
		proc_id_start += ast->procs.size();
	}
	for (u64 i = 0; i < program.structs.size(); i += 1)
	{
		printf("struct: %llu - ", i);
		debug_print_ident(program.structs[i].struct_decl->type, true, false);
	}
	for (u64 i = 0; i < program.enums.size(); i += 1)
	{
		printf("enum:   %llu - ", i);
		debug_print_ident(program.enums[i].enum_decl->type, true, false);
	}
	for (u64 i = 0; i < program.procedures.size(); i += 1)
	{
		printf("proc:   %llu - ", i);
		debug_print_ident(program.procedures[i].proc_decl->ident, true, false);
	}
	if (!check) return 1;

	for (const auto& [module, ast] : modules)
	{
		if (!check_ast(ast, &program)) check = false;
	}
	if (!check) return 1;
	timer.end("Check Ast");

	return 0;
}
