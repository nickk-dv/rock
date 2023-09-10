#include "ast.h"

#include <unordered_map>

static const std::unordered_map<TokenType, BinaryOp> token_to_binary_op =
{
	{TOKEN_PLUS, BINARY_OP_PLUS},
	{TOKEN_MINUS, BINARY_OP_MINUS},
	{TOKEN_TIMES, BINARY_OP_TIMES},
	{TOKEN_DIV, BINARY_OP_DIV},
	{TOKEN_MOD, BINARY_OP_MOD},
};

static const std::unordered_map<BinaryOp, u32> binary_op_precedence =
{
	{BINARY_OP_PLUS, 0},
	{BINARY_OP_MINUS, 0},
	{BINARY_OP_TIMES, 1},
	{BINARY_OP_DIV, 1},
	{BINARY_OP_MOD, 1},
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
