#include "parser.h"

#include "common.h"
#include "error.h"
#include "token.h"

void debug_print_tab()
{
	printf("   ");
}

void debug_print_parameter(const IdentTypePair& parameter, bool tab = true, bool newline = true)
{
	if (tab) debug_print_tab();
	error_report_token_ident(parameter.ident.token);
	printf(": ");
	error_report_token_ident(parameter.type.token);
	if (newline) printf(",\n");
}

void debug_print_enum_parameter(const IdentTypePair& parameter, bool tab = true, bool newline = true)
{
	if (tab) debug_print_tab();
	error_report_token_ident(parameter.ident.token);
	if (newline) printf(",\n");
}

void debug_print_definitions(const Ast& ast)
{
	printf("[STRUCTS]\n\n");
	for (const auto& def : ast.structs)
	{
		printf("struct ");
		error_report_token_ident(def.type.token);
		printf("\n{\n");

		for (const auto& parameter : def.fields)
			debug_print_parameter(parameter);
		printf("}\n\n");
	}

	printf("[ENUMS]\n\n");
	for (const auto& def : ast.enums)
	{
		printf("enum ");
		error_report_token_ident(def.type.token);
		printf("\n{\n");
		
		for (const auto& variant : def.variants)
			debug_print_enum_parameter(variant);
		printf("}\n\n");
	}

	printf("[FUNCTIONS]\n\n");
	for (const auto& def : ast.procedures)
	{
		printf("fn ");
		error_report_token_ident(def.ident.token);

		printf("(");
		size_t size = def.input_parameters.size();
		size_t counter = 0;
		for (const auto& parameter : def.input_parameters)
		{
			debug_print_parameter(parameter, false, false);
			counter += 1;
			if (counter != size) printf(", ");
		}
		printf(")");

		if (def.return_type)
		{
			printf(" :: "); 
			error_report_token_ident(def.return_type.value().token);
		}
		printf("\nstatements parsed: %llu\n\n", def.block->statements.size());
	}
}

Parser::Parser(std::vector<Token> tokens) 
	: m_tokens(std::move(tokens)), 
	  m_arena(1024 * 1024 * 4) 
{
}

std::optional<Ast_Struct_Declaration> Parser::parse_struct()
{
	auto type = try_consume(TOKEN_IDENT); 
	if (!type) return {};
	auto scope_start = try_consume(TOKEN_BLOCK_START);
	if (!scope_start) return {};

	Ast_Struct_Declaration struct_decl = {};
	struct_decl.type = Ast_Identifier { type.value() };

	while (true)
	{
		auto field = peek();
		if (field && field.value().type == TOKEN_IDENT)
			consume();
		else break; // No more fields

		auto colon = try_consume(TOKEN_COLON);
		if (!colon) return {};
		auto field_type = try_consume(TOKEN_IDENT);
		if (!field_type) return {};

		struct_decl.fields.emplace_back(IdentTypePair{ field.value(), field_type.value() });

		auto coma = peek();
		if (coma && coma.value().type == TOKEN_COMA)
			consume();
		else break; // No more fields
	}

	auto scope_end = try_consume(TOKEN_BLOCK_END);
	if (!scope_end) return {};

	return struct_decl;
}

std::optional<Ast_Enum_Declaration> Parser::parse_enum()
{
	auto type = try_consume(TOKEN_IDENT);
	if (!type) return {};
	auto scope_start = try_consume(TOKEN_BLOCK_START);
	if (!scope_start) return {};

	Ast_Enum_Declaration enum_decl = {};
	enum_decl.type = Ast_Identifier { type.value() };

	while (true)
	{
		auto variant = peek();
		if (!variant || variant.value().type != TOKEN_IDENT)
			break; // No more variants
		consume();

		//@Notice no type is specified for the enum variant
		//might support typed numeric enums with literal values
		enum_decl.variants.emplace_back(IdentTypePair { variant.value(), {} });

		auto coma = peek();
		if (coma && coma.value().type == TOKEN_COMA)
			consume();
		else break; // No more variants
	}

	auto scope_end = try_consume(TOKEN_BLOCK_END);
	if (!scope_end) return {};

	return enum_decl;
}

std::optional<Ast_Procedure_Declaration> Parser::parse_procedure()
{
	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) return {};
	auto paren_start = try_consume(TOKEN_PAREN_START);
	if (!paren_start) return {};

	Ast_Procedure_Declaration proc_delc = {};
	proc_delc.ident = Ast_Identifier { ident.value() };
	
	while (true)
	{
		auto param = peek();
		if (!param || param.value().type != TOKEN_IDENT)
			break; // No more params
		consume();

		auto colon = try_consume(TOKEN_COLON);
		if (!colon) return {};
		auto param_type = try_consume(TOKEN_IDENT);
		if (!param_type) return {};

		proc_delc.input_parameters.emplace_back(IdentTypePair{ param.value(), param_type.value() });

		auto token_coma = peek();
		if (!token_coma || token_coma.value().type != TOKEN_COMA)
			break; // No more params
		consume();
	}

	auto paren_end = try_consume(TOKEN_PAREN_END);
	if (!paren_end) return {};

	auto double_colon = try_consume(TOKEN_DOUBLE_COLON);
	if (double_colon)
	{
		auto return_type = try_consume(TOKEN_IDENT);
		if (!return_type) return {};

		proc_delc.return_type = Ast_Identifier { return_type.value() };
	}

	proc_delc.block = parse_block();
	if (proc_delc.block == NULL)
	{
		printf("Func block is null\n"); //@Debug
		return {};
	}

	return proc_delc;
}

Ast_Term* Parser::parse_term()
{
	auto token = peek();
	if (!token) return NULL;

	Ast_Term* term = m_arena.alloc<Ast_Term>();

	switch (token.value().type)
	{
		case TOKEN_STRING:
		{
			term->tag = Ast_Term::Tag::Literal;
			term->_literal = Ast_Literal { token.value() };
			consume();
		} break;
		case TOKEN_NUMBER:
		{
			term->tag = Ast_Term::Tag::Literal;
			term->_literal = Ast_Literal { token.value() };
			consume();
		} break;
		case TOKEN_IDENT:
		{
			auto next_token = peek(1);

			if (next_token && next_token.value().type == TOKEN_PAREN_START)
			{
				term->tag = Ast_Term::Tag::ProcedureCall;
				term->_proc_call = parse_proc_call();
				if (!term->_proc_call) return NULL;
				break;
			}

			term->tag = Ast_Term::Tag::Identifier;
			term->_ident = Ast_Identifier { token.value() };
			consume();
		} break;
		default:
		{
			return NULL;
		}
	}

	return term;
}

Ast_Expression* Parser::parse_expression() //@Incomplete
{
	Ast_Expression* expr = m_arena.alloc<Ast_Expression>();

	Ast_Term* term = parse_term();
	if (term != NULL)
	{
		expr->tag = Ast_Expression::Tag::Term;
		expr->_term = term;
	}
	else return NULL;

	//@Incomplete Expression for now only can be a single Ast_Term
	if (!try_consume(TOKEN_SEMICOLON)) return NULL;

	return expr;
}

Ast_Binary_Expression* Parser::parse_binary_expression() //@Incomplete
{
	Ast_Binary_Expression* bin_expr = m_arena.alloc<Ast_Binary_Expression>();

	return bin_expr;
}

Ast_Block* Parser::parse_block() // { [statement], [statement]... }
{
	auto scope_start = try_consume(TOKEN_BLOCK_START);
	if (!scope_start) return NULL;

	Ast_Block* block = m_arena.alloc<Ast_Block>();

	std::optional<Ast_Statement*> statement = parse_statement();
	while (statement.has_value())
	{
		if (statement.value() == NULL) return NULL;
		block->statements.emplace_back(statement.value());
		statement = parse_statement();
	}

	auto scope_end = try_consume(TOKEN_BLOCK_END);
	if (!scope_end)
	{
		printf("Block missing: }\n");
		return NULL;
	}

	return block;
}

std::optional<Ast_Statement*> Parser::parse_statement() //@Incomplete
{
	// no value -> no stament introducers found
	// value == NULL -> tried to parse but failed
	// value != NULL -> parsed statement

	auto token = peek();
	if (!token) return {};

	Ast_Statement* statement = m_arena.alloc<Ast_Statement>();

	switch (token.value().type)
	{
		case TOKEN_KEYWORD_IF: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::If;
			statement->_if = parse_if();
			if (!statement->_if) return { nullptr };
		} break;
		case TOKEN_KEYWORD_FOR: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::For;
			statement->_for = parse_for();
			if (!statement->_for) return { nullptr };
		} break;
		case TOKEN_KEYWORD_WHILE: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::While;
			statement->_while = parse_while();
			if (!statement->_while) return { nullptr };
		} break;
		case TOKEN_KEYWORD_BREAK:
		{
			statement->tag = Ast_Statement::Tag::Break;
			statement->_break = parse_break();
			if (!statement->_break) return { nullptr };
		} break;
		case TOKEN_KEYWORD_RETURN: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::Return;
			statement->_return = parse_return();
			if (!statement->_return) return { nullptr };
		} break;
		case TOKEN_KEYWORD_CONTINUE:
		{
			statement->tag = Ast_Statement::Tag::Continue;
			statement->_continue = parse_continue();
			if (!statement->_continue) return { nullptr };
		} break;
		case TOKEN_IDENT: //@Incomplete
		{
			auto next_token = peek(1);

			if (next_token && next_token.value().type == TOKEN_PAREN_START)
			{
				statement->tag = Ast_Statement::Tag::ProcedureCall;
				statement->_proc_call = parse_proc_call(); //@Incomplete
				if (!statement->_proc_call) return { nullptr };
			}
			else if (next_token && next_token.value().type == TOKEN_ASSIGN)
			{
				statement->tag = Ast_Statement::Tag::VariableAssignment;
				statement->_var_assignment = parse_var_assignment();
				if (!statement->_var_assignment) return { nullptr };
			}
			else return { nullptr };

		} break;
		case TOKEN_KEYWORD_LET: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::VariableDeclaration;
			statement->_var_declaration = parse_var_declaration();
			if (!statement->_var_declaration) return { nullptr };
		} break;
		default:
		{
			return {};
		}
	}
	
	return statement;
}

Ast_If* Parser::parse_if() //@Incomplete
{
	return NULL;
}

Ast_For* Parser::parse_for() //@Incomplete
{
	return NULL;
}

Ast_While* Parser::parse_while() //@Incomplete
{
	return NULL;
}

Ast_Break* Parser::parse_break() //break;
{
	Ast_Break* _break = m_arena.alloc<Ast_Break>();
	_break->token = peek().value();
	consume();

	if (!try_consume(TOKEN_SEMICOLON)) return NULL;
	return _break;
}

Ast_Return* Parser::parse_return() //return [expr]
{
	Ast_Return* _return = m_arena.alloc<Ast_Return>();
	_return->token = peek().value();
	consume();

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	_return->expr = expr;
	return _return;
}

Ast_Continue* Parser::parse_continue() //continue;
{
	Ast_Continue* _continue = m_arena.alloc<Ast_Continue>();
	_continue->token = peek().value();
	consume();

	if (!try_consume(TOKEN_SEMICOLON)) return NULL;
	return _continue;
}

Ast_Procedure_Call* Parser::parse_proc_call() //@Incomplete
{
	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) return NULL;

	printf("Procedure call ident: (not parsing inner things of it yet) "); 
	error_report_token_ident(ident.value());
	printf("\n");

	return NULL;
}

Ast_Variable_Assignment* Parser::parse_var_assignment() //ident = [expr]
{
	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) return NULL;
	if (!try_consume(TOKEN_ASSIGN)) return NULL;

	Ast_Variable_Assignment* var_assignment = m_arena.alloc<Ast_Variable_Assignment>();
	var_assignment->ident = Ast_Identifier { ident.value() };

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	var_assignment->expr = expr;
	return var_assignment;
}

Ast_Variable_Declaration* Parser::parse_var_declaration() //let ident: type; | let ident: type = [expr]
{
	consume();
	auto ident = try_consume(TOKEN_IDENT); if (!ident) return NULL;
	if (!try_consume(TOKEN_COLON)) return NULL;
	auto type = try_consume(TOKEN_IDENT); if (!type) return NULL;

	Ast_Variable_Declaration* var_declaration = m_arena.alloc<Ast_Variable_Declaration>();
	var_declaration->ident = Ast_Identifier { ident.value() };
	var_declaration->type = Ast_Identifier { type.value() };

	if (!try_consume(TOKEN_ASSIGN))
	{
		if (try_consume(TOKEN_SEMICOLON)) 
			return var_declaration;
		return NULL;
	}

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	var_declaration->expr = expr;
	return var_declaration;
}

std::optional<Ast> Parser::parse()
{
	Ast ast;
	
	while (peek().has_value())
	{
		TokenType type = peek().value().type;
		consume();

		switch (type)
		{
			case TOKEN_KEYWORD_STRUCT: 
			{
				auto struct_decl = parse_struct();
				if (struct_decl.has_value())
					ast.structs.emplace_back(struct_decl.value());
				else return {};
			} break;
			case TOKEN_KEYWORD_ENUM:
			{
				auto enum_decl = parse_enum();
				if (enum_decl.has_value())
					ast.enums.emplace_back(enum_decl.value());
				else return {};
			} break;
			case TOKEN_KEYWORD_FN:
			{
				auto proc_decl = parse_procedure();
				if (proc_decl.has_value())
					ast.procedures.emplace_back(proc_decl.value());
				else return {};
			} break;
		}
	}

	debug_print_definitions(ast);

	return ast;
}

std::optional<Token> Parser::peek(u32 offset)
{
	if (m_index + offset >= m_tokens.size()) return {};
	else return m_tokens[m_index + offset];
}

std::optional<Token> Parser::try_consume(TokenType token_type)
{
	auto token = peek();
	if (token && token.value().type == token_type)
	{
		consume();
		return token;
	}
	return {};
}

void Parser::consume()
{
	m_index += 1;
}
