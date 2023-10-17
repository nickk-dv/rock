#ifndef TOKENIZER_H
#define TOKENIZER_H

#include "common.h"
#include "token.h"

enum LexemeType;
struct Tokenizer;

void tokenizer_init();
bool tokenizer_set_input(Tokenizer* tokenizer, const char* filepath);
void tokenizer_tokenize(Tokenizer* tokenizer, Token* tokens);

static void tokenizer_skip_whitespace_comments(Tokenizer* tokenizer);
static std::optional<u8> peek_character(Tokenizer* tokenizer, u32 offset);
static void consume_character(Tokenizer* tokenizer);

const u64 TOKENIZER_BUFFER_SIZE = 256;
const u64 TOKENIZER_LOOKAHEAD = 4;

enum LexemeType
{
	LEXEME_IDENT,
	LEXEME_NUMBER,
	LEXEME_STRING,
	LEXEME_SYMBOL,
	LEXEME_ERROR
};

struct Tokenizer
{
	String input;
	u64 input_cursor;
	u32 line_id;
	u64 line_cursor;
	StringStorage strings;
};

#endif
