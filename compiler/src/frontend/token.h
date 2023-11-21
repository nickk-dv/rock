#ifndef TOKEN_H
#define TOKEN_H

#include "general/general.h"
#include "error_handler.h"

enum class TokenType;
enum class UnaryOp;
enum class BinaryOp;
enum class AssignOp;
enum class BasicType;
struct Span;
struct Token;

TokenType token_str_to_keyword(StringView str);
constexpr option<UnaryOp> token_to_unary_op(TokenType type);
constexpr option<BinaryOp> token_to_binary_op(TokenType type);
constexpr option<AssignOp> token_to_assign_op(TokenType type);
constexpr option<BasicType> token_to_basic_type(TokenType type);
constexpr u32 token_binary_op_prec(BinaryOp binary_op);

enum class TokenType
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
	KEYWORD_USE,           // use
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

enum class UnaryOp
{
	MINUS,          // -
	LOGIC_NOT,      // !
	BITWISE_NOT,    // ~
	ADDRESS_OF,     // *
	DEREFERENCE,    // <<
};

enum class BinaryOp
{
	LOGIC_AND,      // &&
	LOGIC_OR,       // ||
	LESS,           // <
	GREATER,        // >
	LESS_EQUALS,    // <=
	GREATER_EQUALS, // >=
	IS_EQUALS,      // ==
	NOT_EQUALS,     // !=
	PLUS,           // +
	MINUS,          // -
	TIMES,          // *
	DIV,            // /
	MOD,            // %
	BITWISE_AND,    // &
	BITWISE_OR,     // |
	BITWISE_XOR,    // ^
	BITSHIFT_LEFT,  // <<
	BITSHIFT_RIGHT, // >>
};

enum class AssignOp
{
	NONE,           // =
	PLUS,           // +=
	MINUS,          // -=
	TIMES,          // *=
	DIV,            // /=
	MOD,            // %=
	BITWISE_AND,    // &=
	BITWISE_OR,     // |=
	BITWISE_XOR,    // ^=
	BITSHIFT_LEFT,  // <<=
	BITSHIFT_RIGHT, // >>=
};

enum class BasicType
{
	I8,
	U8,
	I16,
	U16,
	I32,
	U32,
	I64,
	U64,
	BOOL,
	F32,
	F64,
	STRING,
};

struct Token
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

constexpr option<UnaryOp> token_to_unary_op(TokenType type)
{
	switch (type)
	{
	case TokenType::MINUS:         return UnaryOp::MINUS;
	case TokenType::LOGIC_NOT:     return UnaryOp::LOGIC_NOT;
	case TokenType::BITWISE_NOT:   return UnaryOp::BITWISE_NOT;
	case TokenType::TIMES:         return UnaryOp::ADDRESS_OF;
	case TokenType::BITSHIFT_LEFT: return UnaryOp::DEREFERENCE;
	default: return {};
	}
}

constexpr option<BinaryOp> token_to_binary_op(TokenType type)
{
	switch (type)
	{
	case TokenType::LOGIC_AND:      return BinaryOp::LOGIC_AND;
	case TokenType::LOGIC_OR:       return BinaryOp::LOGIC_OR;
	case TokenType::LESS:           return BinaryOp::LESS;
	case TokenType::GREATER:        return BinaryOp::GREATER;
	case TokenType::LESS_EQUALS:    return BinaryOp::LESS_EQUALS;
	case TokenType::GREATER_EQUALS: return BinaryOp::GREATER_EQUALS;
	case TokenType::IS_EQUALS:      return BinaryOp::IS_EQUALS;
	case TokenType::NOT_EQUALS:     return BinaryOp::NOT_EQUALS;
	case TokenType::PLUS:           return BinaryOp::PLUS;
	case TokenType::MINUS:          return BinaryOp::MINUS;
	case TokenType::TIMES:          return BinaryOp::TIMES;
	case TokenType::DIV:            return BinaryOp::DIV;
	case TokenType::MOD:            return BinaryOp::MOD;
	case TokenType::BITWISE_AND:    return BinaryOp::BITWISE_AND;
	case TokenType::BITWISE_OR:     return BinaryOp::BITWISE_OR;
	case TokenType::BITWISE_XOR:    return BinaryOp::BITWISE_XOR;
	case TokenType::BITSHIFT_LEFT:  return BinaryOp::BITSHIFT_LEFT;
	case TokenType::BITSHIFT_RIGHT: return BinaryOp::BITSHIFT_RIGHT;
	default: return {};
	}
}

constexpr option<AssignOp> token_to_assign_op(TokenType type)
{
	switch (type)
	{
	case TokenType::ASSIGN:                return AssignOp::NONE;
	case TokenType::PLUS_EQUALS:           return AssignOp::PLUS;
	case TokenType::MINUS_EQUALS:          return AssignOp::MINUS;
	case TokenType::TIMES_EQUALS:          return AssignOp::TIMES;
	case TokenType::DIV_EQUALS:            return AssignOp::DIV;
	case TokenType::MOD_EQUALS:            return AssignOp::MOD;
	case TokenType::BITWISE_AND_EQUALS:    return AssignOp::BITWISE_AND;
	case TokenType::BITWISE_OR_EQUALS:     return AssignOp::BITWISE_OR;
	case TokenType::BITWISE_XOR_EQUALS:    return AssignOp::BITWISE_XOR;
	case TokenType::BITSHIFT_LEFT_EQUALS:  return AssignOp::BITSHIFT_LEFT;
	case TokenType::BITSHIFT_RIGHT_EQUALS: return AssignOp::BITSHIFT_RIGHT;
	default: return {};
	}
}

constexpr option<BasicType> token_to_basic_type(TokenType type)
{
	switch (type)
	{
	case TokenType::TYPE_I8:     return BasicType::I8;
	case TokenType::TYPE_U8:     return BasicType::U8;
	case TokenType::TYPE_I16:    return BasicType::I16;
	case TokenType::TYPE_U16:    return BasicType::U16;
	case TokenType::TYPE_I32:    return BasicType::I32;
	case TokenType::TYPE_U32:    return BasicType::U32;
	case TokenType::TYPE_I64:    return BasicType::I64;
	case TokenType::TYPE_U64:    return BasicType::U64;
	case TokenType::TYPE_F32:    return BasicType::F32;
	case TokenType::TYPE_F64:    return BasicType::F64;
	case TokenType::TYPE_BOOL:   return BasicType::BOOL;
	case TokenType::TYPE_STRING: return BasicType::STRING;
	default: return {};
	}
}

constexpr u32 token_binary_op_prec(BinaryOp binary_op)
{
	switch (binary_op)
	{
	case BinaryOp::LOGIC_AND:
	case BinaryOp::LOGIC_OR:
		return 0;
	case BinaryOp::LESS:
	case BinaryOp::GREATER:
	case BinaryOp::LESS_EQUALS:
	case BinaryOp::GREATER_EQUALS:
	case BinaryOp::IS_EQUALS:
	case BinaryOp::NOT_EQUALS:
		return 1;
	case BinaryOp::PLUS:
	case BinaryOp::MINUS:
		return 2;
	case BinaryOp::TIMES:
	case BinaryOp::DIV:
	case BinaryOp::MOD:
		return 3;
	case BinaryOp::BITWISE_AND:
	case BinaryOp::BITWISE_OR:
	case BinaryOp::BITWISE_XOR:
		return 4;
	case BinaryOp::BITSHIFT_LEFT:
	case BinaryOp::BITSHIFT_RIGHT: 
		return 5;
	default:
	{
		err_report(Error::COMPILER_INTERNAL);
		err_context("unexpected switch case in token_binary_op_prec");
		return 0;
	}
	}
}

#endif
