export module token;

import general;

export enum class TokenType
{
	IDENT,                 // name
	LITERAL_INT,           // 10
	LITERAL_FLOAT,         // 10.5
	LITERAL_BOOL,          // true false
	LITERAL_STRING,        // "string"

	KEYWORD_PUB,           // pub
	KEYWORD_MOD,           // mod
	KEYWORD_MUT,           // mut
	KEYWORD_SELF,          // self
	KEYWORD_IMPL,          // impl
	KEYWORD_ENUM,          // enum
	KEYWORD_STRUCT,        // struct
	KEYWORD_IMPORT,        // import

	KEYWORD_IF,            // if
	KEYWORD_ELSE,          // else
	KEYWORD_FOR,           // for
	KEYWORD_DEFER,         // defer
	KEYWORD_BREAK,         // break
	KEYWORD_RETURN,        // return
	KEYWORD_SWITCH,        // switch
	KEYWORD_CONTINUE,      // continue
	
	KEYWORD_CAST,          // cast
	KEYWORD_SIZEOF,        // sizeof
	KEYWORD_TRUE,          // true
	KEYWORD_FALSE,         // false

	KEYWORD_I8,            // i8
	KEYWORD_I16,           // i16
	KEYWORD_I32,           // i32
	KEYWORD_I64,           // i64
	KEYWORD_U8,            // u8
	KEYWORD_U16,           // u16
	KEYWORD_U32,           // u32
	KEYWORD_U64,           // u64
	KEYWORD_F32,           // f32
	KEYWORD_F64,           // f64
	KEYWORD_BOOL,          // bool
	KEYWORD_STRING,        // string

	AT,                    // @
	DOT,                   // .
	COLON,                 // :
	COMMA,                 // ,
	SEMICOLON,             // ;
	DOUBLE_DOT,            // ..
	DOUBLE_COLON,          // ::
	ARROW_THIN,            // ->
	ARROW_WIDE,            // =>
	PAREN_START,           // (
	PAREN_END,             // )
	BLOCK_START,           // {
	BLOCK_END,             // }
	BRACKET_START,         // [
	BRACKET_END,           // ]

	LOGIC_NOT,             // !
	BITWISE_NOT,           // ~
	
	LOGIC_AND,             // &&
	LOGIC_OR,              // ||
	LESS,                  // <
	GREATER,               // >
	LESS_EQUALS,           // <=
	GREATER_EQUALS,        // >=
	IS_EQUALS,             // ==
	NOT_EQUALS,            // !=
	
	PLUS,                  // +
	MINUS,                 // -
	TIMES,                 // *
	DIV,                   // /
	MOD,                   // %
	BITWISE_AND,           // &
	BITWISE_OR,            // |
	BITWISE_XOR,           // ^
	BITSHIFT_LEFT,         // <<
	BITSHIFT_RIGHT,        // >>
	
	ASSIGN,                // =
	PLUS_EQUALS,           // +=
	MINUS_EQUALS,          // -=
	TIMES_EQUALS,          // *=
	DIV_EQUALS,            // /=
	MOD_EQUALS,            // %=
	BITWISE_AND_EQUALS,    // &=
	BITWISE_OR_EQUALS,     // |=
	BITWISE_XOR_EQUALS,    // ^=
	BITSHIFT_LEFT_EQUALS,  // <<=
	BITSHIFT_RIGHT_EQUALS, // >>=

	INPUT_END,
	ERROR,
};

export struct Token
{
	Span span;
	TokenType type = TokenType::ERROR;

	union
	{
		u64 literal_u64;
		f64 literal_f64;
		bool literal_bool;
		char* literal_string;
		StringView source_str;
	};
};
