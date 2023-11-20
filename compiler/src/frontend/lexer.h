#ifndef LEXER_H
#define LEXER_H

#include "token.h"

struct Lexer;
enum class Lexeme;

Lexer lex_init(StringView source, StringStorage* strings, std::vector<Span>* line_spans);
void lex_token_buffer(Lexer* lexer, Token* tokens);

static option<u8> lex_peek(Lexer* lexer, u32 offset = 0);
static void lex_consume(Lexer* lexer);
static void lex_skip_whitespace(Lexer* lexer);
static Lexeme lex_lexeme(u8 c);
static Token lex_token(Lexer* lexer);
static Token lex_char(Lexer* lexer);
static Token lex_string(Lexer* lexer);
static Token lex_number(Lexer* lexer);
static Token lex_ident(Lexer* lexer);
static Token lex_symbol(Lexer* lexer);
static TokenType lex_ident_keyword(StringView str);
static option<TokenType> lex_symbol_1(u8 c);
static option<TokenType> lex_symbol_2(u8 c, TokenType type);
static option<TokenType> lex_symbol_3(u8 c, TokenType type);

const u64 TOKEN_BUFFER_SIZE = 256;
const u64 TOKEN_LOOKAHEAD = 4;

struct Lexer
{
	u32 cursor;
	StringView source;
	StringStorage* strings;
	std::vector<Span>* line_spans;
};

enum class Lexeme
{
	CHAR,
	STRING,
	NUMBER,
	IDENT,
	SYMBOL,
};

#endif
