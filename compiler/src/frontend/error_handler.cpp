#include "error_handler.h"

#include "check_context.h"
#include "printer.h"
#include <stdio.h>

bool error_status = false;

bool err_get_status()
{
	return error_status;
}

void err_report(Error error)
{
	ErrorMessage message = err_get_message(error);
	error_status = true;
	
	printf("\n\033[0;31m");
	printf("error: ");
	printf("\033[0m");

	printf("%s\n", message.error);
	if (message.hint != "") 
	printf("hint:  %s\n", message.hint);
}

void err_report_parse(Ast* source, TokenType expected, option<const char*> in, Token token)
{
	printf("\n\033[0;31m");
	printf("error: ");
	printf("\033[0m");

	printf("Expected '");
	print_token_type(expected);
	printf("'");
	if (in) printf(" in %s\n", in.value());
	if (token.type == TokenType::INPUT_END)
	{
		printf("%s '", source->filepath.c_str());
		print_token_type(TokenType::INPUT_END);
		printf("'\n");
	}
	else print_span_context(source, token.span);
}

void err_context(Check_Context* cc)
{
	printf("%s: ", cc->ast->filepath.c_str());
}

void err_context(Check_Context* cc, Span span)
{
	print_span_context(cc->ast, span);
}

void err_context(const char* message)
{
	printf("%s\n", message);
}

void err_internal(const char* message)
{
	err_report(Error::COMPILER_INTERNAL);
	err_context(message);
}

ErrorMessage err_get_message(Error error)
{
	switch (error)
	{
	case Error::COMPILER_INTERNAL:              return { "Internal compiler error", "Sumbit a bug report if you see this error" };
	
	case Error::OS_DIR_CREATE_FAILED:           return { "Failed to create a directory", "Verify permissions to create directories at that location" };
	case Error::OS_FILE_CREATE_FAILED:          return { "Failed to create a file", "Verify permissions to create file at that location" };
	case Error::OS_FILE_OPEN_FAILED:            return { "Failed to open a file", "Verify that file isnt used by another process and check for permissions" };
	case Error::OS_FILE_READ_FAILED:            return { "Failed to read a file", "Verify that file isnt corrupted and check for permissions" };

	case Error::CMD_NO_ARGS:                    return { "Usage: use 'help' command in your terminal to list all available commands", "" };
	case Error::CMD_INVALID:                    return { "Unknown command: use 'help' command in your terminal to list all available commands", "" };
	case Error::CMD_NEW_DIR_ALREADY_EXIST:      return { "Directory with the same name already exists", "" };
	case Error::CMD_NEW_GIT_NOT_INSTALLED:      return { "Git is not installed, install git to create a new project", "Failed to run 'git version' command" };
	case Error::CMD_NEW_GIT_INIT_FAILED:        return { "Git init failed while creating a new project", "Failed to run 'git init' command" };
	case Error::PARSE_SRC_DIR_NOT_FOUND:        return { "Failed to find 'src' folder during parsing", "Make sure that current directory is set to the project root directory" };
	
	case Error::MAIN_FILE_NOT_FOUND:            return { "Main file not found", "Make sure src/main file exists" };
	case Error::MAIN_PROC_NOT_FOUND:            return { "Main procedure is not found", "Make sure src/main has main :: () :: i32 { ... }" };
	case Error::MAIN_PROC_EXTERNAL:             return { "Main procedure cannot be external", "Remove '@' from main declaration" };
	case Error::MAIN_PROC_VARIADIC:             return { "Main procedure cannot be variadic", "Remove '..' from the parameter list" };
	case Error::MAIN_NOT_ZERO_PARAMS:           return { "Main procedure must have 0 input parameters", "Main declaration should be: main :: () :: i32" };
	case Error::MAIN_PROC_NO_RETURN_TYPE:       return { "Main procedure must have i32 return type", "Main declaration should be: main :: () :: i32" };
	case Error::MAIN_PROC_WRONG_RETURN_TYPE:    return { "Main procedure must have i32 return type", "Main declaration should be: main :: () :: i32" };
	
	case Error::DECL_SYMBOL_ALREADY_DECLARED:   return { "Symbol is already declared", "" };
	case Error::DECL_IMPORT_PATH_NOT_FOUND:     return { "Import path is not found", "" };
	case Error::DECL_USE_SYMBOL_NOT_FOUND:      return { "Symbol is not found in the imported module", "" };
	case Error::DECL_STRUCT_DUPLICATE_FIELD:    return { "Struct has duplicate field identifiers", "" };
	case Error::DECL_STRUCT_SELF_STORAGE:       return { "Struct has infinite size", "Struct cannot store intance of itself. Use pointers for indirection" };
	case Error::DECL_ENUM_ZERO_VARIANTS:        return { "Enum must have at least one variant", "" };
	case Error::DECL_ENUM_NON_INTEGER_TYPE:     return { "Enum can only have integer basic type", "" };
	case Error::DECL_ENUM_DUPLICATE_VARIANT:    return { "Enum has duplicate variant identifiers", "" };
	case Error::DECL_PROC_DUPLICATE_PARAM:      return { "Procedure has duplicate input parameters", "" };
	
	case Error::RESOLVE_IMPORT_NOT_FOUND:       return { "Import module is not found", "" };
	case Error::RESOLVE_TYPE_NOT_FOUND:         return { "Custom type is not found", "" };
	case Error::RESOLVE_VAR_GLOBAL_NOT_FOUND:   return { "Failed to find global variable in imported module", "" };
	case Error::RESOLVE_ENUM_NOT_FOUND:         return { "Enum type is not declared", "" };
	case Error::RESOLVE_ENUM_VARIANT_NOT_FOUND: return { "Enum variant is not declared", "" };
	case Error::RESOLVE_PROC_NOT_FOUND:         return { "Procedure is not declared", "" };
	case Error::RESOLVE_ARRAY_WRONG_CONTEXT:    return { "Array initializer in non array expression context", "" };
	case Error::RESOLVE_ARRAY_TYPE_MISMATCH:    return { "Array initializer type mismatch", "" };
	case Error::RESOLVE_ARRAY_NO_CONTEXT:       return { "Array initializer type cannot be inferred from expression context", "Provide context: 'var : [2]i32 = { 1, 2 }' or specify array type: 'var := [2]i32 { 1, 2 }'" };
	case Error::RESOLVE_STRUCT_NOT_FOUND:       return { "Struct type is not declared", "" };
	case Error::RESOLVE_STRUCT_WRONG_CONTEXT:   return { "Struct initializer in non struct expression context", "" };
	case Error::RESOLVE_STRUCT_TYPE_MISMATCH:   return { "Struct initializer type mismatch", "" };
	case Error::RESOLVE_STRUCT_NO_CONTEXT:      return { "Struct initializer type cannot be inferred from expression context", "Provide context: 'var : Type = .{ 1, 2 }' or specify struct type: 'var : Type.{ 1, 2 }'" };
	
	case Error::CFG_NOT_ALL_PATHS_RETURN:       return { "Not all control flow paths return a value", "" };
	case Error::CFG_UNREACHABLE_STATEMENT:      return { "Statement is never executed", "Comment out or remove dead code" };
	case Error::CFG_NESTED_DEFER:               return { "Defer cannot contain nested 'defer' statement", "" };
	case Error::CFG_RETURN_INSIDE_DEFER:        return { "Defer cannot contain 'return' statement", "" };
	case Error::CFG_BREAK_INSIDE_DEFER:         return { "Defer cannot contain 'break' statement", "Break can only be used in loops declared inside the defer block" };
	case Error::CFG_CONTINUE_INSIDE_DEFER:      return { "Defer cannot contain 'continue' statement", "Continue can only be used in loops declared inside the defer block" };
	case Error::CFG_BREAK_OUTSIDE_LOOP:         return { "Break outside a loop", "" };
	case Error::CFG_CONTINUE_OUTSIDE_LOOP:      return { "Continue outside a loop", "" };

	case Error::VAR_LOCAL_NOT_FOUND:            return { "Local variable is not found in current scope", "" };
	case Error::RETURN_EXPECTED_NO_EXPR:        return { "Expected no return expression", "" };
	case Error::RETURN_EXPECTED_EXPR:           return { "Expected return expression to match return type of the procedure", "" };
	case Error::SWITCH_INCORRECT_EXPR_TYPE:     return { "Switching is only allowed on expression of enum or integer type", "" };
	case Error::SWITCH_ZERO_CASES:              return { "Switch must have at least one case", "" };
	case Error::VAR_DECL_ALREADY_IS_GLOBAL:     return { "Global variable with same identifier is already in scope", "" };
	case Error::VAR_DECL_ALREADY_IN_SCOPE:      return { "Variable is already in scope", "Variable shadowing isnt supported" };

	case Error::TYPE_MISMATCH:                  return { "Type mismatch", "" };
	case Error::EXPR_EXPECTED_CONSTANT:         return { "Expected constant expression", "" };
	case Error::CONST_PROC_IS_NOT_CONST:        return { "Constant expression cannot contain procedure call", "" };
	case Error::CONST_VAR_IS_NOT_GLOBAL:        return { "Constant expression variable must be a global variable", "" };
	case Error::CONSTEVAL_DEPENDENCY_CYCLE:     return { "Compile time constant is part of dependency cycle", "" };
	
	case Error::CAST_EXPR_NON_BASIC_TYPE:       return { "Cast can only be used on expressions of numeric basic types", "" };
	case Error::CAST_EXPR_BOOL_BASIC_TYPE:      return { "Cast cannot be used on bool expressions", "" };
	case Error::CAST_EXPR_STRING_BASIC_TYPE:    return { "Cast cannot be used on string expressions", "" };
	case Error::CAST_INTO_BOOL_BASIC_TYPE:      return { "Cast into bool is not supported", "" };
	case Error::CAST_INTO_STRING_BASIC_TYPE:    return { "Cast into string is not supported", "" };
	case Error::CAST_REDUNDANT_FLOAT_CAST:      return { "Cast into same float type is redundant", "Remove redundant cast" };
	case Error::CAST_REDUNDANT_INTEGER_CAST:    return { "Cast into same integer type is redundant", "Remove redundant cast" };
	case Error::CAST_FOLD_REDUNDANT_INT_CAST:   return { "Cast int to int in constant expression is redundant", "Remove redundant cast" };
	case Error::CAST_FOLD_REDUNDANT_FLOAT_CAST: return { "Cast float to float in constant expression is redundant", "Remove redundant cast" };
	
	case Error::TEMP_VAR_ASSIGN_OP:             return { "Only = operator is supported in variable assignments", "" };
	
	default:
	{ 
		err_internal("Unknown or unspecified error message");
		printf("Error code: %d\n", (int)error);
		return { "", "" }; }
	}
}
