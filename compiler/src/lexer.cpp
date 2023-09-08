#include "lexer.h"

#include "common.h"
#include "error.h"
#include "token.h"

bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
bool is_number(u8 c) { return c >= '0' && c <= '9'; }
bool is_ident(u8 c)  { return is_letter(c) || (c == '_') || is_number(c); }
bool is_whitespace(u8 c) { return c == ' ' || c == '\t'; }
bool is_line_break(u8 c) { return c == '\r' || c == '\n'; }

bool is_first_of_ident(u8 c)  { return is_letter(c) || (c == '_'); }
bool is_first_of_number(u8 c) { return is_number(c); }
bool is_first_of_string(u8 c) { return c == '"'; }
bool is_first_of_symbol(u8 c) { return token_get_1_symbol_token_type(c) != TOKEN_ERROR; }

bool Lexer::set_input_from_file(const char* file_path)
{
	input_cursor = 0;
	return os_file_read_all(file_path, &input);
}

LineInfo Lexer::get_next_line()
{
	u64 count = input.count;
	u64 i = input_cursor;

	LineInfo line = { i, i, 0 };

	if (i >= count) 
	{
		line.is_valid = false;
		return line;
	}

	while (i < count && is_whitespace(input.data[i]))
	{
		line.leading_spaces += 1;
		i++;
	}

	line.end_cursor = i;

	while (i < count && !is_line_break(input.data[i]))
	{
		line.end_cursor = i;
		line.is_empty = false;
		i++;
	}

	if (i < count && input.data[i] == '\r') i++;
	if (i < count && input.data[i] == '\n') i++;
	input_cursor = i;

	line.start_cursor += line.leading_spaces;

	return line;
}

LexemeType Lexer::get_lexeme_type(u8 c)
{
	if (is_first_of_ident(c))  return LEXEME_IDENT;
	if (is_first_of_number(c)) return LEXEME_NUMBER;
	if (is_first_of_string(c)) return LEXEME_STRING;
	if (is_first_of_symbol(c)) return LEXEME_SYMBOL;
	return LEXEME_ERROR;
}

std::vector<Token> Lexer::tokenize()
{
	std::vector<Token> tokens;
	//@Performance testing pre allocation of vector
	tokens.reserve(770000);

	LineInfo line = {};
	u32 current_line_number = 0;

	LexemeType lexeme_types[128] = {};
	for (u8 i = 0; i < 128; i++)
	{
		lexeme_types[i] = get_lexeme_type(i);
	}

	while (line.is_valid)
	{
		line = get_next_line();
		current_line_number += 1;
		if (line.is_empty) continue;

		for (u64 i = line.start_cursor; i <= line.end_cursor; )
		{
			u8 fc = input.data[i];

			if (is_whitespace(fc))
			{
				i++;
				continue;
			}

			LexemeType type = fc < 128 ? lexeme_types[fc] : LEXEME_ERROR; 
			u64 lexeme_start = i;
			u64 lexeme_end = i + 1;

			Token token = {};
			token.l0 = current_line_number;
			token.c0 = (u32)(1 + i - (line.start_cursor - line.leading_spaces));

			switch (type)
			{
				case LEXEME_IDENT:
				{
					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						if (!is_ident(c)) break;
						lexeme_end += 1;
					}

					token.type = TOKEN_IDENT;
					token.string_value.data = input.data + lexeme_start;
					token.string_value.count = lexeme_end - lexeme_start;

					TokenType keyword = token_get_keyword_token_type(token.string_value);
					if (keyword != TOKEN_ERROR) token.type = keyword;

					i = lexeme_end;
					lexeme_end -= 1;
				} break;
				case LEXEME_NUMBER:
				{
					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						if (!is_number(c)) break;
						lexeme_end += 1;
					}

					token.type = TOKEN_NUMBER;

					i = lexeme_end;
					lexeme_end -= 1;
				} break;
				case LEXEME_STRING:
				{
					bool terminated = false;

					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						lexeme_end += 1;
						if (c == '"') { terminated = true; break; }
					}

					token.type = TOKEN_STRING;
					token.string_value.data = input.data + lexeme_start;
					token.string_value.count = lexeme_end - lexeme_start;

					i = lexeme_end;
					lexeme_end -= 1;

					if (!terminated)
					{
						//error_report(LEXER_ERROR_STRING_NOT_TERMINATED, token);
						token.type = TOKEN_ERROR;
					}
				} break;
				case LEXEME_SYMBOL:
				{
					token.type = token_get_1_symbol_token_type(fc);

					if (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];

						TokenType symbol = token_get_2_symbol_token_type(fc, c);
						if (symbol != TOKEN_ERROR) 
						{
							token.type = symbol;
							lexeme_end += 1;
						}
					}

					i = lexeme_end;
					lexeme_end -= 1;
				} break;
				case LEXEME_ERROR:
				{
					i++;
					//error_report(LEXER_ERROR_INVALID_CHARACTER, token); @Error handling disabled single char errors
				} break;
			}

			if (token.type != TOKEN_ERROR)
				tokens.emplace_back(token);
		}
	}

	Token token_end = {};
	token_end.type = TOKEN_INPUT_END;
	tokens.emplace_back(token_end);

	return tokens;
}

void Lexer::print_debug_metrics(const std::vector<Token>& tokens)
{
	u64 ident_count = 0;
	u64 number_count = 0;
	u64 string_count = 0;
	u64 keyword_count = 0;
	u64 symbol_count = 0;

	for (const Token& token : tokens)
	{
		if (token.type == TOKEN_IDENT) ident_count += 1;
		else if (token.type == TOKEN_NUMBER) number_count += 1;
		else if (token.type == TOKEN_STRING) string_count += 1;
		else if (token.type >= TOKEN_KEYWORD_STRUCT && token.type <= TOKEN_KEYWORD_CONTINUE) keyword_count += 1;
		else symbol_count += 1;
	}

	/*
	Lexer time(ms) : 49.000000
	Lexer : TokenCount :     766174
	Lexer : TokenTypesSum :  766174

	Lexer : [IdentCount] :   347752
	Lexer : [NumberCount] :  2878
	Lexer : [StringCount] :  5858
	Lexer : [KeywordCount] : 19701
	Lexer : [SymbolCount] :  389985

	Lexer : MemoryRequired(Mb) : 23.381775
	Lexer : MemoryUsed(Mb) : 32.039459

	//~27-28 ms memory preallocation
	// 25.7 ms lexeme lookup, keyword.count > 8
	*/

	printf("Lexer: TokenCount:          %llu \n", tokens.size());
	printf("Lexer: TokenTypesSum:       %llu \n", ident_count + number_count + string_count + keyword_count + symbol_count);
	printf("Lexer: [IdentCount]:        %llu \n", ident_count);
	printf("Lexer: [NumberCount]:       %llu \n", number_count);
	printf("Lexer: [StringCount]:       %llu \n", string_count);
	printf("Lexer: [KeywordCount]:      %llu \n", keyword_count);
	printf("Lexer: [SymbolCount]:       %llu \n", symbol_count);
	printf("Lexer: MemoryRequired (Mb): %f \n", double(sizeof(Token) * tokens.size()) / (1024.0 * 1024.0));
	printf("Lexer: MemoryUsed     (Mb): %f \n\n", double(sizeof(Token) * tokens.capacity()) / (1024.0 * 1024.0));
}
