#include "ast.h"

#include <unordered_map>

static const std::unordered_map<TokenType, BinaryOp> token_to_binary_op =
{
	{TOKEN_LOGIC_AND, BINARY_OP_LOGIC_AND},
	{TOKEN_LOGIC_OR, BINARY_OP_LOGIC_OR},

	{TOKEN_LESS, BINARY_OP_LESS},
	{TOKEN_GREATER, BINARY_OP_GREATER},
	{TOKEN_LESS_EQUALS, BINARY_OP_LESS_EQUALS},
	{TOKEN_GREATER_EQUALS, BINARY_OP_GREATER_EQUALS},
	{TOKEN_IS_EQUALS, BINARY_OP_IS_EQUALS},
	{TOKEN_NOT_EQUALS, BINARY_OP_NOT_EQUALS},

	{TOKEN_PLUS, BINARY_OP_PLUS},
	{TOKEN_MINUS, BINARY_OP_MINUS},

	{TOKEN_TIMES, BINARY_OP_TIMES},
	{TOKEN_DIV, BINARY_OP_DIV},
	{TOKEN_MOD, BINARY_OP_MOD},

	{TOKEN_BITWISE_AND, BINARY_OP_BITWISE_AND},
	{TOKEN_BITWISE_XOR, BINARY_OP_BITWISE_XOR},
	{TOKEN_BITWISE_OR, BINARY_OP_BITWISE_OR},

	{TOKEN_BITSHIFT_LEFT, BINARY_OP_BITSHIFT_LEFT},
	{TOKEN_BITSHIFT_RIGHT, BINARY_OP_BITSHIFT_RIGHT},
};

//@Article about improving clarity of op prec: 
//https://www.foonathan.net/2017/07/operator-precedence/

static const std::unordered_map<BinaryOp, u32> binary_op_precedence =
{
	{BINARY_OP_LOGIC_AND, 0},
	{BINARY_OP_LOGIC_OR, 0},

	{BINARY_OP_LESS, 1},
	{BINARY_OP_GREATER, 1},
	{BINARY_OP_LESS_EQUALS, 1},
	{BINARY_OP_GREATER_EQUALS, 1},
	{BINARY_OP_IS_EQUALS, 1},
	{BINARY_OP_NOT_EQUALS, 1},

	{BINARY_OP_PLUS, 2},
	{BINARY_OP_MINUS, 2},

	{BINARY_OP_TIMES, 3},
	{BINARY_OP_DIV, 3},
	{BINARY_OP_MOD, 3},

	{BINARY_OP_BITWISE_AND, 4},
	{BINARY_OP_BITWISE_XOR, 4},
	{BINARY_OP_BITWISE_OR, 4},

	{BINARY_OP_BITSHIFT_LEFT, 5},
	{BINARY_OP_BITSHIFT_RIGHT, 5},
};

BinaryOp ast_binary_op_from_token(TokenType type)
{
	if (token_to_binary_op.find(type) != token_to_binary_op.end())
		return token_to_binary_op.at(type);
	return BINARY_OP_ERROR;
}

u32 ast_binary_op_precedence(BinaryOp op)
{
	return binary_op_precedence.at(op);
}
