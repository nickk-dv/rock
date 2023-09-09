#include "lexer.h"

#include "common.h"
#include "error.h"
#include "token.h"

bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
bool is_number(u8 c) { return c >= '0' && c <= '9'; }
bool is_ident(u8 c)  { return is_letter(c) || (c == '_') || is_number(c); }
bool is_whitespace(u8 c) { return c == ' ' || c == '\t'; }
bool is_line_break(u8 c) { return c == '\r' || c == '\n'; }

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

std::vector<Token> Lexer::tokenize()
{
	std::vector<Token> tokens;
	//@Performance testing pre allocation of vector
	tokens.reserve(770000);

	TokenType c_to_sym[128] = {};
	for (u8 i = 0; i < 128; i++)
	{
		c_to_sym[i] = TOKEN_ERROR;
	}

	c_to_sym['.'] = TOKEN_DOT;
	c_to_sym[','] = TOKEN_COMA;
	c_to_sym[':'] = TOKEN_COLON;
	c_to_sym[';'] = TOKEN_SEMICOLON;
	c_to_sym['{'] = TOKEN_BLOCK_START;
	c_to_sym['}'] = TOKEN_BLOCK_END;
	c_to_sym['['] = TOKEN_BRACKET_START;
	c_to_sym[']'] = TOKEN_BRACKET_END;
	c_to_sym['('] = TOKEN_PAREN_START;
	c_to_sym[')'] = TOKEN_PAREN_END;
	c_to_sym['='] = TOKEN_ASSIGN;
	c_to_sym['+'] = TOKEN_PLUS;
	c_to_sym['-'] = TOKEN_MINUS;
	c_to_sym['*'] = TOKEN_TIMES;
	c_to_sym['/'] = TOKEN_DIV;
	c_to_sym['%'] = TOKEN_MOD;
	c_to_sym['&'] = TOKEN_BITWISE_AND;
	c_to_sym['|'] = TOKEN_BITWISE_OR;
	c_to_sym['^'] = TOKEN_BITWISE_XOR;
	c_to_sym['<'] = TOKEN_LESS;
	c_to_sym['>'] = TOKEN_GREATER;
	c_to_sym['!'] = TOKEN_LOGIC_NOT;
	c_to_sym['~'] = TOKEN_BITWISE_NOT;

	enum LexemeType
	{
		LEXEME_IDENT,
		LEXEME_NUMBER,
		LEXEME_STRING,
		LEXEME_SYMBOL,
		LEXEME_ERROR
	};

	LexemeType lexeme_types[128] = {};
	for (u8 c = 0; c < 128; c++)
	{
		if (is_letter(c) || (c == '_')) lexeme_types[c] = LEXEME_IDENT;
		else if (c_to_sym[c] != TOKEN_ERROR) lexeme_types[c] = LEXEME_SYMBOL;
		else if (is_number(c)) lexeme_types[c] = LEXEME_NUMBER;
		else if (c == '"') lexeme_types[c] = LEXEME_STRING;
		else lexeme_types[c] = LEXEME_ERROR;
	}

	LineInfo line = {};
	u32 current_line_number = 0;

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
					u64 integer = fc - '0';

					while (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];
						if (!is_number(c)) break;
						lexeme_end += 1;

						integer *= 10;
						integer += c - '0';
					}

					token.type = TOKEN_NUMBER;
					token.integer_value = integer;

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
					token.type = c_to_sym[fc];

					if (lexeme_end <= line.end_cursor)
					{
						u8 c = input.data[lexeme_end];

						constexpr u32 equal_composable_symbol_token_offset = 12;
						constexpr u32 double_composable_symbol_token_offset = 18;
						
						u32 sym2 = TOKEN_ERROR;
						if (c == '=' && token.type >= TOKEN_ASSIGN && token.type <= TOKEN_LOGIC_NOT) sym2 = token.type + equal_composable_symbol_token_offset;
						else if ((c == fc) && (c == '&' || c == '|' || c == '<' || c == '>'))  sym2 = token.type + double_composable_symbol_token_offset;
						else if (c == ':' && fc == ':') sym2 = TOKEN_DOUBLE_COLON;

						if (sym2 != TOKEN_ERROR)
						{
							token.type = (TokenType)sym2;
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
	Lexer: TokenCount:          768829
	Lexer: TokenTypesSum:       768829
	Lexer: [IdentCount]:        338633
	Lexer: [NumberCount]:       2878
	Lexer: [StringCount]:       5858
	Lexer: [KeywordCount]:      28820
	Lexer: [SymbolCount]:       392640
	Lexer: MemoryRequired (Mb): 23.462799
	Lexer: MemoryUsed     (Mb): 23.498535

	//~49-50 ms original timing
	//~27-28 ms memory preallocation
	// 25.7 ms lexeme lookup, keyword.count > 8
	// 25.4 ms 2 symbol hack
	// 22.3 ms 2 symbol relational token derive
	// 21.5~22 ms -> removed
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
