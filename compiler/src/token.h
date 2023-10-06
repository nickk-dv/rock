#ifndef TOKEN_H
#define TOKEN_H

#include "common.h"

enum TokenType;
enum UnaryOp;
enum BinaryOp;
enum AssignOp;
enum BasicType;
struct Token;

UnaryOp token_to_unary_op(TokenType type);
BinaryOp token_to_binary_op(TokenType type);
AssignOp token_to_assign_op(TokenType type);
BasicType token_to_basic_type(TokenType type);
u32 token_binary_op_prec(BinaryOp binary_op);
TokenType token_str_to_keyword(StringView str);
bool token_is_literal(TokenType type);

enum TokenType
{
	TOKEN_IDENT,                 // name
	TOKEN_BOOL_LITERAL,          // true false
	TOKEN_FLOAT_LITERAL,         // 10.5
	TOKEN_INTEGER_LITERAL,       // 10
	TOKEN_STRING_LITERAL,        // "string"

	TOKEN_KEYWORD_STRUCT,        // struct
	TOKEN_KEYWORD_ENUM,          // enum
	TOKEN_KEYWORD_IF,            // if
	TOKEN_KEYWORD_ELSE,          // else
	TOKEN_KEYWORD_TRUE,          // true
	TOKEN_KEYWORD_FALSE,         // false
	TOKEN_KEYWORD_FOR,           // for
	TOKEN_KEYWORD_DEFER,         // defer
	TOKEN_KEYWORD_BREAK,         // break
	TOKEN_KEYWORD_RETURN,        // return
	TOKEN_KEYWORD_CONTINUE,      // continue
	TOKEN_KEYWORD_IMPORT,        // import
	TOKEN_KEYWORD_USE,           // use

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
	TOKEN_COLON,                 // :
	TOKEN_QUOTE,                 // '
	TOKEN_COMMA,                 // ,
	TOKEN_SEMICOLON,             // ;
	TOKEN_DOUBLE_DOT,            // ..
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
	UNARY_OP_ADRESS_OF,       // &
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

enum BasicType
{
	BASIC_TYPE_I8, 
	BASIC_TYPE_U8,
	BASIC_TYPE_I16, 
	BASIC_TYPE_U16,
	BASIC_TYPE_I32, 
	BASIC_TYPE_U32,
	BASIC_TYPE_I64, 
	BASIC_TYPE_U64,
	BASIC_TYPE_F32, 
	BASIC_TYPE_F64,
	BASIC_TYPE_BOOL, 
	BASIC_TYPE_STRING,
	BASIC_TYPE_ERROR,
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
		char* string_literal_value; //@Not sure about nessesary implementation yet
		StringView string_value;
	};
};

#endif
