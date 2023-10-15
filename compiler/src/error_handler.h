#ifndef ERROR_HANDLER_H
#define ERROR_HANDLER_H

//@Notice just storing has_err, in the future extend err handler to simplify err reporting of checker
struct Error_Handler
{
	bool has_err = false;
};

#define err_set err->has_err = true

#endif
