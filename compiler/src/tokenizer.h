#ifndef TOKENIZER_H
#define TOKENIZER_H

#include "common.h"
#include "token.h"

enum LexemeType;
struct Tokenizer;

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
public:
	bool set_input_from_file(const char* file_path);
	void tokenize_buffer();

	u32 peek_index = 0;
	static const u64 TOKENIZER_BUFFER_SIZE = 256;
	static const u64 TOKENIZER_LOOKAHEAD = 2;
	Token tokens[TOKENIZER_BUFFER_SIZE];

private:
	void skip_whitespace();
	void skip_whitespace_comments();
	std::optional<u8> peek(u32 offset = 0);
	void consume();
	bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
	bool is_number(u8 c) { return c >= '0' && c <= '9'; }
	bool is_ident(u8 c) { return is_letter(c) || (c == '_') || is_number(c); }
	bool is_whitespace(u8 c) { return c == ' ' || c == '\t' || c == '\r' || c == '\n'; }

	String input;
	u64 input_cursor = 0;
	u64 line_start_cursor = 0;
	u32 line_id = 1;
	TokenType c_to_sym[128];
	LexemeType lexeme_types[128];
	StringStorage string_storage;
};

#endif
