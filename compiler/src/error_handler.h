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
};

#endif
