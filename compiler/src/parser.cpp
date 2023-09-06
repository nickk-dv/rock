#include "parser.h"

#include "common.h"

#include <time.h> //@Performance timing

void Parser::parse(const Lexer& lexer)
{
	clock_t start_clock = clock(); //@Performance timing

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
				token_debug_print(token);
				i++;
			}
		}
	}

	clock_t time = clock() - start_clock; //@Performance
	float time_ms = (float)time / CLOCKS_PER_SEC * 1000.0f;

	printf("Parsing done.\n");
	printf("Parser: Time           (Ms): %f \n", time_ms);
}
