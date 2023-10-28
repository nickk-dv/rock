#ifndef ERROR_HANDLER_H
#define ERROR_HANDLER_H

//@Notice just storing has_err, in the future extend err handler to simplify err reporting of checker
struct Error_Handler
{
	bool has_err = false;
};

#define err_set cc->err->has_err = true

//@Todo add ability to attach context to the error
//for example ident, token, type, or source code span

enum class Error;
struct ErrorMessage;

bool err_get_status();
void err_report(Error error);
static ErrorMessage error_get_message(Error error);

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
	
	SYMBOL_ALREADY_DECLARED,
	IMPORT_PATH_NOT_FOUND,
	IMPORT_MODULE_NOT_FOUND,
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
};

#endif
