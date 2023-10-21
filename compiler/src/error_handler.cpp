#include "error_handler.h"

#include "common.h"

bool error_status = false;

bool err_get_status()
{
	return error_status;
}

void err_report(Error error)
{
	error_status = true;
	ErrorMessage message = error_get_message(error);
	printf("%s\n", message.error);
	if (message.hint != "") 
	printf("Hint: %s\n", message.hint);
}

ErrorMessage error_get_message(Error error)
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
	}
}
