#ifndef LEXER_H
#define LEXER_H

#include "token.h"

struct Lexer
{
public:
	static const u64 TOKEN_BUFFER_SIZE = 256;
	static const u64 TOKEN_LOOKAHEAD = 4;

private:
	u32 cursor;
	StringView source;
	StringStorage* strings;
	std::vector<Span>* line_spans;

public:
	void init(StringView source, StringStorage* strings, std::vector<Span>* line_spans);
	void lex_token_buffer(Token* tokens);

private:
	Token lex_token();
	Token lex_char();
	Token lex_string();
	Token lex_number();
	Token lex_ident();
	Token lex_symbol();
	TokenType lex_ident_keyword(StringView str);
	option<TokenType> lex_symbol_1(u8 c);
	option<TokenType> lex_symbol_2(u8 c, TokenType type);
	option<TokenType> lex_symbol_3(u8 c, TokenType type);
	void skip_whitespace();
	void consume();
	option<u8> peek(u32 offset = 0);
};

#endif
