
enum TokenType
{
	TOKEN_IDENT,                 // name
	TOKEN_NUMBER,                // 10
	TOKEN_STRING,                // "string"
	TOKEN_BOOL_LITERAL,          // true false

	TOKEN_KEYWORD_STRUCT,        // struct
	TOKEN_KEYWORD_ENUM,          // enum
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

enum UnaryOp
{
	UNARY_OP_MINUS,           // -
	UNARY_OP_LOGIC_NOT,       // !
	UNARY_OP_BITWISE_NOT,     // ~
	UNARY_OP_ERROR,
};

enum BinaryOp
{
	BINARY_OP_LOGIC_AND,      // &&
	BINARY_OP_LOGIC_OR,       // ||
	BINARY_OP_LESS,           // <
	BINARY_OP_GREATER,        // >
	BINARY_OP_LESS_EQUALS,    // <=
	BINARY_OP_GREATER_EQUALS, // >=
	BINARY_OP_IS_EQUALS,      // ==
	BINARY_OP_NOT_EQUALS,     // !=
	BINARY_OP_PLUS,           // +
	BINARY_OP_MINUS,          // -
	BINARY_OP_TIMES,          // *
	BINARY_OP_DIV,            // /
	BINARY_OP_MOD,            // %
	BINARY_OP_BITWISE_AND,    // &
	BINARY_OP_BITWISE_OR,     // |
	BINARY_OP_BITWISE_XOR,    // ^
	BINARY_OP_BITSHIFT_LEFT,  // <<
	BINARY_OP_BITSHIFT_RIGHT, // >>
	BINARY_OP_ERROR,
};

enum AssignOp
{
	ASSIGN_OP_NONE,           // =
	ASSIGN_OP_PLUS,           // +=
	ASSIGN_OP_MINUS,          // -=
	ASSIGN_OP_TIMES,          // *=
	ASSIGN_OP_DIV,            // /=
	ASSIGN_OP_MOD,            // %=
	ASSIGN_OP_BITWISE_AND,    // &=
	ASSIGN_OP_BITWISE_OR,	  // |=
	ASSIGN_OP_BITWISE_XOR,	  // ^=
	ASSIGN_OP_BITSHIFT_LEFT,  // <<=
	ASSIGN_OP_BITSHIFT_RIGHT, // >>=
	ASSIGN_OP_ERROR,
};

u32 binary_op_get_prec(BinaryOp op) 
{
	switch (op) 
	{
		case BINARY_OP_LOGIC_AND:
		case BINARY_OP_LOGIC_OR:
			return 0;
		case BINARY_OP_LESS:
		case BINARY_OP_GREATER:
		case BINARY_OP_LESS_EQUALS:
		case BINARY_OP_GREATER_EQUALS:
		case BINARY_OP_IS_EQUALS:
		case BINARY_OP_NOT_EQUALS:
			return 1;
		case BINARY_OP_PLUS:
		case BINARY_OP_MINUS:
			return 2;
		case BINARY_OP_TIMES:
		case BINARY_OP_DIV:
		case BINARY_OP_MOD:
			return 3;
		case BINARY_OP_BITWISE_AND:
		case BINARY_OP_BITWISE_OR:
		case BINARY_OP_BITWISE_XOR:
			return 4;
		case BINARY_OP_BITSHIFT_LEFT:
		case BINARY_OP_BITSHIFT_RIGHT:
			return 5;
		default: return 0;
	}
}

static const std::unordered_map<u64, TokenType> keyword_hash_to_token_type =
{
	{ hash_ascii_9("struct"),   TOKEN_KEYWORD_STRUCT },
	{ hash_ascii_9("enum"),     TOKEN_KEYWORD_ENUM },
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
	bool set_input_from_file(const char* file_path);
	void tokenize_buffer();

	void skip_whitespace();
	void skip_whitespace_comments();
	std::optional<u8> peek(u32 offset = 0);
	void consume();
	TokenType get_keyword_token_type(const StringView& str);
	bool is_letter(u8 c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z'); }
	bool is_number(u8 c) { return c >= '0' && c <= '9'; }
	bool is_ident(u8 c) { return is_letter(c) || (c == '_') || is_number(c); }
	bool is_whitespace(u8 c) { return c == ' ' || c == '\t' || c == '\r' || c == '\n'; }
	UnaryOp token_to_unary_op(TokenType type) { return tok_to_unop[type]; }
	BinaryOp token_to_binary_op(TokenType type) { return tok_to_binop[type]; }
	AssignOp token_to_assign_op(TokenType type) { return tok_to_asgnop[type]; }

	String input;
	u64 input_cursor = 0;
	u64 line_start_cursor = 0;
	u32 line_id = 1;
	u32 peek_index = 0;
	TokenType c_to_sym[128];
	LexemeType lexeme_types[128];
	UnaryOp tok_to_unop[TOKEN_EOF + 1];
	BinaryOp tok_to_binop[TOKEN_EOF + 1];
	AssignOp tok_to_asgnop[TOKEN_EOF + 1];
	static const u64 TOKENIZER_BUFFER_SIZE = 256;
	static const u64 TOKENIZER_LOOKAHEAD = 2;
	Token tokens[TOKENIZER_BUFFER_SIZE];
	StringStorage string_storage;
};

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

	for (u32 i = 0; i < TOKEN_EOF + 1; i++)
	{
		tok_to_unop[i] = UNARY_OP_ERROR;
		tok_to_binop[i] = BINARY_OP_ERROR;
		tok_to_asgnop[i] = ASSIGN_OP_ERROR;
	}

	tok_to_unop[TOKEN_MINUS] = UNARY_OP_MINUS;
	tok_to_unop[TOKEN_LOGIC_NOT] = UNARY_OP_LOGIC_NOT;
	tok_to_unop[TOKEN_BITWISE_NOT] = UNARY_OP_BITWISE_NOT;

	tok_to_binop[TOKEN_PLUS] = BINARY_OP_PLUS;
	tok_to_binop[TOKEN_MINUS] = BINARY_OP_MINUS;
	tok_to_binop[TOKEN_TIMES] = BINARY_OP_TIMES;
	tok_to_binop[TOKEN_DIV] = BINARY_OP_DIV;
	tok_to_binop[TOKEN_MOD] = BINARY_OP_MOD;
	tok_to_binop[TOKEN_BITWISE_AND] = BINARY_OP_BITWISE_AND;
	tok_to_binop[TOKEN_BITWISE_OR] = BINARY_OP_BITWISE_OR;
	tok_to_binop[TOKEN_BITWISE_XOR] = BINARY_OP_BITWISE_XOR;
	tok_to_binop[TOKEN_LESS] = BINARY_OP_LESS;
	tok_to_binop[TOKEN_GREATER] = BINARY_OP_GREATER;
	tok_to_binop[TOKEN_IS_EQUALS] = BINARY_OP_IS_EQUALS;
	tok_to_binop[TOKEN_LESS_EQUALS] = BINARY_OP_LESS_EQUALS;
	tok_to_binop[TOKEN_GREATER_EQUALS] = BINARY_OP_GREATER_EQUALS;
	tok_to_binop[TOKEN_NOT_EQUALS] = BINARY_OP_NOT_EQUALS;
	tok_to_binop[TOKEN_LOGIC_AND] = BINARY_OP_LOGIC_AND;
	tok_to_binop[TOKEN_LOGIC_OR] = BINARY_OP_LOGIC_OR;
	tok_to_binop[TOKEN_BITSHIFT_LEFT] = BINARY_OP_BITSHIFT_LEFT;
	tok_to_binop[TOKEN_BITSHIFT_RIGHT] = BINARY_OP_BITSHIFT_RIGHT;

	tok_to_asgnop[TOKEN_ASSIGN] = ASSIGN_OP_NONE;
	tok_to_asgnop[TOKEN_PLUS] = ASSIGN_OP_PLUS;
	tok_to_asgnop[TOKEN_MINUS] = ASSIGN_OP_MINUS;
	tok_to_asgnop[TOKEN_TIMES] = ASSIGN_OP_TIMES;
	tok_to_asgnop[TOKEN_DIV] = ASSIGN_OP_DIV;
	tok_to_asgnop[TOKEN_MOD] = ASSIGN_OP_MOD;
	tok_to_asgnop[TOKEN_BITWISE_AND] = ASSIGN_OP_BITWISE_AND;
	tok_to_asgnop[TOKEN_BITWISE_OR] = ASSIGN_OP_BITWISE_OR;
	tok_to_asgnop[TOKEN_BITWISE_XOR] = ASSIGN_OP_BITWISE_XOR;
	tok_to_asgnop[TOKEN_BITSHIFT_LEFT] = ASSIGN_OP_BITSHIFT_LEFT;
	tok_to_asgnop[TOKEN_BITSHIFT_RIGHT] = ASSIGN_OP_BITSHIFT_RIGHT;

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

				TokenType keyword = get_keyword_token_type(token.string_value);
				if (keyword != TOKEN_ERROR) token.type = keyword;
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

				token.type = TOKEN_NUMBER;
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

				token.type = TOKEN_STRING;
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

//@Perf test the different keyword methods on bigger files like switching on char in a tree like search
TokenType Tokenizer::get_keyword_token_type(const StringView& str)
{
	if (str.count > 8 || str.count < 2) return TOKEN_ERROR;
	u64 hash = string_hash_ascii_9(str);
	bool is_keyword = keyword_hash_to_token_type.find(hash) != keyword_hash_to_token_type.end();
	return is_keyword ? keyword_hash_to_token_type.at(hash) : TOKEN_ERROR;
}
