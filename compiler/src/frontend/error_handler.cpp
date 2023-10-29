#include "error_handler.h"

#include "debug_printer.h"
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

	printf("%s.\n", message.error);
	if (message.hint != "") 
	printf("Hint: %s.\n", message.hint);
}

void err_context(Check_Context* cc)
{
	printf("%s: ", cc->ast->filepath.c_str());
}

void err_context(Check_Context* cc, Span span)
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
	case Error::MAIN_FILE_NOT_FOUND:         return { "Main file not found", "Make sure src/main file exists" };
	case Error::MAIN_PROC_NOT_FOUND:         return { "Main procedure is not found", "Make sure src/main has main :: () :: i32 { ... }" };
	case Error::MAIN_PROC_EXTERNAL:          return { "Main procedure cannot be external", "Remove '@' from main declaration" };
	case Error::MAIN_PROC_VARIADIC:          return { "Main procedure cannot be variadic", "Remove '..' from the parameter list" };
	case Error::MAIN_NOT_ZERO_PARAMS:        return { "Main procedure must have 0 input parameters", "Main declaration should be: main :: () :: i32" };
	case Error::MAIN_PROC_NO_RETURN_TYPE:    return { "Main procedure must have i32 return type", "Main declaration should be: main :: () :: i32" };
	case Error::MAIN_PROC_WRONG_RETURN_TYPE: return { "Main procedure must have i32 return type", "Main declaration should be: main :: () :: i32" };
	
	case Error::SYMBOL_ALREADY_DECLARED:     return { "Symbol is already declared", "" };
	case Error::IMPORT_PATH_NOT_FOUND:       return { "Import path is not found", "" };
	case Error::IMPORT_MODULE_NOT_FOUND:     return { "Import module is not found", "" };
	case Error::USE_SYMBOL_NOT_FOUND:        return { "Symbol is not found in the imported module", "" };
	case Error::STRUCT_DUPLICATE_FIELD:      return { "Struct has duplicate field identifiers", "" };
	case Error::STRUCT_INFINITE_SIZE:        return { "Struct has infinite size", "Struct cannot store intance of itself. Use pointers for indirection" };
	case Error::ENUM_ZERO_VARIANTS:          return { "Enum must have at least one variant", "" };
	case Error::ENUM_NON_INTEGER_TYPE:       return { "Enum can only have integer basic type", "" };
	case Error::ENUM_DUPLICATE_VARIANT:      return { "Enum has duplicate variant identifiers", "" };
	case Error::PROC_DUPLICATE_PARAM:        return { "Procedure has duplicate input parameters", "" };

	case Error::CFG_NOT_ALL_PATHS_RETURN:    return { "Not all control flow paths return a value", "" };
	case Error::CFG_UNREACHABLE_STATEMENT:   return { "Statement is never executed", "Comment out or remove dead code" };
	case Error::CFG_NESTED_DEFER:            return { "Defer cannot be nested", "" };
	case Error::CFG_RETURN_INSIDE_DEFER:     return { "Defer cannot contain 'return' statement", "" };
	case Error::CFG_BREAK_INSIDE_DEFER:      return { "Defer cannot contain 'break' statement", "Break can only be used in loops declared inside defer block" };
	case Error::CFG_CONTINUE_INSIDE_DEFER:   return { "Defer cannot contain 'continue' statement", "Continue can only be used in loops declared inside defer block" };
	case Error::CFG_BREAK_OUTSIDE_LOOP:      return { "Break outside a loop", "" };
	case Error::CFG_CONTINUE_OUTSIDE_LOOP:   return { "Continue outside a loop", "" };
	}
}
