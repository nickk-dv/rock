
enum TokenType
{
	TOKEN_IDENT,                 // name
	TOKEN_NUMBER,                // 10
	TOKEN_STRING,                // "string"
	TOKEN_BOOL_LITERAL,          // true false

	TOKEN_KEYWORD_STRUCT,        // struct
	TOKEN_KEYWORD_ENUM,          // enum
	TOKEN_KEYWORD_FN,            // fn
	TOKEN_KEYWORD_IF,            // if
	TOKEN_KEYWORD_ELSE,          // else
	TOKEN_KEYWORD_TRUE,          // true
	TOKEN_KEYWORD_FALSE,         // false
	TOKEN_KEYWORD_FOR,           // for
	TOKEN_KEYWORD_BREAK,         // break
	TOKEN_KEYWORD_RETURN,        // return
	TOKEN_KEYWORD_CONTINUE,      // continue

	TOKEN_TYPE_I8,               // i8
	TOKEN_TYPE_U8,               // u8
	TOKEN_TYPE_I16,              // i16
	TOKEN_TYPE_U16,              // u16
	TOKEN_TYPE_I32,              // i32
	TOKEN_TYPE_U32,              // u32
	TOKEN_TYPE_I64,              // i64
	TOKEN_TYPE_U64,              // u64
	TOKEN_TYPE_F32,              // f32
	TOKEN_TYPE_F64,              // f64
	TOKEN_TYPE_BOOL,             // bool
	TOKEN_TYPE_STRING,           // string

	TOKEN_DOT,                   // .
	TOKEN_QUOTE,                 // '
	TOKEN_COMMA,                 // ,
	TOKEN_COLON,                 // :
	TOKEN_SEMICOLON,             // ;
	TOKEN_DOUBLE_COLON,          // ::
	TOKEN_BLOCK_START,           // {
	TOKEN_BLOCK_END,             // }
	TOKEN_BRACKET_START,         // [
	TOKEN_BRACKET_END,           // ]
	TOKEN_PAREN_START,           // (
	TOKEN_PAREN_END,             // )
	TOKEN_AT,                    // @
	TOKEN_HASH,                  // #
	TOKEN_QUESTION,              // ?

	TOKEN_ASSIGN,                // =
	TOKEN_PLUS,                  // +
	TOKEN_MINUS,                 // -
	TOKEN_TIMES,                 // *
	TOKEN_DIV,                   // /
	TOKEN_MOD,                   // %
	TOKEN_BITWISE_AND,           // &
	TOKEN_BITWISE_OR,            // |
	TOKEN_BITWISE_XOR,           // ^
	TOKEN_LESS,                  // <
	TOKEN_GREATER,               // >
	TOKEN_LOGIC_NOT,             // !
	TOKEN_IS_EQUALS,             // ==
	TOKEN_PLUS_EQUALS,           // +=
	TOKEN_MINUS_EQUALS,          // -=
	TOKEN_TIMES_EQUALS,          // *=
	TOKEN_DIV_EQUALS,            // /=
	TOKEN_MOD_EQUALS,            // %=
	TOKEN_BITWISE_AND_EQUALS,    // &=
	TOKEN_BITWISE_OR_EQUALS,     // |=
	TOKEN_BITWISE_XOR_EQUALS,    // ^=
	TOKEN_LESS_EQUALS,           // <=
	TOKEN_GREATER_EQUALS,        // >=
	TOKEN_NOT_EQUALS,            // !=
	TOKEN_LOGIC_AND,             // &&
	TOKEN_LOGIC_OR,              // ||
	TOKEN_BITWISE_NOT,           // ~
	TOKEN_BITSHIFT_LEFT,         // <<
	TOKEN_BITSHIFT_RIGHT,        // >>
	TOKEN_BITSHIFT_LEFT_EQUALS,  // <<=
	TOKEN_BITSHIFT_RIGHT_EQUALS, // >>=

	TOKEN_ERROR,
	TOKEN_EOF,
};

struct Token
{
	Token() {};

	TokenType type = TOKEN_ERROR;
	u32 l0 = 0;
	u32 c0 = 0;

	union
	{
		bool bool_value;
		double float64_value;
		u64 integer_value;
		StringView string_value;
	};
};

//@Todo comment support
// skip '//' until end of line
// skip '/* */' until all nested openings are closed
struct Tokenizer
{
	bool set_input_from_file(const char* file_path);
	void tokenize_buffer();

	void skip_whitespace();
	void skip_whitespace_comments();
	TokenType get_keyword_token_type(const StringView& str);
	bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
	bool is_number(u8 c) { return c >= '0' && c <= '9'; }
	bool is_ident(u8 c) { return is_letter(c) || (c == '_') || is_number(c); }
	bool is_whitespace(u8 c) { return c == ' ' || c == '\t' || c == '\r' || c == '\n'; }

	String input;
	u64 input_cursor = 0;
	u32 line_id = 1;
	u64 line_start_cursor = 0;
	u32 peek_index = 0;
	static const u64 TOKENIZER_BUFFER_SIZE = 256;
	static const u64 TOKENIZER_LOOKAHEAD = 1;
	Token tokens[TOKENIZER_BUFFER_SIZE];

	std::optional<u8> peek_c(u32 offset = 0)
	{
		if (input_cursor + offset < input.count) return input.data[input_cursor + offset];
		return {};
	}

	void consume_c()
	{
		input_cursor += 1;
	}

	Token peek(u32 offset = 0)
	{
		return tokens[peek_index + offset];
	}

	std::optional<Token> try_consume(TokenType token_type)
	{
		Token token = peek();
		if (token.type == token_type)
		{
			consume();
			return token;
		}
		return {};
	}

	Token consume_get()
	{
		Token token = peek();
		consume();
		return token;
	}

	void consume()
	{
		peek_index += 1;
		if (peek_index >= (TOKENIZER_BUFFER_SIZE - TOKENIZER_LOOKAHEAD))
		{
			peek_index = 0; tokenize_buffer();
		}
	}
};

bool Tokenizer::set_input_from_file(const char* file_path)
{
	input_cursor = 0;
	return os_file_read_all(file_path, &input);
}

void Tokenizer::skip_whitespace()
{
	while (peek_c().has_value())
	{
		u8 c = peek_c().value();
		if (!is_whitespace(c)) break;
		if (c == '\n')
		{
			line_id += 1;
			line_start_cursor = input_cursor;
		}
		consume_c();
	}
}

void Tokenizer::skip_whitespace_comments()
{
	while (peek_c().has_value())
	{
		u8 c = peek_c().value();
		if (is_whitespace(c))
		{
			if (c == '\n')
			{
				line_id += 1;
				line_start_cursor = input_cursor;
			}
			consume_c();
		}
		else if (c == '/' && peek_c(1).has_value() && peek_c(1).value() == '/')
		{
			consume_c();
			consume_c();
			while (peek_c().has_value() && peek_c().value() != '\n') consume_c();
		}
		else break;
	}
}

void Tokenizer::tokenize_buffer()
{
	TokenType c_to_sym[128] = {};
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

	u32 copy_offset = input_cursor == 0 ? 0 : TOKENIZER_LOOKAHEAD;

	for (u32 k = 0; k < copy_offset; k++)
	{
		tokens[k] = tokens[TOKENIZER_BUFFER_SIZE - TOKENIZER_LOOKAHEAD + k];
	}

	for (u32 k = copy_offset; k < TOKENIZER_BUFFER_SIZE; k++)
	{
		skip_whitespace_comments();

		if (!peek_c().has_value())
		{
			for (u32 i = k; i < TOKENIZER_BUFFER_SIZE; i++)
			{
				tokens[i].type = TOKEN_EOF;
			}
			return;
		}

		u8 fc = peek_c().value();
		LexemeType type = fc < 128 ? lexeme_types[fc] : LEXEME_ERROR;
		u64 lexeme_start = input_cursor;
		consume_c();

		Token token = {};
		token.l0 = line_id;
		token.c0 = u32(input_cursor - line_start_cursor);

		switch (type)
		{
			case LEXEME_IDENT:
			{
				while (peek_c().has_value())
				{
					if (!is_ident(peek_c().value())) break;
					consume_c();
				}

				token.type = TOKEN_IDENT;
				token.string_value.data = input.data + lexeme_start;
				token.string_value.count = input_cursor - lexeme_start;

				TokenType keyword = get_keyword_token_type(token.string_value);
				if (keyword != TOKEN_ERROR) token.type = keyword;
			} break;
			case LEXEME_NUMBER:
			{
				u64 integer = fc - '0';

				while (peek_c().has_value())
				{
					u8 c = peek_c().value();
					if (!is_number(c)) break;
					consume_c();
					integer *= 10;
					integer += c - '0';
				}

				token.type = TOKEN_NUMBER;
				token.integer_value = integer;
			} break;
			case LEXEME_STRING:
			{
				bool terminated = false;
				while (peek_c().has_value())
				{
					u8 c = peek_c().value();
					consume_c();
					if (c == '"') { terminated = true; break; }
					else if (c == '\n') break;
				}

				token.type = TOKEN_STRING;
				token.string_value.data = input.data + lexeme_start;
				token.string_value.count = input_cursor - lexeme_start;

				if (!terminated)
				{
					token.type = TOKEN_ERROR;
				}
			} break;
			case LEXEME_SYMBOL:
			{
				token.type = c_to_sym[fc];

				if (peek_c().has_value())
				{
					u8 c = peek_c().value();

					constexpr u32 equal_composable_symbol_token_offset = 12;
					constexpr u32 double_composable_symbol_token_offset = 18;
					constexpr u32 bitshift_to_bitshift_equals_offset = 2;

					u32 sym2 = TOKEN_ERROR;
					if (c == '=' && token.type >= TOKEN_ASSIGN && token.type <= TOKEN_LOGIC_NOT) sym2 = token.type + equal_composable_symbol_token_offset;
					else if ((c == fc) && (c == '&' || c == '|' || c == '<' || c == '>'))
					{
						sym2 = token.type + double_composable_symbol_token_offset;
						if (peek_c(1).has_value() && peek_c(1).value() == '=')
						{
							sym2 += bitshift_to_bitshift_equals_offset;
							consume_c();
						}
					}
					else if (c == ':' && fc == ':') sym2 = TOKEN_DOUBLE_COLON;

					if (sym2 != TOKEN_ERROR)
					{
						token.type = (TokenType)sym2;
						consume_c();
					}
				}
			} break;
			default: break;
		}

		tokens[k] = token;
	}
}

static const std::unordered_map<u64, TokenType> keyword_hash_to_token_type =
{
	{ hash_ascii_9("struct"),   TOKEN_KEYWORD_STRUCT },
	{ hash_ascii_9("enum"),     TOKEN_KEYWORD_ENUM },
	{ hash_ascii_9("fn"),       TOKEN_KEYWORD_FN },
	{ hash_ascii_9("if"),       TOKEN_KEYWORD_IF },
	{ hash_ascii_9("else"),     TOKEN_KEYWORD_ELSE },
	{ hash_ascii_9("true"),     TOKEN_KEYWORD_TRUE },
	{ hash_ascii_9("false"),    TOKEN_KEYWORD_FALSE },
	{ hash_ascii_9("for"),      TOKEN_KEYWORD_FOR },
	{ hash_ascii_9("break"),    TOKEN_KEYWORD_BREAK },
	{ hash_ascii_9("return"),   TOKEN_KEYWORD_RETURN },
	{ hash_ascii_9("continue"), TOKEN_KEYWORD_CONTINUE },

	{ hash_ascii_9("i8"),       TOKEN_TYPE_I8 },
	{ hash_ascii_9("u8"),       TOKEN_TYPE_U8 },
	{ hash_ascii_9("i16"),      TOKEN_TYPE_I16 },
	{ hash_ascii_9("u16"),      TOKEN_TYPE_U16 },
	{ hash_ascii_9("i32"),      TOKEN_TYPE_I32 },
	{ hash_ascii_9("u32"),      TOKEN_TYPE_U32 },
	{ hash_ascii_9("i64"),      TOKEN_TYPE_I64 },
	{ hash_ascii_9("u64"),      TOKEN_TYPE_U64 },
	{ hash_ascii_9("f32"),      TOKEN_TYPE_F32 },
	{ hash_ascii_9("f64"),      TOKEN_TYPE_F64 },
	{ hash_ascii_9("bool"),     TOKEN_TYPE_BOOL },
	{ hash_ascii_9("string"),   TOKEN_TYPE_STRING },
};

//@Perf test the different keyword methods on bigger files like switching on char in a tree like search
TokenType Tokenizer::get_keyword_token_type(const StringView& str)
{
	if (str.count > 8 || str.count < 2) return TOKEN_ERROR;
	u64 hash = string_hash_ascii_9(str);
	bool is_keyword = keyword_hash_to_token_type.find(hash) != keyword_hash_to_token_type.end();
	return is_keyword ? keyword_hash_to_token_type.at(hash) : TOKEN_ERROR;
}
