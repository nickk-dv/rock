#ifndef TOKENIZER_H
#define TOKENIZER_H

#include "common.h"
#include "token.h"

struct Tokenizer;
enum class LexemeType;

void tokenizer_init();
bool tokenizer_set_input(Tokenizer* tokenizer, const char* filepath);
void tokenizer_tokenize(Tokenizer* tokenizer, Token* tokens);

static void tokenizer_skip_whitespace_comments(Tokenizer* tokenizer);
static option<u8> peek_character(Tokenizer* tokenizer, u32 offset);
static void consume_character(Tokenizer* tokenizer);

const u64 TOKENIZER_BUFFER_SIZE = 256;
const u64 TOKENIZER_LOOKAHEAD = 4;

struct Tokenizer
{
	String input;
	u64 input_cursor;
	u32 line_id;
	u64 line_cursor;
	StringStorage strings;
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
