
struct Parser
{
	Parser(std::vector<Token> tokens);

	Ast* parse();

	std::optional<Ast_Struct_Declaration> parse_struct();
	std::optional<Ast_Enum_Declaration> parse_enum();
	std::optional<Ast_Procedure_Declaration> parse_procedure();
	Ast_Access_Chain* parse_access_chain();
	Ast_Term* parse_term();
	Ast_Expression* parse_expression();
	Ast_Expression* parse_primary_expression();
	Ast_Expression* parse_sub_expression(u32 min_precedence = 0);
	Ast_Block* parse_block();
	Ast_Statement* parse_statement();
	Ast_If* parse_if();
	Ast_Else* parse_else();
	Ast_For* parse_for();
	Ast_Break* parse_break();
	Ast_Return* parse_return();
	Ast_Continue* parse_continue();
	Ast_Proc_Call* parse_proc_call();
	Ast_Var_Assign* parse_var_assign();
	Ast_Var_Declare* parse_var_declare();
	std::optional<Token> peek(u32 offset = 0);
	std::optional<Token> try_consume(TokenType token_type);
	Token consume_get();
	void consume();

	const std::vector<Token> m_tokens;
	size_t m_index = 0;
	ArenaAllocator m_arena;
};

Parser::Parser(std::vector<Token> tokens) 
	: m_tokens(std::move(tokens)), 
	  m_arena(1024 * 1024 * 4) {}

Ast* Parser::parse()
{
	Ast* ast = m_arena.alloc<Ast>();

	while (peek().has_value())
	{
		Token token = peek().value();
		consume();

		switch (token.type)
		{
			case TOKEN_KEYWORD_STRUCT:
			{
				auto struct_decl = parse_struct();
				if (!struct_decl) return NULL;
				ast->structs.emplace_back(struct_decl.value());
			} break;
			case TOKEN_KEYWORD_ENUM:
			{
				auto enum_decl = parse_enum();
				if (!enum_decl) return NULL;
				ast->enums.emplace_back(enum_decl.value());
			} break;
			case TOKEN_KEYWORD_FN:
			{
				auto proc_decl = parse_procedure();
				if (!proc_decl) return NULL;
				ast->procedures.emplace_back(proc_decl.value());
			} break;
			default:
			{
				if (m_tokens[m_index - 1].type == TOKEN_INPUT_END) return ast;
				printf("Expected fn, enum or struct declaration.\n");
				debug_print_token(m_tokens[m_index - 1], true); //@Hack reporting on prev token, current one is consumed above
				return NULL;
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
		if (!try_consume(TOKEN_COMMA)) break;
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
		if (!try_consume(TOKEN_COMMA)) break;
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

		decl.input_params.emplace_back(IdentTypePair { param.value(), param_type.value() });
		if (!try_consume(TOKEN_COMMA)) break;
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

Ast_Access_Chain* Parser::parse_access_chain()
{
	Ast_Access_Chain* access_chain = m_arena.alloc<Ast_Access_Chain>();
	access_chain->ident = Ast_Identifier { consume_get() };
	Ast_Access_Chain* current = access_chain;

	while (true)
	{
		if (!try_consume(TOKEN_DOT)) break;
		auto ident = try_consume(TOKEN_IDENT);
		if (!ident) { printf("Expected an identifier after '.' in the access chain.\n"); return NULL; }
		
		current->next = m_arena.alloc<Ast_Access_Chain>();
		current->next->ident = Ast_Identifier { ident.value() };
		current = current->next;
	}

	return access_chain;
}

Ast_Term* Parser::parse_term()
{
	Ast_Term* term = m_arena.alloc<Ast_Term>();

	auto peek_token = peek();
	if (!peek_token) { printf("No token found at the start of a term.\n"); return NULL; }
	Token token = peek_token.value();

	switch (token.type)
	{
		case TOKEN_STRING:
		{
			term->tag = Ast_Term::Tag::Literal;
			term->as_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_NUMBER:
		{
			term->tag = Ast_Term::Tag::Literal;
			term->as_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_KEYWORD_TRUE:
		{
			token.type = TOKEN_BOOL_LITERAL;
			token.bool_value = true;
			term->tag = Ast_Term::Tag::Literal;
			term->as_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_KEYWORD_FALSE:
		{
			token.type = TOKEN_BOOL_LITERAL;
			token.bool_value = false;
			term->tag = Ast_Term::Tag::Literal;
			term->as_literal = Ast_Literal { token };
			consume();
		} break;
		case TOKEN_IDENT:
		{
			auto next = peek(1);
			if (next && next.value().type == TOKEN_PAREN_START)
			{
				Ast_Proc_Call* proc_call = parse_proc_call();
				if (!proc_call) return NULL;
				term->tag = Ast_Term::Tag::Proc_Call;
				term->as_proc_call = proc_call;
				break;
			}

			Ast_Access_Chain* access_chain = parse_access_chain();
			if (!access_chain) return NULL;
			term->tag = Ast_Term::Tag::Access_Chain;
			term->as_access_chain = access_chain;
		} break;
		default:
		{
			printf("Expected a valid expression term.\n");
			debug_print_token(token, true);
			return NULL;
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

Ast_Expression* Parser::parse_sub_expression(u32 min_precedence)
{
	Ast_Expression* expr_lhs = parse_primary_expression();
	if (!expr_lhs) return NULL;

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

		Ast_Expression* expr_lhs_copy = m_arena.alloc<Ast_Expression>();
		expr_lhs_copy->tag = expr_lhs->tag;
		expr_lhs_copy->as_term = expr_lhs->as_term;
		expr_lhs_copy->as_binary_expr = expr_lhs->as_binary_expr;

		Ast_Binary_Expression* bin_expr = m_arena.alloc<Ast_Binary_Expression>();
		bin_expr->op = op;
		bin_expr->left = expr_lhs_copy;
		bin_expr->right = expr_rhs;

		expr_lhs->tag = Ast_Expression::Tag::BinaryExpression;
		expr_lhs->as_binary_expr = bin_expr;
	}

	return expr_lhs;
}

Ast_Expression* Parser::parse_primary_expression()
{
	if (try_consume(TOKEN_PAREN_START))
	{
		Ast_Expression* expr = parse_sub_expression();

		if (!try_consume(TOKEN_PAREN_END))
		{
			printf("Expected closing ')' after '('.\n");
			return NULL;
		}

		return expr;
	}

	auto next = peek();
	if (next)
	{
		UnaryOp op = ast_unary_op_from_token(next.value().type);
		if (op != UNARY_OP_ERROR)
		{
			consume();
			Ast_Expression* right_expr = parse_primary_expression();
			if (!right_expr) return NULL;

			Ast_Unary_Expression* unary_expr = m_arena.alloc<Ast_Unary_Expression>();
			unary_expr->op = op;
			unary_expr->right = right_expr;

			Ast_Expression* expr = m_arena.alloc<Ast_Expression>();
			expr->tag = Ast_Expression::Tag::UnaryExpression;
			expr->as_unary_expr = unary_expr;
			return expr;
		}
	}

	Ast_Term* term = parse_term();
	if (!term) return NULL;

	Ast_Expression* expr = m_arena.alloc<Ast_Expression>();
	expr->tag = Ast_Expression::Tag::Term;
	expr->as_term = term;

	return expr;
}

Ast_Block* Parser::parse_block()
{
	Ast_Block* block = m_arena.alloc<Ast_Block>();

	if (!try_consume(TOKEN_BLOCK_START)) { printf("Expected code block that starts with '{'.\n"); return NULL; }
	while (true)
	{
		if (try_consume(TOKEN_BLOCK_END)) return block;

		Ast_Statement* statement = parse_statement();
		if (!statement) return NULL;
		block->statements.emplace_back(statement);
	}
	if (!try_consume(TOKEN_BLOCK_END)) { printf("Expected code block to end with '}'.\n"); return NULL; }

	return block;
}

Ast_Statement* Parser::parse_statement()
{
	auto token = peek();
	if (!token) { printf("No token found at the start of a statement.\n"); return NULL; }

	Ast_Statement* statement = m_arena.alloc<Ast_Statement>();

	switch (token.value().type)
	{
		case TOKEN_KEYWORD_IF:
		{
			statement->tag = Ast_Statement::Tag::If;
			statement->as_if = parse_if();
			if (!statement->as_if) return NULL;
		} break;
		case TOKEN_KEYWORD_FOR:
		{
			statement->tag = Ast_Statement::Tag::For;
			statement->as_for = parse_for();
			if (!statement->as_for) return NULL;
		} break;
		case TOKEN_KEYWORD_BREAK:
		{
			statement->tag = Ast_Statement::Tag::Break;
			statement->as_break = parse_break();
			if (!statement->as_break) return NULL;
		} break;
		case TOKEN_KEYWORD_RETURN:
		{
			statement->tag = Ast_Statement::Tag::Return;
			statement->as_return = parse_return();
			if (!statement->as_return) return NULL;
		} break;
		case TOKEN_KEYWORD_CONTINUE:
		{
			statement->tag = Ast_Statement::Tag::Continue;
			statement->as_continue = parse_continue();
			if (!statement->as_continue) return NULL;
		} break;
		case TOKEN_IDENT:
		{
			auto next = peek(1);
			if (!next) { printf("Expected identifier to be followed by a valid statement ':' '.' '('.\n"); return NULL; }

			if (next.value().type == TOKEN_COLON)
			{
				statement->tag = Ast_Statement::Tag::Var_Declare;
				statement->as_var_declare = parse_var_declare();
				if (!statement->as_var_declare) return NULL;
			}
			else if (next.value().type == TOKEN_PAREN_START)
			{
				statement->tag = Ast_Statement::Tag::Proc_Call;
				statement->as_proc_call = parse_proc_call();
				if (!statement->as_proc_call) return NULL;
				if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected procedure call statement to be followed by ';'.\n"); return NULL; }
			}
			else
			{
				statement->tag = Ast_Statement::Tag::Var_Assign;
				statement->as_var_assign = parse_var_assign();
				if (!statement->as_var_assign) return NULL;
			}
		} break;
		default: { printf("Invalid token at the start of a statement.\n"); return NULL; }
	}
	
	return statement;
}

Ast_If* Parser::parse_if()
{
	Ast_If* _if = m_arena.alloc<Ast_If>();
	_if->token = consume_get();

	Ast_Expression* expr = parse_sub_expression();
	if (!expr) return NULL;
	_if->condition_expr = expr;

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
	_else->token = consume_get();

	auto next = peek();
	if (!next) { printf("Expected 'else' to be followed by 'if' or a code block '{ ... }'.\n"); return NULL; }

	if (next.value().type == TOKEN_KEYWORD_IF)
	{
		Ast_If* _if = parse_if();
		if (!_if) return NULL;
		_else->tag = Ast_Else::Tag::If;
		_else->as_if = _if;
	}
	else if (next.value().type == TOKEN_BLOCK_START)
	{
		Ast_Block* block = parse_block();
		if (!block) return NULL;
		_else->tag = Ast_Else::Tag::Block;
		_else->as_block = block;
	}
	else { printf("Expected 'else' to be followed by 'if' or a code block '{ ... }'.\n"); return NULL; }

	return _else;
}

Ast_For* Parser::parse_for()
{
	Ast_For* _for = m_arena.alloc<Ast_For>();
	_for->token = consume_get();
	
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

	//optional var declaration
	if (curr && curr.value().type == TOKEN_IDENT && next && next.value().type == TOKEN_COLON)
	{
		Ast_Var_Declare* var_declaration = parse_var_declare();
		if (!var_declaration) return NULL;
		_for->var_declare = var_declaration;
	}

	//conditional expr
	Ast_Expression* condition_expr = parse_sub_expression();
	if (!condition_expr) { printf("Expected a conditional expression.\n"); return NULL; }
	_for->condition_expr = condition_expr;

	//optional post expr
	if (try_consume(TOKEN_SEMICOLON))
	{
		Ast_Var_Assign* var_assignment = parse_var_assign();
		if (!var_assignment) return NULL;
		_for->var_assign = var_assignment;
	}

	Ast_Block* block = parse_block();
	if (!block) return NULL;
	_for->block = block;

	return _for;
}

Ast_Break* Parser::parse_break()
{
	Ast_Break* _break = m_arena.alloc<Ast_Break>();
	_break->token = consume_get();

	if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected ';' after break statement.\n"); return NULL; }
	return _break;
}

Ast_Return* Parser::parse_return()
{
	Ast_Return* _return = m_arena.alloc<Ast_Return>();
	_return->token = consume_get();

	if (try_consume(TOKEN_SEMICOLON)) return _return;

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	_return->expr = expr;
	return _return;
}

Ast_Continue* Parser::parse_continue()
{
	Ast_Continue* _continue = m_arena.alloc<Ast_Continue>();
	_continue->token = consume_get();

	if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected ';' after continue statement.\n"); return NULL; }
	return _continue;
}

Ast_Proc_Call* Parser::parse_proc_call()
{
	Ast_Proc_Call* proc_call = m_arena.alloc<Ast_Proc_Call>();
	proc_call->ident = Ast_Identifier { consume_get() };
	consume();

	while (true)
	{
		if (try_consume(TOKEN_PAREN_END)) return proc_call;

		Ast_Expression* param_expr = parse_sub_expression();
		if (!param_expr) return NULL;
		proc_call->input_expressions.emplace_back(param_expr);

		if (!try_consume(TOKEN_COMMA)) break;
	}

	if (!try_consume(TOKEN_PAREN_END)) { printf("Expected closing ')' after procedure call statement.\n"); return NULL; }
	return proc_call;
}

Ast_Var_Assign* Parser::parse_var_assign()
{
	Ast_Var_Assign* var_assign = m_arena.alloc<Ast_Var_Assign>();
	
	Ast_Access_Chain* access_chain = parse_access_chain();
	if (!access_chain) return NULL;
	var_assign->access_chain = access_chain;

	auto token = peek();
	if (!token) { printf("Expected assigment operator.\n"); return NULL; }
	AssignOp op = ast_assign_op_from_token(token.value().type);
	if (op == ASSIGN_OP_ERROR) { printf("Expected assigment operator.\n"); return NULL; }
	consume();
	var_assign->op = op;

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	var_assign->expr = expr;
	return var_assign;
}

Ast_Var_Declare* Parser::parse_var_declare()
{
	Ast_Var_Declare* var_declare = m_arena.alloc<Ast_Var_Declare>();
	var_declare->ident = Ast_Identifier { consume_get() };
	consume();

	auto type = try_consume(TOKEN_IDENT);
	if (type) var_declare->type = Ast_Identifier { type.value() };

	bool default_init = !try_consume(TOKEN_ASSIGN);
	if (default_init)
	{
		bool has_semicolon = try_consume(TOKEN_SEMICOLON).has_value();
		if (type && has_semicolon) return var_declare;
		if (!type && has_semicolon) printf("Expected specified type for default initialized variable.\n");
		else if (type && !has_semicolon) printf("Expected ';'.\n");
		else if (!type && !has_semicolon) printf("Expected specified type and ';' for default initialized variable.\n");
		return NULL;
	}

	Ast_Expression* expr = parse_expression();
	if (!expr) return NULL;
	var_declare->expr = expr;
	return var_declare;
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

Token Parser::consume_get()
{
	m_index += 1;
	return m_tokens[m_index - 1];
}

void Parser::consume()
{
	m_index += 1;
}
