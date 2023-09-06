#include "parser.h"

#include "common.h"
#include "error.h"
#include "token.h"

void Parser::parse(const Lexer& lexer)
{
	for (u64 i = 0; i < lexer.tokens.size(); )
	{
		const Token& token = lexer.tokens[i];

		switch (token.type)
		{
			case TOKEN_KEYWORD_STRUCT: 
			{
				printf("struct\n");
				i++;
			} break;
			case TOKEN_KEYWORD_ENUM: 
			{
				printf("enum\n"); 
				i++;
			} break;
			case TOKEN_KEYWORD_FN: 
			{
				printf("fn\n");
				i++;
			} break;
			default:
			{
				error_report(PARSE_ERROR_TEST, token);
				i++;
			}
		}
	}
}
