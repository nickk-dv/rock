module;
#include <stdio.h>

export module err_handler;

import general;
import ast;
import token;
import printer;

enum class Error;

struct ErrorMessage
{
	const char* error;
	const char* hint;
};

export bool err_get_status();
export void err_report(Error error);
export void err_report_parse(Ast_Source* source, TokenType expected, option<const char*> in, Token token);
export void err_context(Ast_Source* source);
export void err_context(Ast_Source* source, Span span);
export void err_context(const char* message);
export void err_internal(const char* message);
ErrorMessage err_get_message(Error error);

export enum class Error
{
	COMPILER_INTERNAL,

	OS_DIR_CREATE_FAILED,
	OS_FILE_CREATE_FAILED,
	OS_FILE_OPEN_FAILED,
	OS_FILE_READ_FAILED,

	CMD_NO_ARGS,
	CMD_INVALID,
	CMD_NEW_DIR_ALREADY_EXIST,
	CMD_NEW_GIT_NOT_INSTALLED,
	CMD_NEW_GIT_INIT_FAILED,
	PARSE_SRC_DIR_NOT_FOUND,
	PARSE_MOD_DECL_DUPLICATE,
	PARSE_MOD_CYCLE,
	PARSE_MOD_NOT_FOUND,
	PARSE_MOD_AMBIGUITY,

	MAIN_FILE_NOT_FOUND,
	MAIN_PROC_NOT_FOUND,
	MAIN_PROC_EXTERNAL,
	MAIN_PROC_VARIADIC,
	MAIN_NOT_ZERO_PARAMS,
	MAIN_PROC_NO_RETURN_TYPE,
	MAIN_PROC_WRONG_RETURN_TYPE,

	DECL_SYMBOL_ALREADY_DECLARED,
	DECL_IMPORT_PATH_NOT_FOUND,
	DECL_USE_SYMBOL_NOT_FOUND,
	DECL_STRUCT_DUPLICATE_FIELD,
	DECL_STRUCT_SELF_STORAGE,
	DECL_ENUM_ZERO_VARIANTS,
	DECL_ENUM_NON_INTEGER_TYPE,
	DECL_ENUM_DUPLICATE_VARIANT,
	DECL_PROC_DUPLICATE_PARAM,
	DECL_PROC_DUPLICATE_EXTERNAL,

	IMPORT_SYMBOL_NOT_FOUND,
	IMPORT_SYMBOL_NOT_PUB,
	IMPORT_SYMBOL_ALREADY_DEFINED,
	IMPORT_SYMBOL_ALREADY_IMPORTED,

	RESOLVE_IMPORT_NOT_FOUND,
	RESOLVE_TYPE_NOT_FOUND,
	RESOLVE_TYPE_ARRAY_ZERO_SIZE,
	RESOLVE_VAR_GLOBAL_NOT_FOUND,
	RESOLVE_ENUM_NOT_FOUND,
	RESOLVE_ENUM_VARIANT_NOT_FOUND,
	RESOLVE_PROC_NOT_FOUND,
	RESOLVE_ARRAY_WRONG_CONTEXT,
	RESOLVE_ARRAY_TYPE_MISMATCH,
	RESOLVE_ARRAY_NO_CONTEXT,
	RESOLVE_STRUCT_NOT_FOUND,
	RESOLVE_STRUCT_WRONG_CONTEXT,
	RESOLVE_STRUCT_TYPE_MISMATCH,
	RESOLVE_STRUCT_NO_CONTEXT,

	CFG_NOT_ALL_PATHS_RETURN,
	CFG_UNREACHABLE_STATEMENT,
	CFG_NESTED_DEFER,
	CFG_RETURN_INSIDE_DEFER,
	CFG_BREAK_INSIDE_DEFER,
	CFG_CONTINUE_INSIDE_DEFER,
	CFG_BREAK_OUTSIDE_LOOP,
	CFG_CONTINUE_OUTSIDE_LOOP,

	VAR_LOCAL_NOT_FOUND,
	VAR_ALREADY_IS_GLOBAL,
	RETURN_EXPECTED_NO_EXPR,
	RETURN_EXPECTED_EXPR,
	SWITCH_INCORRECT_EXPR_TYPE,
	SWITCH_ZERO_CASES,
	VAR_DECL_ALREADY_IN_SCOPE,

	TYPE_MISMATCH,
	EXPR_EXPECTED_CONSTANT,
	CONST_PROC_IS_NOT_CONST,
	CONST_VAR_IS_NOT_GLOBAL,
	CONSTEVAL_DEPENDENCY_CYCLE,

	CAST_EXPR_NON_BASIC_TYPE,
	CAST_EXPR_BOOL_BASIC_TYPE,
	CAST_EXPR_STRING_BASIC_TYPE,
	CAST_INTO_BOOL_BASIC_TYPE,
	CAST_INTO_STRING_BASIC_TYPE,
	CAST_REDUNDANT_FLOAT_CAST,
	CAST_REDUNDANT_INTEGER_CAST,
	CAST_FOLD_REDUNDANT_INT_CAST,
	CAST_FOLD_REDUNDANT_FLOAT_CAST,

	FOLD_UNARY_MINUS_ON_BOOL,
	FOLD_UNARY_MINUS_OVERFLOW,
	FOLD_LOGIC_NOT_ONLY_ON_BOOL,
	FOLD_BITWISE_NOT_ONLY_ON_UINT,
	FOLD_ADDRESS_OF_ON_CONSTANT,
	FOLD_DEREFERENCE_ON_CONSTANT,

	BINARY_EXPR_NON_BASIC,
	BINARY_EXPR_NON_MATCHING_BASIC,
	BINARY_LOGIC_ONLY_ON_BOOL,
	BINARY_CMP_ON_BOOL,
	BINARY_CMP_ON_STRING,
	BINARY_CMP_EQUAL_ON_ENUMS,
	BINARY_MATH_ONLY_ON_NUMERIC,
	BINARY_BITWISE_ONLY_ON_UINT,

	TEMP_VAR_ASSIGN_OP,
};

module : private;

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
	if (message.hint[0] != '\0')
	printf("hint:  %s\n", message.hint);
}

void err_report_parse(Ast_Source* source, TokenType expected, option<const char*> in, Token token)
{
	error_status = true;
	
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

void err_context(Ast_Source* source)
{
	printf("%s: ", source->filepath.c_str());
}

void err_context(Ast_Source* source, Span span)
{
	print_span_context(source, span);
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
	case Error::PARSE_MOD_DECL_DUPLICATE:       return { "Module redeclaration", "Remove duplicate mod declratation" };
	case Error::PARSE_MOD_CYCLE:                return { "Module forms a cycle", "Review and remove cyclic module dependencies" };
	case Error::PARSE_MOD_NOT_FOUND:            return { "Failed to find a module", "" };
	case Error::PARSE_MOD_AMBIGUITY:            return { "File for module found at both possible paths", "Delete or remove one of them to remove ambiguity" };

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
	case Error::DECL_PROC_DUPLICATE_EXTERNAL:   return { "External procedure with same identifier is already defined", "Import and use existing procedure, its redefiniton will cause linker errors" };

	case Error::IMPORT_SYMBOL_NOT_FOUND:        return { "Imported symbol isnt found in referenced module", "" };
	case Error::IMPORT_SYMBOL_NOT_PUB:          return { "Imported symbol is not public", "" };
	case Error::IMPORT_SYMBOL_ALREADY_DEFINED:  return { "Imported symbol is already defined in current module", "" };
	case Error::IMPORT_SYMBOL_ALREADY_IMPORTED: return { "Imported symbol is already imported", "Remove unnecessary import" };

	case Error::RESOLVE_IMPORT_NOT_FOUND:       return { "Import module is not found", "" };
	case Error::RESOLVE_TYPE_NOT_FOUND:         return { "Custom type is not found", "" };
	case Error::RESOLVE_TYPE_ARRAY_ZERO_SIZE:   return { "Static array cannot have size of 0", "Array size expression cannot evaluate to 0" };
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
	case Error::VAR_ALREADY_IS_GLOBAL:          return { "Global variable with same identifier is already in scope", "" };
	case Error::RETURN_EXPECTED_NO_EXPR:        return { "Expected no return expression", "" };
	case Error::RETURN_EXPECTED_EXPR:           return { "Expected return expression to match return type of the procedure", "" };
	case Error::SWITCH_INCORRECT_EXPR_TYPE:     return { "Switching is only allowed on expression of enum or integer type", "" };
	case Error::SWITCH_ZERO_CASES:              return { "Switch must have at least one case", "" };
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

	case Error::FOLD_UNARY_MINUS_ON_BOOL:       return { "Unary op minus '-' cannot be applied on bool values, only on numeric types", "" };
	case Error::FOLD_UNARY_MINUS_OVERFLOW:      return { "Unary op minus '-' results in integer overflow", "Expression value cannot be represented by signed 64 bit integer" };
	case Error::FOLD_LOGIC_NOT_ONLY_ON_BOOL:    return { "Unary op logic not '!' can only be applied to expressions of bool type", "" };
	case Error::FOLD_BITWISE_NOT_ONLY_ON_UINT:  return { "Unary op bitwise not '~' can only be applied to expresions of unsigned integer type", "" };
	case Error::FOLD_ADDRESS_OF_ON_CONSTANT:    return { "Unary op address '*' cannot be applied on temporary or constant value", "" };
	case Error::FOLD_DEREFERENCE_ON_CONSTANT:   return { "Unary op dereference '<<' cannot be applied on temporary or constant value", "" };

	case Error::BINARY_EXPR_NON_BASIC:          return { "Binary ops can only be applied to expressions of basic types", "Enum types represented by set of integer constants can also be used" };
	case Error::BINARY_EXPR_NON_MATCHING_BASIC: return { "Binary ops can only be applied to expressions of same basic types", "Use explicit 'cast', only float casts and integer upcasts are implicit" };
	case Error::BINARY_LOGIC_ONLY_ON_BOOL:      return { "Binary logic ops can only be used on bools", "" };
	case Error::BINARY_CMP_ON_BOOL:             return { "Binary comparison ops cannot be applied to bools", "" };
	case Error::BINARY_CMP_ON_STRING:           return { "Binary comparison ops cannot be applied to strings", "" };
	case Error::BINARY_CMP_EQUAL_ON_ENUMS:      return { "Binary equals ops must be applied on enums of the same type", "" };
	case Error::BINARY_MATH_ONLY_ON_NUMERIC:    return { "Binary math ops can only be used on numeric types", "" };
	case Error::BINARY_BITWISE_ONLY_ON_UINT:    return { "Binary bitwise ops can only be used on unsigned integers", "" };

	case Error::TEMP_VAR_ASSIGN_OP:             return { "Only = operator is supported in variable assignments", "" };
	default:
	{ 
		err_internal("Unknown or unspecified error message");
		printf("Error code: %d\n", (int)error);
		return { "", "" }; }
	}
}