#include "lexer.h"

#include "token.h"

option<TokenType> lex_symbol_1(u8 c)
{
	switch (c)
	{
	case '.': return TokenType::DOT;
	case ':': return TokenType::COLON;
	case ',': return TokenType::COMMA;
	case ';': return TokenType::SEMICOLON;
	case '{': return TokenType::BLOCK_START;
	case '}': return TokenType::BLOCK_END;
	case '[': return TokenType::BRACKET_START;
	case ']': return TokenType::BRACKET_END;
	case '(': return TokenType::PAREN_START;
	case ')': return TokenType::PAREN_END;
	case '@': return TokenType::AT;
	case '=': return TokenType::ASSIGN;
	case '+': return TokenType::PLUS;
	case '-': return TokenType::MINUS;
	case '*': return TokenType::TIMES;
	case '/': return TokenType::DIV;
	case '%': return TokenType::MOD;
	case '&': return TokenType::BITWISE_AND;
	case '|': return TokenType::BITWISE_OR;
	case '^': return TokenType::BITWISE_XOR;
	case '<': return TokenType::LESS;
	case '>': return TokenType::GREATER;
	case '!': return TokenType::LOGIC_NOT;
	case '~': return TokenType::BITWISE_NOT;
	default: return {};
	}
}

option<TokenType> lex_symbol_2(u8 c, TokenType type)
{
	switch (c)
	{
	case '.': if (type == TokenType::DOT)         return TokenType::DOUBLE_DOT;     else return {};
	case ':': if (type == TokenType::COLON)       return TokenType::DOUBLE_COLON;   else return {};
	case '&': if (type == TokenType::BITWISE_AND) return TokenType::LOGIC_AND;      else return {};
	case '|': if (type == TokenType::BITWISE_OR)  return TokenType::LOGIC_OR;       else return {};
	case '<': if (type == TokenType::LESS)        return TokenType::BITSHIFT_LEFT;  else return {};
	case '>':
	{
		switch (type)
		{
		case TokenType::MINUS:   return TokenType::ARROW;
		case TokenType::GREATER: return TokenType::BITSHIFT_RIGHT;
		default: return {};
		}
	}
	case '=':
	{
		switch (type)
		{
		case TokenType::ASSIGN:      return TokenType::IS_EQUALS;
		case TokenType::PLUS:        return TokenType::PLUS_EQUALS;
		case TokenType::MINUS:       return TokenType::MINUS_EQUALS;
		case TokenType::TIMES:       return TokenType::TIMES_EQUALS;
		case TokenType::DIV:         return TokenType::DIV_EQUALS;
		case TokenType::MOD:         return TokenType::MOD_EQUALS;
		case TokenType::BITWISE_AND: return TokenType::BITWISE_AND_EQUALS;
		case TokenType::BITWISE_OR:  return TokenType::BITWISE_OR_EQUALS;
		case TokenType::BITWISE_XOR: return TokenType::BITWISE_XOR_EQUALS;
		case TokenType::LESS:        return TokenType::LESS_EQUALS;
		case TokenType::GREATER:     return TokenType::GREATER_EQUALS;
		case TokenType::LOGIC_NOT:   return TokenType::NOT_EQUALS;
		default: return {};
		}
	}
	default: return {};
	}
}

option<TokenType> lex_symbol_3(u8 c, TokenType type)
{
	switch (c)
	{
	case '=':
	{
		switch (type)
		{
		case TokenType::BITSHIFT_LEFT:  return TokenType::BITSHIFT_LEFT_EQUALS;
		case TokenType::BITSHIFT_RIGHT: return TokenType::BITSHIFT_RIGHT_EQUALS;
		default: return {};
		}
	}
	default: return {};
	}
}
