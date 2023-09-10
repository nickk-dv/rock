#pragma once

#include "token.h"

enum LexerError
{
	LEXER_ERROR_STRING_NOT_TERMINATED,
	LEXER_ERROR_INVALID_CHARACTER,
};

enum ParseError
{
	PARSE_ERROR_TEST,
};

//@Incomplete: maybe delayed error reporting with buffered errors
//@Incomplete: some approach for error counting to not run other compilation stages 
void error_report(LexerError error, Token token);
void error_report(ParseError error, Token token);
void error_report_token(Token token);
void error_report_token_ident(Token token, bool endl = false);
