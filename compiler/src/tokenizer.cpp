
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
	TOKEN_COMMA,                 // ,
	TOKEN_COLON,                 // :
	TOKEN_SEMICOLON,             // ;
	TOKEN_BLOCK_START,           // {
	TOKEN_BLOCK_END,             // }
	TOKEN_BRACKET_START,         // [
	TOKEN_BRACKET_END,           // ]
	TOKEN_PAREN_START,           // (
	TOKEN_PAREN_END,             // )
	TOKEN_DOUBLE_COLON,          // ::

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

void debug_print_token_t(Token token, bool endl, bool location);

struct Tokenizer
{
	bool set_input_from_file(const char* file_path);

	TokenType get_keyword_token_type(const StringView& str);
	bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
	bool is_number(u8 c) { return c >= '0' && c <= '9'; }
	bool is_ident(u8 c) { return is_letter(c) || (c == '_') || is_number(c); }
	bool is_whitespace(u8 c) { return c == ' ' || c == '\t' || c == '\r' || c == '\n'; }

	String input;
	u64 input_cursor = 0;

	void skip_whitespace();
	void tokenize_buffer();

	u32 peek_index = 0;

	u32 line_id = 1;
	static const u64 TOKEN_BUFFER_SIZE = 8;
	static const u64 TOKEN_LOOKAHEAD = 2;
	Token tokens[TOKEN_BUFFER_SIZE];

	std::optional<Token> peek(u32 offset = 0) //@Always exists by design not optional
	{
		return tokens[peek_index + offset];
	}

	std::optional<Token> try_consume(TokenType token_type)
	{
		auto token = peek();
		if (token && token.value().type == token_type)
		{
			consume();
			return token;
		}
		return {};
	}

	Token consume_get()
	{
		Token token = tokens[peek_index];
		consume();
		return token;
	}

	void consume()
	{
		printf("consumed: "); debug_print_token_t(tokens[peek_index], true, false);
		peek_index += 1;
		if (peek_index >= (TOKEN_BUFFER_SIZE - TOKEN_LOOKAHEAD)) //correct
		{
			peek_index = 0;
			tokenize_buffer();
		}
	}
};

void debug_print_token_t(Token token, bool endl, bool location)
{
	if (location) printf("l: %lu c: %lu Token: ", token.l0, token.c0);

	if (token.type == TOKEN_IDENT || token.type == TOKEN_STRING)
	{
		for (u64 i = 0; i < token.string_value.count; i++)
			printf("%c", token.string_value.data[i]);
	}
	else if (token.type == TOKEN_NUMBER)
	{
		printf("%llu", token.integer_value);
		//@Incomplete need to lex f32 f64 and store numeric flags
	}
	else if (token.type == TOKEN_BOOL_LITERAL)
	{
		if (token.bool_value)
			printf("true");
		else printf("false");
	}
	else
	{
		switch (token.type)
		{
		case TOKEN_KEYWORD_STRUCT: printf("struct"); break;
		case TOKEN_KEYWORD_ENUM: printf("enum"); break;
		case TOKEN_KEYWORD_FN: printf("fn"); break;
		case TOKEN_KEYWORD_IF: printf("if"); break;
		case TOKEN_KEYWORD_ELSE: printf("else"); break;
		case TOKEN_KEYWORD_TRUE: printf("true"); break;
		case TOKEN_KEYWORD_FALSE: printf("false"); break;
		case TOKEN_KEYWORD_FOR: printf("for"); break;
		case TOKEN_KEYWORD_BREAK: printf("break"); break;
		case TOKEN_KEYWORD_RETURN: printf("return"); break;
		case TOKEN_KEYWORD_CONTINUE: printf("continue"); break;

		case TOKEN_TYPE_I8: printf("i8"); break;
		case TOKEN_TYPE_U8: printf("u8"); break;
		case TOKEN_TYPE_I16: printf("i16"); break;
		case TOKEN_TYPE_U16: printf("u16"); break;
		case TOKEN_TYPE_I32: printf("i32"); break;
		case TOKEN_TYPE_U32: printf("u32"); break;
		case TOKEN_TYPE_I64: printf("i64"); break;
		case TOKEN_TYPE_U64: printf("u64"); break;
		case TOKEN_TYPE_F32: printf("f32"); break;
		case TOKEN_TYPE_F64: printf("f64"); break;
		case TOKEN_TYPE_BOOL: printf("bool"); break;
		case TOKEN_TYPE_STRING: printf("string"); break;

		case TOKEN_DOT: printf("."); break;
		case TOKEN_COMMA: printf(","); break;
		case TOKEN_COLON: printf(":"); break;
		case TOKEN_SEMICOLON: printf(";"); break;
		case TOKEN_BLOCK_START: printf("{"); break;
		case TOKEN_BLOCK_END: printf("}"); break;
		case TOKEN_BRACKET_START: printf("["); break;
		case TOKEN_BRACKET_END: printf("]"); break;
		case TOKEN_PAREN_START: printf("("); break;
		case TOKEN_PAREN_END: printf(")"); break;
		case TOKEN_DOUBLE_COLON: printf("::"); break;

		case TOKEN_ASSIGN: printf("="); break;
		case TOKEN_PLUS: printf("+"); break;
		case TOKEN_MINUS: printf("-"); break;
		case TOKEN_TIMES: printf("*"); break;
		case TOKEN_DIV: printf("/"); break;
		case TOKEN_MOD: printf("%%"); break;
		case TOKEN_BITWISE_AND: printf("&"); break;
		case TOKEN_BITWISE_OR: printf("|"); break;
		case TOKEN_BITWISE_XOR: printf("^"); break;
		case TOKEN_LESS: printf("<"); break;
		case TOKEN_GREATER: printf(">"); break;
		case TOKEN_LOGIC_NOT: printf("!"); break;
		case TOKEN_IS_EQUALS: printf("=="); break;
		case TOKEN_PLUS_EQUALS: printf("+="); break;
		case TOKEN_MINUS_EQUALS: printf("-="); break;
		case TOKEN_TIMES_EQUALS: printf("*="); break;
		case TOKEN_DIV_EQUALS: printf("/="); break;
		case TOKEN_MOD_EQUALS: printf("%%="); break;
		case TOKEN_BITWISE_AND_EQUALS: printf("&="); break;
		case TOKEN_BITWISE_OR_EQUALS: printf("|="); break;
		case TOKEN_BITWISE_XOR_EQUALS: printf("^="); break;
		case TOKEN_LESS_EQUALS: printf("<="); break;
		case TOKEN_GREATER_EQUALS: printf(">="); break;
		case TOKEN_NOT_EQUALS: printf("!="); break;
		case TOKEN_LOGIC_AND: printf("&&"); break;
		case TOKEN_LOGIC_OR: printf("||"); break;
		case TOKEN_BITWISE_NOT: printf("~"); break;
		case TOKEN_BITSHIFT_LEFT: printf("<<"); break;
		case TOKEN_BITSHIFT_RIGHT: printf(">>"); break;

		case TOKEN_ERROR: printf("ERROR"); break;
		case TOKEN_EOF: printf("EOF"); break;

		default: printf("[UNKNOWN TOKEN]"); break;
		}
	}
	printf(" ");
	if (endl) printf("\n");
}

bool Tokenizer::set_input_from_file(const char* file_path)
{
	input_cursor = 0;
	return os_file_read_all(file_path, &input);
}

void Tokenizer::skip_whitespace()
{
	while (input_cursor < input.count)
	{
		u8 c = input.data[input_cursor];
		if (!is_whitespace(c)) break;
		if (c == '\n') line_id += 1;
		input_cursor += 1;
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

	u32 copy_offset = input_cursor == 0 ? 0 : TOKEN_LOOKAHEAD;

	if (copy_offset != 0)
	printf("\n [COPY LAST PASS] \n");
	for (u32 k = 0; k < copy_offset; k++)
	{
		tokens[k] = tokens[TOKEN_BUFFER_SIZE - TOKEN_LOOKAHEAD + k];
		debug_print_token_t(tokens[k], true, false);
		printf("doing copy from: %lu to %lu \n", (TOKEN_BUFFER_SIZE - TOKEN_LOOKAHEAD + k), k);
	}

	for (u32 k = copy_offset; k < TOKEN_BUFFER_SIZE; k++)
	{
		skip_whitespace();

		if (input_cursor >= input.count)
		{
			for (u32 i = k; i < TOKEN_BUFFER_SIZE; i++)
			{
				tokens[i].type = TOKEN_EOF;
			}
			break;
		}

		//not imporant, tokeninzing the next token
		u8 fc = input.data[input_cursor];
		LexemeType type = fc < 128 ? lexeme_types[fc] : LEXEME_ERROR;
		u64 lexeme_start = input_cursor;
		u64 lexeme_end = input_cursor + 1;

		Token token = {};
		token.l0 = line_id;
		token.c0 = 1; //@Ignore it for now //(u32)(1 + input_cursor - (line.start_cursor - line.leading_spaces));

		switch (type)
		{
			case LEXEME_IDENT:
			{
				while (lexeme_end < input.count)
				{
					u8 c = input.data[lexeme_end];
					if (!is_ident(c)) break;
					lexeme_end += 1;
				}

				token.type = TOKEN_IDENT;
				token.string_value.data = input.data + lexeme_start;
				token.string_value.count = lexeme_end - lexeme_start;

				TokenType keyword = get_keyword_token_type(token.string_value);
				if (keyword != TOKEN_ERROR) token.type = keyword;

				input_cursor = lexeme_end;
				lexeme_end -= 1;
			} break;
			case LEXEME_NUMBER:
			{
				u64 integer = fc - '0';

				while (lexeme_end < input.count)
				{
					u8 c = input.data[lexeme_end];
					if (!is_number(c)) break;
					lexeme_end += 1;

					integer *= 10;
					integer += c - '0';
				}

				token.type = TOKEN_NUMBER;
				token.integer_value = integer;

				input_cursor = lexeme_end;
				lexeme_end -= 1;
			} break;
			case LEXEME_STRING:
			{
				bool terminated = false;

				while (lexeme_end < input.count)
				{
					u8 c = input.data[lexeme_end];
					lexeme_end += 1;
					if (c == '"') { terminated = true; break; }
				}

				token.type = TOKEN_STRING;
				token.string_value.data = input.data + lexeme_start;
				token.string_value.count = lexeme_end - lexeme_start;

				input_cursor = lexeme_end;
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

				if (lexeme_end <= input.count)
				{
					u8 c = input.data[lexeme_end];

					constexpr u32 equal_composable_symbol_token_offset = 12;
					constexpr u32 double_composable_symbol_token_offset = 18;
					constexpr u32 bitshift_to_bitshift_equals_offset = 2;

					u32 sym2 = TOKEN_ERROR;
					if (c == '=' && token.type >= TOKEN_ASSIGN && token.type <= TOKEN_LOGIC_NOT) sym2 = token.type + equal_composable_symbol_token_offset;
					else if ((c == fc) && (c == '&' || c == '|' || c == '<' || c == '>'))
					{
						sym2 = token.type + double_composable_symbol_token_offset;
						if (lexeme_end + 1 <= input.count && input.data[lexeme_end + 1] == '=')
						{
							sym2 += bitshift_to_bitshift_equals_offset;
							lexeme_end += 1;
						}
					}
					else if (c == ':' && fc == ':') sym2 = TOKEN_DOUBLE_COLON;

					if (sym2 != TOKEN_ERROR)
					{
						token.type = (TokenType)sym2;
						lexeme_end += 1;
					}
				}

				input_cursor = lexeme_end;
				lexeme_end -= 1;
			} break;
			case LEXEME_ERROR:
			{
				input_cursor += 1;
				//error_report(LEXER_ERROR_INVALID_CHARACTER, token); @Error handling disabled single char errors
			} break;
		}

		tokens[k] = token;
	}

	printf("\n [TOKENIZATION PASS] \n");
	for (int i = 0; i < TOKEN_BUFFER_SIZE; i++)
	{
		printf("token buffer state: "); debug_print_token_t(tokens[i], true, false);
	}
}

//@Performance find a way to not use maps an use logic or table lookups instead
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

TokenType Tokenizer::get_keyword_token_type(const StringView& str)
{
	if (str.count > 8) return TOKEN_ERROR;
	u64 hash = string_hash_ascii_9(str);
	bool is_keyword = keyword_hash_to_token_type.find(hash) != keyword_hash_to_token_type.end();
	return is_keyword ? keyword_hash_to_token_type.at(hash) : TOKEN_ERROR;
}
