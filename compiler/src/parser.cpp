#include "parser.h"

#include "common.h"
#include "error.h"
#include "token.h"

//@Notice: this is temporary prototype representation of types and function declarations
//proper symbol table management will need to be compared to performance of this approach on bigger programs 
struct ParameterInfo
{
	Token name_ident;
	Token type_ident;
};

struct StructInfo
{
	Token type_ident;
	std::vector<ParameterInfo> fields;
};

struct VariantInfo
{
	Token name_ident;
};

struct EnumInfo
{
	Token type_ident;
	std::vector<VariantInfo> variants;
};

struct FunctionInfo
{
	Token name_ident;
	size_t block_start_token_index;
	std::optional<Token> return_type;
	std::vector<ParameterInfo> parameters;
};

static std::vector<StructInfo> definitions_struct;
static std::vector<EnumInfo> definitions_enum;
static std::vector<FunctionInfo> definitions_fn;

void debug_print_tab()
{
	printf("   ");
}

void debug_print_parameter(const IdentTypePair& parameter, bool tab = true, bool newline = true)
{
	if (tab) debug_print_tab();
	error_report_token_ident(parameter.ident);
	printf(": ");
	error_report_token_ident(parameter.type);
	if (newline) printf(",\n");
}

void debug_print_variant(const VariantInfo& variant, bool tab = true)
{
	if (tab) debug_print_tab();
	error_report_token_ident(variant.name_ident);
	printf(",\n");
}

void debug_print_definitions(const Ast& ast)
{
	printf("[STRUCTS]\n\n");
	for (const auto& def : ast.structs)
	{
		printf("struct ");
		error_report_token_ident(def.type);
		printf("\n{\n");

		for (const auto& parameter : def.fields)
			debug_print_parameter(parameter);
		printf("}\n\n");
	}

	printf("[ENUMS]\n\n");
	for (const auto& def : ast.enums)
	{
		printf("enum ");
		error_report_token_ident(def.type);
		printf("\n{\n");
		
		for (const auto& variant : def.variants)
			debug_print_parameter(variant);
		printf("}\n\n");
	}

	printf("[FUNCTIONS]\n\n");
	for (const auto& def : ast.procedures)
	{
		printf("fn ");
		error_report_token_ident(def.ident);

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
			printf(" -> "); 
			error_report_token_ident(def.return_type.value());
		}
		printf("\n\n");
	}
}
//@Notice: see above message

enum BinaryOp
{
	BINARY_OP_ASSIGN,
	BINARY_OP_PLUS,
	BINARY_OP_MINUS,
	BINARY_OP_TIMES,
	BINARY_OP_DIV,
	BINARY_OP_MOD,
	BINARY_OP_BITWISE_AND,
	BINARY_OP_BITWISE_OR,
	BINARY_OP_BITWISE_XOR,
	BINARY_OP_PLUS_EQUALS,
	BINARY_OP_MINUS_EQUALS,
	BINARY_OP_TIMES_EQUALS,
	BINARY_OP_DIV_EQUALS,
	BINARY_OP_MOD_EQUALS,
	BINARY_OP_BITWISE_AND_EQUALS,
	BINARY_OP_BITWISE_OR_EQUALS,
	BINARY_OP_BITWISE_XOR_EQUALS,
	BINARY_OP_BITSHIFT_LEFT,
	BINARY_OP_BITSHIFT_RIGHT,
	BINARY_OP_LESS,
	BINARY_OP_GREATER,
	BINARY_OP_LESS_EQUALS,
	BINARY_OP_GREATER_EQUALS,
	BINARY_OP_IS_EQUAL,
	BINARY_OP_NOT_EQUAL,
	BINARY_OP_LOGIC_AND,
	BINARY_OP_LOGIC_OR
};

enum UnaryOp
{
	UNARY_OP_BITWISE_NOT,
	UNARY_OP_LOGIC_NOT,
};

struct NodeTerm
{
	Token number_literal;
};

struct NodeVariableDeclaration
{
	Token ident;
	Token type;
	NodeTerm term;
};

struct NodeReturn
{
	NodeTerm term;
};

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
	struct_decl.type = type.value();

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
	enum_decl.type = type.value();

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
	auto paren_start = try_consume(TOKEN_PARENTHESIS_START);
	if (!paren_start) return {};

	Ast_Procedure_Declaration proc_delc = {};
	proc_delc.ident = ident.value();
	
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

	auto paren_end = try_consume(TOKEN_PARENTHESIS_END);
	if (!paren_end) return {};

	auto arrow = try_consume(TOKEN_ARROW);
	if (arrow)
	{
		auto return_type = try_consume(TOKEN_IDENT);
		if (!return_type) return {};

		proc_delc.return_type = return_type.value();
	}

	proc_delc.block = parse_block();
	if (proc_delc.block == NULL)
	{
		printf("Func block is null\n"); //@Debug
		return {};
	}

	return proc_delc;
}

Ast_Block* Parser::parse_block()
{
	auto scope_start = try_consume(TOKEN_BLOCK_START);
	if (!scope_start) return NULL;

	Ast_Block* block = m_arena.alloc<Ast_Block>();

	Ast_Statement* statement = parse_statement();
	while (statement != NULL)
	{
		block->statements.emplace_back(statement);
		statement = parse_statement();
	}

	auto scope_end = try_consume(TOKEN_BLOCK_END);
	if (!scope_end) return NULL;

	return block;
}

Ast_Statement* Parser::parse_statement()
{
	Ast_Statement* statement = m_arena.alloc<Ast_Statement>();

	printf("First token of parse_statement \n");
	error_report_token(peek().value());

	auto token = peek();
	if (!token) return NULL;

	switch (token.value().type)
	{
		case TOKEN_KEYWORD_IF: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::If;
			statement->_if = parse_if();
			if (!statement->_if) return NULL;
		} break;
		case TOKEN_KEYWORD_FOR: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::For;
			statement->_for = parse_for();
			if (!statement->_for) return NULL;
		} break;
		case TOKEN_KEYWORD_WHILE: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::While;
			statement->_while = parse_while();
			if (!statement->_while) return NULL;
		} break;
		case TOKEN_KEYWORD_BREAK:
		{
			statement->tag = Ast_Statement::Tag::Break;
			statement->_break = parse_break();
			if (!statement->_break) return NULL;
		} break;
		case TOKEN_KEYWORD_RETURN: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::Return;
			statement->_return = parse_return();
			if (!statement->_return) return NULL;
		} break;
		case TOKEN_KEYWORD_CONTINUE:
		{
			statement->tag = Ast_Statement::Tag::Continue;
			statement->_continue = parse_continue();
			if (!statement->_continue) return NULL;
		} break;
		case TOKEN_IDENT: //@Incomplete
		{
			auto next_token = peek(1);

			if (next_token && next_token.value().type == TOKEN_PARENTHESIS_START)
			{
				statement->tag = Ast_Statement::Tag::ProcedureCall;
				statement->_proc_call = parse_proc_call(); //@Incomplete
				if (!statement->_proc_call) return NULL;
				break;
			}

			if (next_token && next_token.value().type == TOKEN_ASSIGN)
			{
				statement->tag = Ast_Statement::Tag::VariableAssignment;
				statement->_var_assignment = parse_var_assignment(); //@Incomplete
				if (!statement->_var_assignment) return NULL;
				break;
			}

			return NULL;

		} break;
		case TOKEN_KEYWORD_LET: //@Incomplete
		{
			statement->tag = Ast_Statement::Tag::VariableDeclaration;
			statement->_var_declaration = parse_var_declaration();
			if (!statement->_var_declaration) return NULL;
		} break;
		default:
		{
			return NULL;
		}
	}
	
	return statement;
}

Ast_If* Parser::parse_if()
{
	return NULL;
}

Ast_For* Parser::parse_for()
{
	return NULL;
}

Ast_While* Parser::parse_while()
{
	return NULL;
}

Ast_Break* Parser::parse_break()
{
	Ast_Break* _break = m_arena.alloc<Ast_Break>();
	_break->token = try_consume(TOKEN_KEYWORD_BREAK).value();
	if (!try_consume(TOKEN_SEMICOLON)) return NULL;
	return _break;
}

Ast_Return* Parser::parse_return()
{
	return NULL;
}

Ast_Continue* Parser::parse_continue()
{
	Ast_Continue* _continue = m_arena.alloc<Ast_Continue>();
	_continue->token = try_consume(TOKEN_KEYWORD_CONTINUE).value();
	if (!try_consume(TOKEN_SEMICOLON)) return NULL;
	return _continue;
}

Ast_Procedure_Call* Parser::parse_proc_call()
{
	return NULL;
}

Ast_Variable_Assignment* Parser::parse_var_assignment()
{
	return NULL;
}

Ast_Variable_Declaration* Parser::parse_var_declaration()
{
	return NULL;
}

Ast Parser::parse()
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
				else return ast;
			} break;
			case TOKEN_KEYWORD_ENUM:
			{
				auto enum_decl = parse_enum();
				if (enum_decl.has_value())
					ast.enums.emplace_back(enum_decl.value());
				else return ast;
			} break;
			case TOKEN_KEYWORD_FN:
			{
				auto proc_decl = parse_procedure();
				if (proc_decl.has_value())
					ast.procedures.emplace_back(proc_decl.value());
				else return ast;
			} break;
		}
	}

	debug_print_definitions(ast);

	return ast;

	//@Debug printing
	for (const FunctionInfo& fn : definitions_fn)
	{
		printf("function:");
		error_report_token_ident(fn.name_ident);
		printf("\n");

		consume();

		auto token_scope_end = peek(); // }
		if (token_scope_end && token_scope_end.value().type == TOKEN_BLOCK_END)
			continue;

		//@Notice this is temporary, assuming no nested blocks to parse all inner statements of the fn
		while(peek().has_value() && peek().value().type != TOKEN_BLOCK_END)
		{
			auto token_statement_intro = peek();
			consume();

			if (token_statement_intro)
			switch(token_statement_intro.value().type)
			{
				case TOKEN_KEYWORD_LET:
				{
					auto token_variable_ident = peek(); // ident
					if (!token_variable_ident || token_variable_ident.value().type != TOKEN_IDENT)
						exit_error();
					consume();

					auto token_colon = peek(); // :
					if (!token_colon || token_colon.value().type != TOKEN_COLON)
						exit_error();
					consume();
			
					auto token_variable_type = peek(); // Type
					if (!token_variable_type || token_variable_type.value().type != TOKEN_IDENT)
						exit_error();
					consume();

					NodeVariableDeclaration declaration = {};
					declaration.ident = token_variable_ident.value();
					declaration.type = token_variable_type.value();

					auto token_semi_default_init = peek(); // ;
					if (token_semi_default_init && token_semi_default_init.value().type == TOKEN_SEMICOLON)
					{
						//@Debug
						error_report_token(declaration.ident);
						printf("Debug parsed variable: with default init\n");

						consume();
						break;
					}

					auto token_assign = peek(); // =
					if (!token_assign || token_assign.value().type != TOKEN_ASSIGN)
						exit_error();
					consume();

					auto token_number = peek(); // number term @Notice using a number literal instead of expression chain
					if (!token_number || token_number.value().type != TOKEN_NUMBER)
						exit_error();
					consume();

					declaration.term.number_literal = token_number.value();

					auto token_semi = peek(); // ;
					if (!token_semi || token_semi.value().type != TOKEN_SEMICOLON)
						exit_error();
					consume();

					//@Debug
					error_report_token(declaration.ident);
					printf("Debug parsed variable: with value %llu \n", declaration.term.number_literal.integer_value);
				} break;
				case TOKEN_KEYWORD_RETURN:
				{
					printf("Debug parsed return \n");

					auto token_number = peek(); // number term @Notice using a number literal instead of expression chain
					if (!token_number || token_number.value().type != TOKEN_NUMBER)
						exit_error();
					consume();

					NodeReturn fn_return = {};
					fn_return.term.number_literal = token_number.value();

					auto token_semi = peek(); // ;
					if (!token_semi || token_semi.value().type != TOKEN_SEMICOLON)
						exit_error();
					consume();
				} break;
			}
		}

		printf("\n");
	}

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

void Parser::exit_error()
{
	error_report(PARSE_ERROR_TEST, peek().value_or(Token {}));
	exit(EXIT_FAILURE);
}
