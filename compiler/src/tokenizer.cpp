#include "tokenizer.h"

inline bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
inline bool is_number(u8 c) { return c >= '0' && c <= '9'; }
inline bool is_ident(u8 c) { return is_letter(c) || (c == '_') || is_number(c); }
inline bool is_whitespace(u8 c) { return c == ' ' || c == '\t' || c == '\r' || c == '\n'; }

#define peek() peek_character(tokenizer, 0)
#define peek_next(offset) peek_character(tokenizer, offset)
#define consume() consume_character(tokenizer)

TokenType c_to_sym[128];
LexemeType lexeme_types[128];

void tokenizer_init()
{
	for (u8 i = 0; i < 128; i++)
	{
		c_to_sym[i] = TOKEN_ERROR;
	}

	c_to_sym['.'] = TOKEN_DOT;
	c_to_sym[':'] = TOKEN_COLON;
	c_to_sym['\''] = TOKEN_QUOTE;
	c_to_sym[','] = TOKEN_COMMA;
	c_to_sym[';'] = TOKEN_SEMICOLON;
	c_to_sym['{'] = TOKEN_BLOCK_START;
	c_to_sym['}'] = TOKEN_BLOCK_END;
	c_to_sym['['] = TOKEN_BRACKET_START;
	c_to_sym[']'] = TOKEN_BRACKET_END;
	c_to_sym['('] = TOKEN_PAREN_START;
	c_to_sym[')'] = TOKEN_PAREN_END;
	c_to_sym['@'] = TOKEN_AT;
	c_to_sym['#'] = TOKEN_HASH;
	c_to_sym['?'] = TOKEN_QUESTION;
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

	for (u8 c = 0; c < 128; c++)
	{
		if (is_letter(c) || (c == '_')) lexeme_types[c] = LEXEME_IDENT;
		else if (c_to_sym[c] != TOKEN_ERROR) lexeme_types[c] = LEXEME_SYMBOL;
		else if (is_number(c)) lexeme_types[c] = LEXEME_NUMBER;
		else if (c == '"') lexeme_types[c] = LEXEME_STRING;
		else lexeme_types[c] = LEXEME_ERROR;
	}
}

bool tokenizer_set_input(Tokenizer* tokenizer, const char* filepath)
{
	tokenizer->input_cursor = 0;
	tokenizer->line_cursor = 0;
	tokenizer->line_id = 1;
	tokenizer->strings.init();
	return os_file_read_all(filepath, &tokenizer->input);
}

void tokenizer_tokenize(Tokenizer* tokenizer, Token* tokens)
{
	u32 copy_count = tokenizer->input_cursor == 0 ? 0 : TOKENIZER_LOOKAHEAD;

	for (u32 k = 0; k < copy_count; k++)
	{
		tokens[k] = tokens[TOKENIZER_BUFFER_SIZE - TOKENIZER_LOOKAHEAD + k];
	}

	for (u32 k = copy_count; k < TOKENIZER_BUFFER_SIZE; k++)
	{
		tokenizer_skip_whitespace_comments(tokenizer);

		if (!peek().has_value())
		{
			for (u32 i = k; i < TOKENIZER_BUFFER_SIZE; i++)
			{
				tokens[i].type = TOKEN_EOF;
			}
			return;
		}

		u8 fc = peek().value();
		LexemeType type = fc < 128 ? lexeme_types[fc] : LEXEME_ERROR;
		u64 lexeme_start = tokenizer->input_cursor;
		consume();

		Token token = {};
		token.l0 = tokenizer->line_id;
		token.c0 = u32(tokenizer->input_cursor - tokenizer->line_cursor) - 1;

		switch (type)
		{
			case LEXEME_IDENT:
			{
				while (peek().has_value())
				{
					if (!is_ident(peek().value())) break;
					consume();
				}

				token.type = TOKEN_IDENT;
				token.string_value.data = tokenizer->input.data + lexeme_start;
				token.string_value.count = tokenizer->input_cursor - lexeme_start;

				TokenType keyword = token_str_to_keyword(token.string_value);
				if (keyword != TOKEN_ERROR) token.type = keyword;

				if (keyword == TOKEN_KEYWORD_TRUE) 
				{ token.type = TOKEN_BOOL_LITERAL; token.bool_value = true; }
				else if (keyword == TOKEN_KEYWORD_FALSE)
				{ token.type = TOKEN_BOOL_LITERAL; token.bool_value = false; }
			} break;
			case LEXEME_NUMBER:
			{
				u32 offset = 0;
				bool is_float = false;
				while (peek_next(offset).has_value())
				{
					u8 c = peek_next(offset).value();
					if (!is_float && c == '.')
					{
						is_float = true;
					}
					else if (!is_number(c)) break;
					offset += 1;
				}

				if (is_float)
				{
					u64 expected_len = offset + 1;
					u8 last_c = tokenizer->input.data[tokenizer->input_cursor + expected_len];
					tokenizer->input.data[tokenizer->input_cursor + expected_len] = '\0';
					char* start = (char*)tokenizer->input.data + (tokenizer->input_cursor - 1);
					char* end = start + 1;
					f64 float64_value = strtod(start, &end); //@Later replace this with custom to avoid \0 hacks and ensure valid number grammar
					tokenizer->input.data[tokenizer->input_cursor + expected_len] = last_c;

					for (u32 i = 0; i < offset; i += 1)
					{
						consume();
					}

					if (end != start)
					{
						token.type = TOKEN_FLOAT_LITERAL;
						token.float64_value = float64_value;
					}
				}
				else
				{
					//@Todo catch u64 overflows
					u64 integer = fc - '0';

					while (peek().has_value())
					{
						u8 c = peek().value();
						if (!is_number(c)) break;
						consume();
						integer *= 10;
						integer += c - '0';
					}

					token.type = TOKEN_INTEGER_LITERAL;
					token.integer_value = integer;
				}
			} break;
			case LEXEME_STRING:
			{
				bool terminated = false;
				bool escapes_valid = true;
				tokenizer->strings.start_str();

				while (peek().has_value())
				{
					u8 c = peek().value();
					consume();
					
					if (c == '"') { terminated = true; break; }
					if (c == '\n') break;
					if (c == '\\')
					{
						u32 line = tokenizer->line_id;
						u32 col = u32(tokenizer->input_cursor - tokenizer->line_cursor) - 1;
						
						if (peek().has_value())
						{
							u8 next = peek().value();
							consume();

							switch (next)
							{
							case 'n': tokenizer->strings.put_char('\n'); break;
							case 'r': tokenizer->strings.put_char('\r'); break;
							case 't': tokenizer->strings.put_char('\t'); break;
							case '\"': tokenizer->strings.put_char('\"'); break;
							case '\\': tokenizer->strings.put_char('\\'); break;
							case '0': tokenizer->strings.put_char('\0'); break;
							default:
							{
								tokenizer->strings.put_char(next);
								escapes_valid = false;
								printf("Invalid escape character: \\%c at %lu:%lu\n", next, line, col);
								printf("Hint: if you meant to use backslash type: \\\\ \n\n");
							}
							}
						}
						else
						{
							escapes_valid = false;
							printf("Invalid escape character: \\ at %lu:%lu\n", line, col);
							printf("Hint: if you meant to use backslash type: \\\\ \n\n");
						}
					}
					else tokenizer->strings.put_char(c);
				}

				token.type = TOKEN_STRING_LITERAL;
				token.string_literal_value = tokenizer->strings.end_str();
				if (!terminated || !escapes_valid) token.type = TOKEN_ERROR;
			} break;
			case LEXEME_SYMBOL:
			{
				token.type = c_to_sym[fc];

				if (peek().has_value())
				{
					u8 c = peek().value();

					constexpr u32 equal_composable_symbol_token_offset = 12;
					constexpr u32 double_composable_symbol_token_offset = 18;
					constexpr u32 bitshift_to_bitshift_equals_offset = 2;

					u32 sym2 = TOKEN_ERROR;
					if (c == '=' && token.type >= TOKEN_ASSIGN && token.type <= TOKEN_LOGIC_NOT) sym2 = token.type + equal_composable_symbol_token_offset;
					else if ((c == fc) && (c == '&' || c == '|' || c == '<' || c == '>'))
					{
						sym2 = token.type + double_composable_symbol_token_offset;
						if (peek_next(1).has_value() && peek_next(1).value() == '=')
						{
							sym2 += bitshift_to_bitshift_equals_offset;
							consume();
						}
					}
					else if (c == fc)
					{
						if (c == ':') sym2 = TOKEN_DOUBLE_COLON;
						else if (c == '.') sym2 = TOKEN_DOUBLE_DOT;
					}

					if (sym2 != TOKEN_ERROR)
					{
						token.type = (TokenType)sym2;
						consume();
					}
				}
			} break;
			default: break;
		}

		tokens[k] = token;
	}
}

void tokenizer_skip_whitespace_comments(Tokenizer* tokenizer)
{
	while (peek().has_value())
	{
		u8 c = peek().value();
		if (is_whitespace(c))
		{
			if (c == '\n')
			{
				tokenizer->line_id += 1;
				tokenizer->line_cursor = tokenizer->input_cursor;
			}
			consume();
		}
		else if (c == '/' && peek_next(1).has_value() && peek_next(1).value() == '/')
		{
			consume();
			consume();
			while (peek().has_value() && peek().value() != '\n') consume();
		}
		else break;
	}
}

option<u8> peek_character(Tokenizer* tokenizer, u32 offset)
{
	if (tokenizer->input_cursor + offset < tokenizer->input.count)
		return tokenizer->input.data[tokenizer->input_cursor + offset];
	return {};
}

void consume_character(Tokenizer* tokenizer)
{
	tokenizer->input_cursor += 1;
}
