
struct Parser
{
	Parser(std::vector<Token> tokens);

	std::optional<Ast> parse();

	std::optional<Ast_Struct_Declaration> parse_struct();
	std::optional<Ast_Enum_Declaration> parse_enum();
	std::optional<Ast_Procedure_Declaration> parse_procedure();
	Ast_Term* parse_term();
	Ast_Expression* parse_expression();
	Ast_Expression* parse_sub_expression(u32 min_precedence = 0);
	Ast_Block* parse_block();
	std::optional<Ast_Statement*> parse_statement();
	Ast_If* parse_if();
	Ast_Else* parse_else();
	Ast_For* parse_for();
	Ast_Break* parse_break();
	Ast_Return* parse_return();
	Ast_Continue* parse_continue();
	Ast_Procedure_Call* parse_proc_call();
	Ast_Variable_Assignment* parse_var_assignment();
	Ast_Variable_Declaration* parse_var_declaration();
	std::optional<Token> peek(u32 offset = 0);
	std::optional<Token> try_consume(TokenType token_type);
	void consume();
	void debug_print_ast(Ast* ast);
	void debug_print_expr(Ast_Expression* expr, u32 depth);
	void debug_print_binary_expr(Ast_Binary_Expression* expr, u32 depth);

	const std::vector<Token> m_tokens;
	size_t m_index = 0;
	ArenaAllocator m_arena;
};

Parser::Parser(std::vector<Token> tokens) 
	: m_tokens(std::move(tokens)), 
	  m_arena(1024 * 1024 * 4) {}

std::optional<Ast> Parser::parse()
{
	Ast ast;

	while (peek().has_value())
	{
		Token token = peek().value();
		consume();

		switch (token.type)
		{
			case TOKEN_KEYWORD_STRUCT:
			{
				auto struct_decl = parse_struct();
				if (!struct_decl) return {};
				ast.structs.emplace_back(struct_decl.value());
			} break;
			case TOKEN_KEYWORD_ENUM:
			{
				auto enum_decl = parse_enum();
				if (!enum_decl) return {};
				ast.enums.emplace_back(enum_decl.value());
			} break;
			case TOKEN_KEYWORD_FN:
			{
				auto proc_decl = parse_procedure();
				if (!proc_decl) return {};
				ast.procedures.emplace_back(proc_decl.value());
			} break;
			default:
			{
				if (m_tokens[m_index - 1].type == TOKEN_INPUT_END) return ast;
				printf("Expected fn, enum or struct declaration.\n");
				error_report_token(m_tokens[m_index - 1]); //@Hack reporting on prev token, current one is consumed above
				return {};
			} break;
		}
	}

	return ast;
}

std::optional<Ast_Struct_Declaration> Parser::parse_struct()
{
	auto type = try_consume(TOKEN_IDENT); 
	if (!type) { printf("Expected an identifier.\n"); return {}; }

	Ast_Struct_Declaration decl = {};
	decl.type = Ast_Identifier { type.value() };
	
	if (!try_consume(TOKEN_BLOCK_START)) { printf("Expected opening '{'.\n"); return {}; }
	while (true)
	{
		auto field = try_consume(TOKEN_IDENT);
		if (!field) break;
		if (!try_consume(TOKEN_COLON)) { printf("Expected ':' with type identifier.\n"); return {}; }
		auto field_type = try_consume(TOKEN_IDENT);
		if (!field_type) { printf("Expected type idenifier.\n"); return {}; }

		decl.fields.emplace_back(IdentTypePair { field.value(), field_type.value() });
		if (!try_consume(TOKEN_COMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { printf("Struct Expected closing '}'.\n"); return {}; }

	return decl;
}

std::optional<Ast_Enum_Declaration> Parser::parse_enum()
{
	auto type = try_consume(TOKEN_IDENT); 
	if (!type) { printf("Expected an identifier.\n"); return {}; }

	Ast_Enum_Declaration decl = {};
	decl.type = Ast_Identifier { type.value() };

	if (!try_consume(TOKEN_BLOCK_START)) { printf("Expected opening '{'.\n"); return {}; }
	while (true)
	{
		auto variant = try_consume(TOKEN_IDENT);
		if (!variant) break;

		decl.variants.emplace_back(IdentTypePair { variant.value(), {} }); //@Notice type is empty token, might support typed enums
		if (!try_consume(TOKEN_COMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { printf("Enum Expected closing '}'.\n"); return {}; }

	return decl;
}

std::optional<Ast_Procedure_Declaration> Parser::parse_procedure()
{
	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) { printf("Expected an identifier.\n"); return {}; }

	Ast_Procedure_Declaration decl = {};
	decl.ident = Ast_Identifier { ident.value() };
	
	if (!try_consume(TOKEN_PAREN_START)) { printf("Expected opening '('.\n"); return {}; }
	while (true)
	{
		auto param = try_consume(TOKEN_IDENT);
		if (!param) break;
		if (!try_consume(TOKEN_COLON)) { printf("Expected ':' with type identifier.\n"); return {}; }
		auto param_type = try_consume(TOKEN_IDENT);
		if (!param_type) { printf("Expected type idenifier.\n"); return {}; }

		decl.input_parameters.emplace_back(IdentTypePair { param.value(), param_type.value() });
		if (!try_consume(TOKEN_COMA)) break;
	}
	if (!try_consume(TOKEN_PAREN_END)) { printf("Expected closing ')'.\n"); return {}; }

	if (try_consume(TOKEN_DOUBLE_COLON))
	{
		auto return_type = try_consume(TOKEN_IDENT);
		if (!return_type) { printf("Expected return type identifier.\n"); return {}; }
		decl.return_type = Ast_Identifier { return_type.value() };
	}

	Ast_Block* block = parse_block();
	if (block == NULL) return {};
	decl.block = block;

	return decl;
}

Ast_Term* Parser::parse_term()
{
	Ast_Term* term = m_arena.alloc<Ast_Term>();
	Token token = peek().value();

	switch (token.type)
	{
		case TOKEN_STRING:
		{
			term->tag = Ast_Term::Tag::Literal;
			term->_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_NUMBER:
		{
			term->tag = Ast_Term::Tag::Literal;
			term->_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_KEYWORD_TRUE:
		{
			token.type = TOKEN_BOOL_LITERAL;
			token.bool_value = true;
			term->tag = Ast_Term::Tag::Literal;
			term->_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_KEYWORD_FALSE:
		{
			token.type = TOKEN_BOOL_LITERAL;
			token.bool_value = false;
			term->tag = Ast_Term::Tag::Literal;
			term->_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_IDENT:
		{
			auto next = peek(1);
			if (next && next.value().type == TOKEN_PAREN_START)
			{
				Ast_Procedure_Call* proc_call = parse_proc_call();
				if (!proc_call) return NULL;
				term->tag = Ast_Term::Tag::ProcedureCall;
				term->_proc_call = proc_call;
				break;
			}

			term->tag = Ast_Term::Tag::Identifier;
			term->_ident = Ast_Identifier { token };
			consume();
		} break;
		default:
		{
			//@Bug failing to parse a term of proc call when it has no arguments
			error_report_token(token);
			printf("Failed to parse expression term.\n");
			return {}; //
		}
	}

	return term;
}

Ast_Expression* Parser::parse_expression()
{
	Ast_Expression* expr = parse_sub_expression();
	if (!expr) return NULL;
	if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected ';' after expression.\n"); return NULL; }
	return expr;
}

Ast_Expression* Parser::parse_sub_expression(u32 min_precedence) //@Incomplete think about handling parens ( )
{
	Ast_Term* term_lhs = parse_term();
	if (!term_lhs) return NULL;

	Ast_Expression* expr_lhs = m_arena.alloc<Ast_Expression>();
	expr_lhs->tag = Ast_Expression::Tag::Term;
	expr_lhs->_term = term_lhs;

	while (true)
	{
		auto token_op = peek();
		if (!token_op) break;
		BinaryOp op = ast_binary_op_from_token(token_op.value().type);
		if (op == BINARY_OP_ERROR) break;
		u32 prec = ast_binary_op_precedence(op);
		if (prec < min_precedence) break;
		consume();

		u32 next_min_prec = prec + 1;
		Ast_Expression* expr_rhs = parse_sub_expression(next_min_prec);
		if (expr_rhs == NULL) return NULL;

		Ast_Expression* expr_lhs2 = m_arena.alloc<Ast_Expression>(); //@Hacks
		expr_lhs2->tag = expr_lhs->tag;
		expr_lhs2->_term = expr_lhs->_term;
		expr_lhs2->_bin_expr = expr_lhs->_bin_expr;

		Ast_Binary_Expression* bin_expr = m_arena.alloc<Ast_Binary_Expression>();
		bin_expr->op = op;
		bin_expr->left = expr_lhs2; //Assembling the magic tree
		bin_expr->right = expr_rhs;

		expr_lhs->tag = Ast_Expression::Tag::BinaryExpression;
		expr_lhs->_bin_expr = bin_expr;
	}

	return expr_lhs;
}

Ast_Block* Parser::parse_block()
{
	if (!try_consume(TOKEN_BLOCK_START)) { printf("Expected code block that starts with '{'.\n"); return NULL; }
	
	Ast_Block* block = m_arena.alloc<Ast_Block>();
	std::optional<Ast_Statement*> statement = parse_statement();

	while (statement.has_value())
	{
		if (statement.value() == NULL) return NULL;
		block->statements.emplace_back(statement.value());
		statement = parse_statement();
	}
	if (!try_consume(TOKEN_BLOCK_END)) { printf("Expected code block to end with '}'.\n"); return NULL; }
	
	return block;
}

std::optional<Ast_Statement*> Parser::parse_statement()
{
	//@Hack '}' check prevents wasted allocation when its the last statement
	// maybe restructure block / statement interactions
	// no value -> no stament introducers found
	// value == NULL -> tried to parse but failed
	// value != NULL -> parsed statement
	auto token = peek();
	if (!token || token.value().type == TOKEN_BLOCK_END) return {};

	Ast_Statement* statement = m_arena.alloc<Ast_Statement>();

	switch (token.value().type)
	{
		case TOKEN_KEYWORD_IF:
		{
			statement->tag = Ast_Statement::Tag::If;
			statement->_if = parse_if();
			if (!statement->_if) return { nullptr };
		} break;
		case TOKEN_KEYWORD_FOR:
		{
			statement->tag = Ast_Statement::Tag::For;
			statement->_for = parse_for();
			if (!statement->_for) return { nullptr };
		} break;
		case TOKEN_KEYWORD_BREAK:
		{
			statement->tag = Ast_Statement::Tag::Break;
			statement->_break = parse_break();
			if (!statement->_break) return { nullptr };
		} break;
		case TOKEN_KEYWORD_RETURN:
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
		case TOKEN_IDENT:
		{
			auto next = peek(1);
			if (!next) { printf("Expected identifier to be followed by a valid statement ':' '=' '('.\n"); return { nullptr }; }

			if (next.value().type == TOKEN_COLON)
			{
				statement->tag = Ast_Statement::Tag::VariableDeclaration;
				statement->_var_declaration = parse_var_declaration();
				if (!statement->_var_declaration) return { nullptr };
			}
			else if (next.value().type == TOKEN_ASSIGN)
			{
				statement->tag = Ast_Statement::Tag::VariableAssignment;
				statement->_var_assignment = parse_var_assignment();
				if (!statement->_var_assignment) return { nullptr };
			}
			else if (next.value().type == TOKEN_PAREN_START)
			{
				statement->tag = Ast_Statement::Tag::ProcedureCall;
				statement->_proc_call = parse_proc_call();
				if (!statement->_proc_call) return { nullptr };
				if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected procedure call statement to be followed by ';'.\n"); return { nullptr }; } //@Hack the proc_call statament requires ';' at the end
			}
			else { printf("Expected identifier to be followed by a valid statement: ':' '=' '('.\n"); return { nullptr }; }
		} break;
		default: { printf("Invalid token at the start of a statement.\n"); return { nullptr }; }
	}
	
	return statement;
}

Ast_If* Parser::parse_if()
{
	Ast_If* _if = m_arena.alloc<Ast_If>();
	_if->token = peek().value();
	consume();

	Ast_Expression* expr = parse_sub_expression();
	if (!expr) return NULL;
	_if->expr = expr;

	Ast_Block* block = parse_block();
	if (!block) return NULL;
	_if->block = block;

	auto next = peek();
	if (next && next.value().type == TOKEN_KEYWORD_ELSE)
	{
		Ast_Else* _else = parse_else();
		if (!_else) return NULL;
		_if->_else = _else;
	}

	return _if;
}

Ast_Else* Parser::parse_else()
{
	Ast_Else* _else = m_arena.alloc<Ast_Else>();
	_else->token = peek().value();
	consume();

	auto next = peek();
	if (!next) { printf("Expected 'else' to be followed by 'if' or a code block '{ ... }'.\n"); return NULL; }

	if (next.value().type == TOKEN_KEYWORD_IF)
	{
		Ast_If* _if = parse_if();
		if (!_if) return NULL;
		_else->tag = Ast_Else::Tag::If;
		_else->_if = _if;
	}
	else if (next.value().type == TOKEN_BLOCK_START)
	{
		Ast_Block* block = parse_block();
		if (!block) return NULL;
		_else->tag = Ast_Else::Tag::Block;
		_else->_block = block;
	}
	else { printf("Expected 'else' to be followed by 'if' or a code block '{ ... }'.\n"); return NULL; }

	return _else;
}

Ast_For* Parser::parse_for()
{
	Ast_For* _for = m_arena.alloc<Ast_For>();
	_for->token = peek().value();
	consume();

	//@Design
	// 0 expressions = infinite loop
	// 1 var declaration; 1 conditional expr; 1 post expr;
	// 1 conditional expr; 1 post expr;
	// 1 conditional expr;
	
	auto curr = peek();
	auto next = peek(1);

	//infinite loop
	if (curr && curr.value().type == TOKEN_BLOCK_START)
	{
		Ast_Block* block = parse_block();
		if (!block) return NULL;
		_for->block = block;
		
		return _for;
	}

	//var declaration detection
	if (curr && curr.value().type == TOKEN_IDENT && next && next.value().type == TOKEN_COLON)
	{
		Ast_Variable_Declaration* var_declaration = parse_var_declaration();
		if (!var_declaration) return NULL;
		_for->var_declaration = var_declaration;
	}

	//conditional expr must exist
	Ast_Expression* condition_expr = parse_sub_expression();
	if (!condition_expr) { printf("Expected a valid conditional expression in a for loop.\n"); return NULL; }
	_for->condition_expr = condition_expr;

	//post expr might exist
	if (try_consume(TOKEN_SEMICOLON))
	{
		//@Issue post expr can be a Ident or Literal Term which doesnt do anything?
		Ast_Expression* post_expr = parse_sub_expression();
		if (!post_expr) return NULL;
		_for->post_expr = post_expr;
	}

	Ast_Block* block = parse_block();
	if (!block) return NULL;
	_for->block = block;

	return _for;
}

Ast_Break* Parser::parse_break()
{
	Ast_Break* _break = m_arena.alloc<Ast_Break>();
	_break->token = peek().value();
	consume();

	if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected ';' after break statement.\n"); return NULL; }
	return _break;
}

Ast_Return* Parser::parse_return()
{
	Ast_Return* _return = m_arena.alloc<Ast_Return>();
	_return->token = peek().value();
	consume();

	if (try_consume(TOKEN_SEMICOLON))
		return _return;

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	_return->expr = expr;
	return _return;
}

Ast_Continue* Parser::parse_continue()
{
	Ast_Continue* _continue = m_arena.alloc<Ast_Continue>();
	_continue->token = peek().value();
	consume();

	if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected ';' after continue statement.\n"); return NULL; }
	return _continue;
}

Ast_Procedure_Call* Parser::parse_proc_call()
{
	auto ident = try_consume(TOKEN_IDENT); //checked before, cannot error
	if (!ident) return NULL;
	if (!try_consume(TOKEN_PAREN_START)) return NULL;

	Ast_Procedure_Call* proc_call = m_arena.alloc<Ast_Procedure_Call>();
	proc_call->ident = Ast_Identifier { ident.value() };

	while (true)
	{
		Ast_Expression* param_expr = parse_sub_expression();
		if (!param_expr) break;
		proc_call->input_expressions.emplace_back(param_expr);

		if (!try_consume(TOKEN_COMA)) break;
	}

	if (!try_consume(TOKEN_PAREN_END)) { printf("Expected closing ')' after procedure call statement.\n"); return NULL; }
	return proc_call;
}

Ast_Variable_Assignment* Parser::parse_var_assignment()
{
	auto ident = try_consume(TOKEN_IDENT); //checked before, cannot error
	if (!ident) return NULL;
	if (!try_consume(TOKEN_ASSIGN)) return NULL;

	Ast_Variable_Assignment* var_assignment = m_arena.alloc<Ast_Variable_Assignment>();
	var_assignment->ident = Ast_Identifier { ident.value() };

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	var_assignment->expr = expr;
	return var_assignment;
}

Ast_Variable_Declaration* Parser::parse_var_declaration()
{
	auto ident = try_consume(TOKEN_IDENT); //checked before, cannot error
	if (!ident) return NULL;
	if (!try_consume(TOKEN_COLON)) return NULL;

	Ast_Variable_Declaration* var_declaration = m_arena.alloc<Ast_Variable_Declaration>();
	var_declaration->ident = Ast_Identifier { ident.value() };

	auto type = try_consume(TOKEN_IDENT); //explicit type
	if (type) var_declaration->type = Ast_Identifier { type.value() };

	if (!try_consume(TOKEN_ASSIGN)) //default init
	{
		bool has_semicolon = try_consume(TOKEN_SEMICOLON).has_value();
		if (type && has_semicolon) //default init must have a type and ';'
			return var_declaration;

		if (!type && has_semicolon) printf("No type before ;.\n");
		else if (type && !has_semicolon) printf("No ; after type.\n");
		else if (!type && !has_semicolon) printf("No type or ; specified.\n");

		return NULL;
	}

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	var_declaration->expr = expr;
	return var_declaration;
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

void Parser::debug_print_ast(Ast* ast)
{
	printf("\n");
	printf("[AST]\n\n");

	u32 depth = 1;

	for (const auto& proc : ast->procedures)
	{
		printf("ident:  ");
		error_report_token_ident(proc.ident.token, true);
		printf("params: ");
		if (proc.input_parameters.empty())
		{
			printf("--\n");
		}
		else
		{
			printf("(");
			u32 count = 0;
			for (const auto& param : proc.input_parameters)
			{
				if (count != 0)
				printf(", ");
				count += 1;
				error_report_token_ident(param.ident.token);
				printf(": ");
				error_report_token_ident(param.type.token);
			}
			printf(")\n");
		}

		printf("return: ");
		if (proc.return_type.has_value())
		{
			error_report_token_ident(proc.return_type.value().token, true);
		}
		else printf("--\n");

		printf("Block\n");
		for (Ast_Statement* statement : proc.block->statements)
		{
			printf("|___");
			switch (statement->tag)
			{
				case Ast_Statement::Tag::If:
				{
					printf("If\n");
				} break;
				case Ast_Statement::Tag::For:
				{
					printf("For\n");
				} break;
				case Ast_Statement::Tag::While:
				{
					printf("While\n");
				} break;
				case Ast_Statement::Tag::Break:
				{
					printf("Break\n");
				} break;
				case Ast_Statement::Tag::Return:
				{
					printf("Return\n");
					if (statement->_return->expr.has_value())
					debug_print_expr(statement->_return->expr.value(), depth);
				} break;
				case Ast_Statement::Tag::Continue:
				{
					printf("Continue\n");
				} break;
				case Ast_Statement::Tag::ProcedureCall:
				{
					printf("ProcedureCall\n");
				} break;
				case Ast_Statement::Tag::VariableAssignment:
				{
					printf("VariableAssignment:  ");
					error_report_token_ident(statement->_var_assignment->ident.token, true);
					debug_print_expr(statement->_var_assignment->expr, depth);
				} break;
				case Ast_Statement::Tag::VariableDeclaration:
				{
					printf("VariableDeclaration: ");
					error_report_token_ident(statement->_var_declaration->ident.token);
					printf(", ");
					error_report_token_ident(statement->_var_declaration->type.token, true);
					if (statement->_var_declaration->expr.has_value())
					debug_print_expr(statement->_var_declaration->expr.value(), depth);
				} break;
			}
		}
		printf("\n");
	}
}

void Parser::debug_print_expr(Ast_Expression* expr, u32 depth)
{
	for (u32 i = 0; i < depth; i++)
	printf("    ");
	printf("|___");
	
	switch (expr->tag)
	{
		case Ast_Expression::Tag::Term:
		{
			printf("Term_");
			switch (expr->_term->tag)
			{
				case Ast_Term::Tag::Literal:
				{
					printf("Literal: %llu\n", expr->_term->_literal.token.integer_value);
				} break;
				case Ast_Term::Tag::Identifier:
				{
					printf("Identifier: ");
					error_report_token_ident(expr->_term->_ident.token, true);
				} break;
				case Ast_Term::Tag::ProcedureCall:
				{
					printf("Procedure_Call\n");
				} break;
			}
		} break;
		case Ast_Expression::Tag::BinaryExpression:
		{
			printf("BinaryExpression\n");
			debug_print_binary_expr(expr->_bin_expr, depth + 1);
		} break;
	}
}

void Parser::debug_print_binary_expr(Ast_Binary_Expression* expr, u32 depth)
{
	for (u32 i = 0; i < depth; i++)
	printf("    ");
	printf("|___");
	printf("BinaryOp: %i\n", (int)expr->op);

	debug_print_expr(expr->left, depth);
	debug_print_expr(expr->right, depth);
}
