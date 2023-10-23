#ifndef TOKENIZER_H
#define TOKENIZER_H

#include "token.h"

struct Tokenizer;
enum class LexemeType;

void tokenizer_init();
Tokenizer tokenizer_create(StringView source, StringStorage* strings);
void tokenizer_tokenize(Tokenizer* tokenizer, Token* tokens);

static void tokenizer_skip_whitespace_comments(Tokenizer* tokenizer);
static option<u8> peek_character(Tokenizer* tokenizer, u32 offset);
static void consume_character(Tokenizer* tokenizer);

const u64 TOKENIZER_BUFFER_SIZE = 256;
const u64 TOKENIZER_LOOKAHEAD = 4;

struct Tokenizer
{
	StringView source;
	StringStorage* strings;
	u32 cursor = 0;
	u32 line_id = 1;
	u32 line_cursor = 0;
};

enum class LexemeType
{
	IDENT,
	NUMBER,
	STRING,
	SYMBOL,
	ERROR
};

#endif
