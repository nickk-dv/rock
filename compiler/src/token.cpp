#include "token.h"

UnaryOp token_to_unary_op(TokenType type) 
{
	switch (type) 
	{
		case TOKEN_MINUS: return UNARY_OP_MINUS;
		case TOKEN_LOGIC_NOT: return UNARY_OP_LOGIC_NOT;
		case TOKEN_BITWISE_AND: return UNARY_OP_ADRESS_OF;
		case TOKEN_BITWISE_NOT: return UNARY_OP_BITWISE_NOT;
		default: return UNARY_OP_ERROR;
	}
}

BinaryOp token_to_binary_op(TokenType type) 
{
	switch (type) 
	{
		case TOKEN_LOGIC_AND: return BINARY_OP_LOGIC_AND;
		case TOKEN_LOGIC_OR: return BINARY_OP_LOGIC_OR;
		case TOKEN_LESS: return BINARY_OP_LESS;
		case TOKEN_GREATER: return BINARY_OP_GREATER;
		case TOKEN_LESS_EQUALS: return BINARY_OP_LESS_EQUALS;
		case TOKEN_GREATER_EQUALS: return BINARY_OP_GREATER_EQUALS;
		case TOKEN_IS_EQUALS: return BINARY_OP_IS_EQUALS;
		case TOKEN_NOT_EQUALS: return BINARY_OP_NOT_EQUALS;
		case TOKEN_PLUS: return BINARY_OP_PLUS;
		case TOKEN_MINUS: return BINARY_OP_MINUS;
		case TOKEN_TIMES: return BINARY_OP_TIMES;
		case TOKEN_DIV: return BINARY_OP_DIV;
		case TOKEN_MOD: return BINARY_OP_MOD;
		case TOKEN_BITWISE_AND: return BINARY_OP_BITWISE_AND;
		case TOKEN_BITWISE_OR: return BINARY_OP_BITWISE_OR;
		case TOKEN_BITWISE_XOR: return BINARY_OP_BITWISE_XOR;
		case TOKEN_BITSHIFT_LEFT: return BINARY_OP_BITSHIFT_LEFT;
		case TOKEN_BITSHIFT_RIGHT: return BINARY_OP_BITSHIFT_RIGHT;
		default: return BINARY_OP_ERROR;
	}
}

AssignOp token_to_assign_op(TokenType type) 
{
	switch (type) 
	{
		case TOKEN_ASSIGN: return ASSIGN_OP_NONE;
		case TOKEN_PLUS_EQUALS: return ASSIGN_OP_PLUS;
		case TOKEN_MINUS_EQUALS: return ASSIGN_OP_MINUS;
		case TOKEN_TIMES_EQUALS: return ASSIGN_OP_TIMES;
		case TOKEN_DIV_EQUALS: return ASSIGN_OP_DIV;
		case TOKEN_MOD_EQUALS: return ASSIGN_OP_MOD;
		case TOKEN_BITWISE_AND_EQUALS: return ASSIGN_OP_BITWISE_AND;
		case TOKEN_BITWISE_OR_EQUALS: return ASSIGN_OP_BITWISE_OR;
		case TOKEN_BITWISE_XOR_EQUALS: return ASSIGN_OP_BITWISE_XOR;
		case TOKEN_BITSHIFT_LEFT_EQUALS: return ASSIGN_OP_BITSHIFT_LEFT;
		case TOKEN_BITSHIFT_RIGHT_EQUALS: return ASSIGN_OP_BITSHIFT_RIGHT;
		default: return ASSIGN_OP_ERROR;
	}
}

BasicType token_to_basic_type(TokenType type)
{
	if (type < TOKEN_TYPE_I8 || type > TOKEN_TYPE_STRING)
		return BASIC_TYPE_ERROR;
	return BasicType(type - TOKEN_TYPE_I8);
}

u32 token_binary_op_prec(BinaryOp binary_op)
{
	switch (binary_op)
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
	{ hash_ascii_9("defer"),    TOKEN_KEYWORD_DEFER },
	{ hash_ascii_9("break"),    TOKEN_KEYWORD_BREAK },
	{ hash_ascii_9("return"),   TOKEN_KEYWORD_RETURN },
	{ hash_ascii_9("continue"), TOKEN_KEYWORD_CONTINUE },
	{ hash_ascii_9("import"),   TOKEN_KEYWORD_IMPORT },
	{ hash_ascii_9("use"),      TOKEN_KEYWORD_USE },
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

TokenType token_str_to_keyword(StringView str)
{
	if (str.count > 8 || str.count < 2) return TOKEN_ERROR;
	u64 hash = hash_str_ascii_9(str);
	bool is_keyword = keyword_hash_to_token_type.find(hash) != keyword_hash_to_token_type.end();
	return is_keyword ? keyword_hash_to_token_type.at(hash) : TOKEN_ERROR;
}

bool token_is_literal(TokenType type)
{
	return type >= TOKEN_BOOL_LITERAL && type <= TOKEN_STRING_LITERAL;
}
