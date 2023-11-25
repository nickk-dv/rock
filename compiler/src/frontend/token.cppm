export module token;

import general;

export enum class TokenType
{
	IDENT,                 // name
	BOOL_LITERAL,          // true false
	FLOAT_LITERAL,         // 10.5
	INTEGER_LITERAL,       // 10
	STRING_LITERAL,        // "string"

	KEYWORD_STRUCT,        // struct
	KEYWORD_ENUM,          // enum
	KEYWORD_IF,            // if
	KEYWORD_ELSE,          // else
	KEYWORD_TRUE,          // true
	KEYWORD_FALSE,         // false
	KEYWORD_FOR,           // for
	KEYWORD_CAST,          // cast
	KEYWORD_DEFER,         // defer
	KEYWORD_BREAK,         // break
	KEYWORD_RETURN,        // return
	KEYWORD_SWITCH,        // switch
	KEYWORD_CONTINUE,      // continue
	KEYWORD_SIZEOF,        // sizeof
	KEYWORD_IMPORT,        // import
	KEYWORD_IMPL,          // impl
	KEYWORD_SELF,          // self

	TYPE_I8,               // i8
	TYPE_U8,               // u8
	TYPE_I16,              // i16
	TYPE_U16,              // u16
	TYPE_I32,              // i32
	TYPE_U32,              // u32
	TYPE_I64,              // i64
	TYPE_U64,              // u64
	TYPE_F32,              // f32
	TYPE_F64,              // f64
	TYPE_BOOL,             // bool
	TYPE_STRING,           // string

	DOT,                   // .
	COLON,                 // :
	COMMA,                 // ,
	SEMICOLON,             // ;
	DOUBLE_DOT,            // ..
	DOUBLE_COLON,          // ::
	BLOCK_START,           // {
	BLOCK_END,             // }
	BRACKET_START,         // [
	BRACKET_END,           // ]
	PAREN_START,           // (
	PAREN_END,             // )
	AT,                    // @
	HASH,                  // #
	QUESTION,              // ?

	ARROW,                 // ->
	ASSIGN,                // =
	PLUS,                  // +
	MINUS,                 // -
	TIMES,                 // *
	DIV,                   // /
	MOD,                   // %
	BITWISE_AND,           // &
	BITWISE_OR,            // |
	BITWISE_XOR,           // ^
	LESS,                  // <
	GREATER,               // >
	LOGIC_NOT,             // !
	IS_EQUALS,             // ==
	PLUS_EQUALS,           // +=
	MINUS_EQUALS,          // -=
	TIMES_EQUALS,          // *=
	DIV_EQUALS,            // /=
	MOD_EQUALS,            // %=
	BITWISE_AND_EQUALS,    // &=
	BITWISE_OR_EQUALS,     // |=
	BITWISE_XOR_EQUALS,    // ^=
	LESS_EQUALS,           // <=
	GREATER_EQUALS,        // >=
	NOT_EQUALS,            // !=
	LOGIC_AND,             // &&
	LOGIC_OR,              // ||
	BITWISE_NOT,           // ~
	BITSHIFT_LEFT,         // <<
	BITSHIFT_RIGHT,        // >>
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
		bool bool_value;
		f64 float64_value;
		u64 integer_value;
		char* string_literal_value;
		StringView string_value;
	};
};
