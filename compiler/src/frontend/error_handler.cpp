#include "error_handler.h"

#include "check_context.h"
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

void err_context(Check_Context* cc)
{
	printf("%s: ", cc->ast->filepath.c_str());
}

void err_context(Check_Context* cc, Span span)
{
	printf("%s:", cc->ast->filepath.c_str());
	printf("%lu:%lu", span.start, span.end);
	printf("\n  |");
	printf("\n4 |");

	while (span.start > 1 && cc->ast->source.data[span.start - 1] != '\n')
	{
		span.start -= 1;
	}

	u32 max_lines = 5;
	u32 lines = 0;

	while (span.start <= span.end)
	{
		u8 c = cc->ast->source.data[span.start];
		printf("%c", c);
		if (c == '\n')
		{
			printf("  |");
			lines += 1;
			if (lines >= max_lines) break;
		}
		span.start += 1;
	}

	if (lines >= max_lines) printf(" ...");
	else printf("\n  |");
	printf("\n");
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

void err_context_old(Check_Context* cc, Span span)
{
	printf("%s:", cc->ast->filepath.c_str());
	printf("%lu:%lu ", span.start, span.end);
	while (span.start <= span.end)
	{
		printf("%c", cc->ast->source.data[span.start]);
		span.start += 1;
	}
	printf("\n");
}

ErrorMessage err_get_message(Error error)
{
	switch (error)
	{
	case Error::COMPILER_INTERNAL:              return { "Internal compiler error", "Sumbit a bug report if you see this error" };
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
	case Error::RESOLVE_STRUCT_NOT_FOUND:       return { "Struct type is not declared", "" };
	
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
	case Error::TEMP_VAR_ASSIGN_OP:             return { "Only = operator is supported in variable assignments", "" };
	
	default:
	{ 
		err_internal("Unknown or unspecified error message");
		printf("Error code: %d\n", (int)error);
		return { "", "" }; }
	}
}
