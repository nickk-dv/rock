export module cmd;

import general;
import parser;
import fmt;
import ast;
import err_handler;
import check;
//@todo enable later #include "middlend/llvm_ir_builder.h"
//@todo enable later #include "backend/llvm_backend.h"
import <fstream>;
import <filesystem>;
namespace fs = std::filesystem;

export i32 parse_cmd(int argc, char** argv);
bool match_arg(char* arg, const char* match);
i32 cmd_help();
i32 cmd_new(char* name);
i32 cmd_check();
i32 cmd_build();
i32 cmd_run();
i32 cmd_fmt();

module : private;

i32 parse_cmd(i32 argc, char** argv)
{
	if (argc == 1)
	{
		err_report(Error::CMD_NO_ARGS);
		return 1;
	}

	if (argc == 2)
	{
		char* arg = argv[1];
		if (match_arg(arg, "help")  || match_arg(arg, "h")) return cmd_help();
		if (match_arg(arg, "check") || match_arg(arg, "c")) return cmd_check();
		if (match_arg(arg, "build") || match_arg(arg, "b")) return cmd_build();
		if (match_arg(arg, "run")   || match_arg(arg, "r")) return cmd_run();
		if (match_arg(arg, "fmt")   || match_arg(arg, "f")) return cmd_fmt();
	}

	if (argc == 3)
	{
		char* arg = argv[1];
		char* arg2 = argv[2];
		if (match_arg(arg, "new") || match_arg(arg, "n")) return cmd_new(arg2);
	}

	err_report(Error::CMD_INVALID);
	return 1;
}

bool match_arg(char* arg, const char* match)
{
	return strcmp(arg, match) == 0;
}

i32 cmd_help()
{
	printf("Commands:\n");
	printf("  help  h\n");
	printf("  new   n [name]\n");
	printf("  check c\n");
	printf("  build b\n");
	printf("  run   r\n");
	printf("  fmt   f\n");
	return 0;
}

i32 cmd_new(char* name)
{
	if (fs::exists(name)) { err_report(Error::CMD_NEW_DIR_ALREADY_EXIST); return 1; }
	if (!fs::create_directory(name)) { err_report(Error::OS_DIR_CREATE_FAILED); return 1; } //@add context
	fs::current_path(name);
	fs::path root_path = fs::current_path();
	
	bool git_intalled = system("git version") == 0;
	if (!git_intalled) { err_report(Error::CMD_NEW_GIT_NOT_INSTALLED); return 1; }
	bool git_init = system("git init") == 0;
	if (!git_init) { err_report(Error::CMD_NEW_GIT_INIT_FAILED); return 1; }
	
	if (!fs::create_directory("src")) { err_report(Error::OS_DIR_CREATE_FAILED); return 1; } //@add context
	if (!fs::create_directory("build")) { err_report(Error::OS_DIR_CREATE_FAILED); return 1; } //@add context
	
	fs::current_path(root_path);
	std::ofstream ignore(".gitignore");
	if (!ignore.is_open()) { err_report(Error::OS_FILE_CREATE_FAILED); return 1; } //@add context
	ignore << "build/\n";
	ignore.close();

	fs::current_path("src");
	std::ofstream file("main.txt"); //@Branding change extension
	if (!file.is_open()) { err_report(Error::OS_FILE_CREATE_FAILED); return 1; } //@add context
	file << "\nmain :: () :: i32 {\n";
	file << "\treturn 0;\n";
	file << "}\n";
	file.close();
	return 0;
}

i32 cmd_check()
{
	Parser parser = {};
	Ast_Program* program = parser.parse_program();
	if (program == NULL) return 1;
	
	bool check = check_program(program);
	if (!check) return 1;

	printf("Check success\n");
	return 0;
}

i32 cmd_build()
{
	Parser parser = {};
	Ast_Program* program = parser.parse_program();
	if (program == NULL) return 1;

	bool check = check_program(program);
	if (!check) return 1;
	
	//@todo enable later LLVMModuleRef mod = build_module(program);
	//@todo enable later backend_build_module(mod);

	printf("Build success\n");
	return 0;
}

i32 cmd_run()
{
	Parser parser = {};
	Ast_Program* program = parser.parse_program();
	if (program == NULL) return 1;

	//for (Ast* ast : program->modules)
	//{
	//	fmt_ast(ast);
	//}
	
	bool check = check_program(program);
	if (!check) return 1;

	//@todo enable later LLVMModuleRef mod = build_module(program);
	//@todo enable later backend_build_module(mod);

	//@todo enable later backend_run();

	printf("Run success\n");
	return 0;
}

i32 cmd_fmt()
{
	return 0;
}
