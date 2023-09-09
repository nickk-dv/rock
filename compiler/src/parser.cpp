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

void debug_print_parameter(const ParameterInfo& parameter, bool tab = true, bool newline = true)
{
	if (tab) debug_print_tab();
	error_report_token_ident(parameter.name_ident);
	printf(": ");
	error_report_token_ident(parameter.type_ident);
	if (newline) printf(",\n");
}

void debug_print_variant(const VariantInfo& variant, bool tab = true)
{
	if (tab) debug_print_tab();
	error_report_token_ident(variant.name_ident);
	printf(",\n");
}

void debug_print_definitions()
{
	printf("[STRUCTS]\n\n");
	for (const StructInfo& def : definitions_struct)
	{
		printf("struct ");
		error_report_token_ident(def.type_ident);
		printf("\n{\n");

		for (const ParameterInfo& parameter : def.fields)
			debug_print_parameter(parameter);
		printf("}\n\n");
	}

	printf("[ENUMS]\n\n");
	for (const EnumInfo& def : definitions_enum)
	{
		printf("enum ");
		error_report_token_ident(def.type_ident);
		printf("\n{\n");
		
		for (const VariantInfo& variant : def.variants)
			debug_print_variant(variant);
		printf("}\n\n");
	}

	printf("[FUNCTIONS]\n\n");
	for (const FunctionInfo& def : definitions_fn)
	{
		printf("fn ");
		error_report_token_ident(def.name_ident);

		printf("(");
		size_t size = def.parameters.size();
		size_t counter = 0;
		for (const ParameterInfo& parameter : def.parameters)
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
	auto token_struct_type = peek(); // Type
	if (!token_struct_type || token_struct_type.value().type != TOKEN_IDENT)
		return {};
	consume();

	auto token_scope_start = peek(); // {
	if (!token_scope_start || token_scope_start.value().type != TOKEN_BLOCK_START)
		return {};
	consume();

	Ast_Struct_Declaration struct_decl = {};
	struct_decl.type = token_struct_type.value();

	while (true)
	{
		auto token_field = peek(); // field
		if (!token_field || token_field.value().type != TOKEN_IDENT)
			break; // No more fields
		else consume();


		auto token_colon = peek(); // :
		if (!token_colon || token_colon.value().type != TOKEN_COLON)
			return {};
		consume();

		auto token_field_type = peek(); // Type
		if (!token_field_type || token_field_type.value().type != TOKEN_IDENT)
			return {};
		consume();

		struct_decl.fields.emplace_back(IdentTypePair { token_field.value(), token_field_type.value() });

		auto token_coma = peek(); // ,
		if (token_coma && token_coma.value().type == TOKEN_COMA)
			consume();
		else break; // No more fields
	}

	auto token_scope_end = peek(); // }
	if (!token_scope_end || token_scope_end.value().type != TOKEN_BLOCK_END)
		return {};
	consume();

	return struct_decl;
}

std::optional<Ast_Enum_Declaration> Parser::parse_enum()
{
	auto token_enum_type = peek(); // Type
	if (!token_enum_type || token_enum_type.value().type != TOKEN_IDENT)
		return {};
	consume();

	auto token_scope_start = peek(); // {
	if (!token_scope_start || token_scope_start.value().type != TOKEN_BLOCK_START)
		return {};
	consume();

	Ast_Enum_Declaration enum_decl = {};
	enum_decl.type = token_enum_type.value();

	while (true)
	{
		auto token_variant = peek(); // variant
		if (!token_variant || token_variant.value().type != TOKEN_IDENT)
			break; // No more variants
		else consume();

		//@Notice no type is specified for the enum variant
		//might support typed numeric enums with literal values
		enum_decl.variants.emplace_back(IdentTypePair { token_variant.value(), {} });

		auto token_coma = peek(); // ,
		if (token_coma && token_coma.value().type == TOKEN_COMA)
			consume();
		else break; // No more variants
	}

	auto token_scope_end = peek(); // }
	if (!token_scope_end || token_scope_end.value().type != TOKEN_BLOCK_END)
		return {};
	consume();

	return enum_decl;
}

std::optional<Ast_Procedure_Declaration> Parser::parse_procedure()
{
	auto token_name = peek(); // name
	if (!token_name || token_name.value().type != TOKEN_IDENT)
		return {};
	consume();

	auto token_paren_start = peek(); // (
	if (!token_paren_start || token_paren_start.value().type != TOKEN_PARENTHESIS_START)
		return {};
	consume();

	Ast_Procedure_Declaration proc_delc = {};
	proc_delc.ident = token_name.value();
	
	while (true)
	{
		auto token_param = peek(); // param
		if (!token_param || token_param.value().type != TOKEN_IDENT)
			break; // No more params
		else consume();

		auto token_colon = peek(); // :
		if (!token_colon || token_colon.value().type != TOKEN_COLON)
			return {};
		consume();

		auto token_param_type = peek(); // Type
		if (!token_param_type || token_param_type.value().type != TOKEN_IDENT)
			return {};
		consume();

		proc_delc.input_parameters.emplace_back(IdentTypePair{ token_param.value(), token_param_type.value() });

		auto token_coma = peek(); // ,
		if (token_coma && token_coma.value().type == TOKEN_COMA)
			return {};
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
			return {};
		consume();

		proc_delc.return_type = token_return_type.value();
	}

	auto token_scope_start = peek(); // {
	if (!token_scope_start || token_scope_start.value().type != TOKEN_BLOCK_START)
		return {};
	consume();

	proc_delc.block = parse_block();
	if (proc_delc.block == NULL) return {};

	return proc_delc;
}

Ast_Block* Parser::parse_block()
{
	Ast_Block* block = m_arena.alloc<Ast_Block>();

	Ast_Statement* statement = parse_statement();
	while (statement != NULL)
	{
		block->statements.emplace_back(statement);
		statement = parse_statement();
	}

	auto token_scope_end = peek(); // }
	if (!token_scope_end || token_scope_end.value().type != TOKEN_BLOCK_END)
		return NULL;
	consume();

	return block;
}

Ast_Statement* Parser::parse_statement()
{
	Ast_Statement* statement = m_arena.alloc<Ast_Statement>();

	//@Incomplete always produces null statement
	return NULL;

	return statement;
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

	return ast;

	//@Debug printing
	debug_print_definitions();
	
	for (const FunctionInfo& fn : definitions_fn)
	{
		printf("function:");
		error_report_token_ident(fn.name_ident);
		printf("\n");

		seek(fn.block_start_token_index);
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

void Parser::seek(size_t index)
{
	m_index = index;
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
