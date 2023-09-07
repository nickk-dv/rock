#include "parser.h"

#include "common.h"
#include "error.h"
#include "token.h"

Parser::Parser(std::vector<Token> tokens) 
	: m_tokens(std::move(tokens)), 
	  m_arena(1024 * 1024 * 4) {}

void Parser::parse_struct()
{
	auto token_struct_type = peek(); // Type
	if (!token_struct_type || token_struct_type.value().type != TOKEN_IDENT)
		exit_error();
	consume();

	auto token_scope_start = peek(); // {
	if (!token_scope_start || token_scope_start.value().type != TOKEN_SCOPE_START)
		exit_error();
	consume();

	while (true)
	{
		auto token_field = peek(); // field
		if (!token_field || token_field.value().type != TOKEN_IDENT)
			break; // No more fields
		else consume();

		auto token_colon = peek(); // :
		if (!token_colon || token_colon.value().type != TOKEN_COLON)
			exit_error();
		consume();

		auto token_field_type = peek(); // Type
		if (!token_field_type || token_field_type.value().type != TOKEN_IDENT)
			exit_error();
		consume();

		auto token_coma = peek(); // ,
		if (token_coma && token_coma.value().type == TOKEN_COMA)
			consume();
		else break; // No more fields
	}

	auto token_scope_end = peek(); // }
	if (!token_scope_end || token_scope_end.value().type != TOKEN_SCOPE_END)
		exit_error();
	consume();
}

void Parser::parse_enum()
{
	auto token_enum_type = peek(); // Type
	if (!token_enum_type || token_enum_type.value().type != TOKEN_IDENT)
		exit_error();
	consume();

	auto token_scope_start = peek(); // {
	if (!token_scope_start || token_scope_start.value().type != TOKEN_SCOPE_START)
		exit_error();
	consume();

	while (true)
	{
		auto token_variant = peek(); // variant
		if (!token_variant || token_variant.value().type != TOKEN_IDENT)
			break; // No more variants
		else consume();

		auto token_coma = peek(); // ,
		if (token_coma && token_coma.value().type == TOKEN_COMA)
			consume();
		else break; // No more variants
	}

	auto token_scope_end = peek(); // }
	if (!token_scope_end || token_scope_end.value().type != TOKEN_SCOPE_END)
		exit_error();
	consume();
}

void Parser::parse_fn()
{
	auto token_name = peek(); // name
	if (!token_name || token_name.value().type != TOKEN_IDENT)
		exit_error();
	consume();

	auto token_paren_start = peek(); // (
	if (!token_paren_start || token_paren_start.value().type != TOKEN_PARENTHESIS_START)
		exit_error();
	consume();

	while (true)
	{
		auto token_param = peek(); // param
		if (!token_param || token_param.value().type != TOKEN_IDENT)
			break; // No more params
		else consume();

		auto token_colon = peek(); // :
		if (!token_colon || token_colon.value().type != TOKEN_COLON)
			exit_error();
		consume();

		auto token_param_type = peek(); // Type
		if (!token_param_type || token_param_type.value().type != TOKEN_IDENT)
			exit_error();
		consume();

		auto token_coma = peek(); // ,
		if (token_coma && token_coma.value().type == TOKEN_COMA)
			consume();
		else break; // No more params
	}

	auto token_paren_end = peek(); // )
	if (!token_paren_end || token_paren_end.value().type != TOKEN_PARENTHESIS_END)
		exit_error();
	consume();

	auto token_arrow = peek(); // ->
	if (token_arrow && token_arrow.value().type == TOKEN_ARROW)
	{
		consume();

		auto token_return_type = peek(); // Type
		if (!token_return_type || token_return_type.value().type != TOKEN_IDENT)
			exit_error();
		consume();
	}

	auto token_scope_start = peek(); // {
	if (!token_scope_start || token_scope_start.value().type != TOKEN_SCOPE_START)
		exit_error();
	consume();
}

void Parser::parse()
{
	while (peek().has_value())
	{
		TokenType type = peek().value().type;
		consume();

		switch (type)
		{
			case TOKEN_KEYWORD_STRUCT: parse_struct(); break;
			case TOKEN_KEYWORD_ENUM: parse_enum(); break;
			case TOKEN_KEYWORD_FN: parse_fn(); break;
		}
	}
}

std::optional<Token> Parser::peek(u32 offset)
{
	if (m_index + offset >= m_tokens.size()) return {};
	else return m_tokens[m_index + offset];
}

void Parser::consume()
{
	m_index += 1;
}

void Parser::exit_error()
{
	error_report(PARSE_ERROR_TEST, {});
	exit(EXIT_FAILURE);
}
