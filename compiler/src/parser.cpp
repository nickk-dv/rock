#include "parser.h"

#include "common.h"
#include "error.h"
#include "token.h"

Parser::Parser(std::vector<Token> tokens) 
	: m_tokens(std::move(tokens)), 
	  m_arena(1024 * 1024 * 4) {}

void Parser::parse()
{
	for (u64 i = 0; i < m_tokens.size(); )
	{
		const Token& token = m_tokens[i];

		switch (token.type)
		{
			case TOKEN_KEYWORD_STRUCT: 
			{
				//printf("struct\n");
				i++;
			} break;
			case TOKEN_KEYWORD_ENUM: 
			{
				//printf("enum\n"); 
				i++;
			} break;
			case TOKEN_KEYWORD_FN: 
			{
				//printf("fn\n");
				i++;
			} break;
			default:
			{
				//error_report(PARSE_ERROR_TEST, token);
				i++;
			}
		}
	}
}
