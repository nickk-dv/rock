#ifndef ERROR_HANDLER_H
#define ERROR_HANDLER_H

#include "check_context.h"

enum class Error;
struct ErrorMessage;

bool err_get_status();
void err_report(Error error);
void err_context(Check_Context* cc);
void err_context(Check_Context* cc, Span span);

static ErrorMessage err_get_message(Error error);

struct ErrorMessage
{
	const char* error;
	const char* hint;
};

enum class Error
{
	MAIN_FILE_NOT_FOUND,
	MAIN_PROC_NOT_FOUND,
	MAIN_PROC_EXTERNAL,
	MAIN_PROC_VARIADIC,
	MAIN_NOT_ZERO_PARAMS,
	MAIN_PROC_NO_RETURN_TYPE,
	MAIN_PROC_WRONG_RETURN_TYPE,
	
	RESOLVE_IMPORT_NOT_FOUND,
	RESOLVE_TYPE_NOT_FOUND,
	RESOLVE_VAR_GLOBAL_NOT_FOUND,
	RESOLVE_ENUM_NOT_FOUND,
	RESOLVE_ENUM_VARIANT_NOT_FOUND,
	RESOLVE_PROC_NOT_FOUND,
	RESOLVE_STRUCT_NOT_FOUND,
	
	SYMBOL_ALREADY_DECLARED,
	IMPORT_PATH_NOT_FOUND,
	USE_SYMBOL_NOT_FOUND,
	STRUCT_DUPLICATE_FIELD,
	STRUCT_INFINITE_SIZE,
	ENUM_ZERO_VARIANTS,
	ENUM_NON_INTEGER_TYPE,
	ENUM_DUPLICATE_VARIANT,
	PROC_DUPLICATE_PARAM,
	
	CFG_NOT_ALL_PATHS_RETURN,
	CFG_UNREACHABLE_STATEMENT,
	CFG_NESTED_DEFER,
	CFG_RETURN_INSIDE_DEFER,
	CFG_BREAK_INSIDE_DEFER,
	CFG_CONTINUE_INSIDE_DEFER,
	CFG_BREAK_OUTSIDE_LOOP,
	CFG_CONTINUE_OUTSIDE_LOOP,

	VAR_LOCAL_NOT_FOUND,
	RETURN_EXPECTED_NO_EXPR,
	RETURN_EXPECTED_EXPR,
	SWITCH_INCORRECT_EXPR_TYPE,
	SWITCH_ZERO_CASES,
	VAR_DECL_ALREADY_IS_GLOBAL,
	VAR_DECL_ALREADY_IN_SCOPE,
	TYPE_MISMATCH,
	EXPR_EXPECTED_CONSTANT,

	CONST_PROC_IS_NOT_CONST,
	CONST_VAR_IS_NOT_GLOBAL,

	TEMP_VAR_ASSIGN_OP,
};

#endif
