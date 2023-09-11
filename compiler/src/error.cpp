
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

const char* lexerErrorMessages[] = 
{
    "String literal not terminated.",
    "Invalid input character.",
};

const char* parserErrorMessages[] =
{
    "Test error.",
};

void error_report(LexerError error, Token token)
{
    printf(lexerErrorMessages[error]);
	error_report_token(token);
}

void error_report(ParseError error, Token token)
{
    printf(parserErrorMessages[error]);
	error_report_token(token);
}

void error_report_token(Token token)
{
	printf("Token: %i ", (int)token.type);
	printf("line: %lu col: %lu ", token.l0, token.c0);

	if (token.type == TOKEN_STRING || token.type == TOKEN_IDENT)
	{
		for (u64 k = 0; k < token.string_value.count; k++)
			printf("%c", (char)token.string_value.data[k]);
	}
	else if (token.type < TOKEN_IDENT)
	{
		printf("%c", (char)token.type);
	}
	printf("\n");
}

void error_report_token_ident(Token token, bool endl)
{
	if (token.type == TOKEN_IDENT)
	{
		for (u64 k = 0; k < token.string_value.count; k++)
			printf("%c", (char)token.string_value.data[k]);
	}
	else 
	{
		printf("TOKEN IS NOT AN IDENTIFIER!");
	}
	if (endl) printf("\n");
}
