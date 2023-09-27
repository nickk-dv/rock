#include "tokenizer.h"

bool Tokenizer::set_input_from_file(const char* filepath)
{
	for (u8 i = 0; i < 128; i++)
	{
		c_to_sym[i] = TOKEN_ERROR;
	}

	c_to_sym['.'] = TOKEN_DOT;
	c_to_sym[','] = TOKEN_COMMA;
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

	for (u8 c = 0; c < 128; c++)
	{
		if (is_letter(c) || (c == '_')) lexeme_types[c] = LEXEME_IDENT;
		else if (c_to_sym[c] != TOKEN_ERROR) lexeme_types[c] = LEXEME_SYMBOL;
		else if (is_number(c)) lexeme_types[c] = LEXEME_NUMBER;
		else if (c == '"') lexeme_types[c] = LEXEME_STRING;
		else lexeme_types[c] = LEXEME_ERROR;
	}

	input_cursor = 0;
	string_storage.init();
	return os_file_read_all(filepath, &input);
}

void Tokenizer::tokenize_buffer()
{
	u32 copy_count = input_cursor == 0 ? 0 : TOKENIZER_LOOKAHEAD;

	for (u32 k = 0; k < copy_count; k++)
	{
		tokens[k] = tokens[TOKENIZER_BUFFER_SIZE - TOKENIZER_LOOKAHEAD + k];
	}

	for (u32 k = copy_count; k < TOKENIZER_BUFFER_SIZE; k++)
	{
		skip_whitespace_comments();

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
		u64 lexeme_start = input_cursor;
		consume();

		Token token = {};
		token.l0 = line_id;
		token.c0 = u32(input_cursor - line_start_cursor) - 1;

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
				token.string_value.data = input.data + lexeme_start;
				token.string_value.count = input_cursor - lexeme_start;

				TokenType keyword = token_str_to_keyword(token.string_value);
				if (keyword != TOKEN_ERROR) token.type = keyword;

				if (keyword == TOKEN_KEYWORD_TRUE) 
				{ token.type = TOKEN_BOOL_LITERAL; token.bool_value = true; }
				else if (keyword == TOKEN_KEYWORD_FALSE)
				{ token.type = TOKEN_BOOL_LITERAL; token.bool_value = false; }
			} break;
			case LEXEME_NUMBER:
			{
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
			} break;
			case LEXEME_STRING:
			{
				bool terminated = false;
				string_storage.start_str();

				while (peek().has_value())
				{
					u8 c = peek().value();
					consume();
					if (c == '"') { terminated = true; break; }
					else if (c == '\n') break;
					else
					{
						string_storage.put_char(c);
					}
				}

				token.type = TOKEN_STRING_LITERAL;
				token.string_value.data = (u8*)string_storage.end_str();
				if (!terminated) token.type = TOKEN_ERROR;
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
						if (peek(1).has_value() && peek(1).value() == '=')
						{
							sym2 += bitshift_to_bitshift_equals_offset;
							consume();
						}
					}
					else if (c == ':' && fc == ':') sym2 = TOKEN_DOUBLE_COLON;

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

void Tokenizer::skip_whitespace()
{
	while (peek().has_value())
	{
		u8 c = peek().value();
		if (!is_whitespace(c)) break;
		if (c == '\n')
		{
			line_id += 1;
			line_start_cursor = input_cursor;
		}
		consume();
	}
}

void Tokenizer::skip_whitespace_comments()
{
	while (peek().has_value())
	{
		u8 c = peek().value();
		if (is_whitespace(c))
		{
			if (c == '\n')
			{
				line_id += 1;
				line_start_cursor = input_cursor;
			}
			consume();
		}
		else if (c == '/' && peek(1).has_value() && peek(1).value() == '/')
		{
			consume();
			consume();
			while (peek().has_value() && peek().value() != '\n') consume();
		}
		else break;
	}
}

std::optional<u8> Tokenizer::peek(u32 offset)
{
	if (input_cursor + offset < input.count)
		return input.data[input_cursor + offset];
	return {};
}

void Tokenizer::consume()
{
	input_cursor += 1;
}
