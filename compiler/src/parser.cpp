
struct Parser
{
	bool init(const char* file_path);
	Ast* parse();

	std::optional<Ast_Struct_Decl> parse_struct_decl(); //Maybe linked lists with pointers are better for this
	std::optional<Ast_Enum_Decl> parse_enum_decl();
	std::optional<Ast_Proc_Decl> parse_proc_decl();
	Ast_Ident_Chain* parse_ident_chain();
	Ast_Term* parse_term();
	Ast_Expr* parse_expr();
	Ast_Expr* parse_sub_expr(u32 min_prec = 0);
	Ast_Expr* parse_primary_expr();
	Ast_Block* parse_block();
	Ast_Statement* parse_statement();
	Ast_If* parse_if();
	Ast_Else* parse_else();
	Ast_For* parse_for();
	Ast_Break* parse_break();
	Ast_Return* parse_return();
	Ast_Continue* parse_continue();
	Ast_Proc_Call* parse_proc_call();
	Ast_Var_Decl* parse_var_decl();
	Ast_Var_Assign* parse_var_assign();
	Token peek(u32 offset = 0);
	std::optional<Token> try_consume(TokenType token_type);
	std::optional<Ast_Ident> try_consume_type_ident();
	Token consume_get();
	void consume();

	Arena m_arena;
	Tokenizer tokenizer;
};

bool Parser::init(const char* file_path)
{
	m_arena.init(1024 * 1024);
	return tokenizer.set_input_from_file(file_path);
}

Ast* Parser::parse()
{
	Ast* ast = m_arena.alloc<Ast>();
	tokenizer.tokenize_buffer();

	while (true)
	{
		Token token = peek();
		consume();

		switch (token.type)
		{
			case TOKEN_KEYWORD_STRUCT:
			{
				auto struct_decl = parse_struct_decl();
				if (!struct_decl) return NULL;
				ast->structs.emplace_back(struct_decl.value());
			} break;
			case TOKEN_KEYWORD_ENUM:
			{
				auto enum_decl = parse_enum_decl();
				if (!enum_decl) return NULL;
				ast->enums.emplace_back(enum_decl.value());
			} break;
			case TOKEN_KEYWORD_FN:
			{
				auto proc_decl = parse_proc_decl();
				if (!proc_decl) return NULL;
				ast->procs.emplace_back(proc_decl.value());
			} break;
			case TOKEN_EOF:
			{
				return ast;
			} break;
			default:
			{
				printf("Expected fn, enum or struct declaration. Got other token.\n");
				debug_print_token(token, true, true);
				return NULL;
			} break;
		}
	}

	return ast;
}

std::optional<Ast_Struct_Decl> Parser::parse_struct_decl()
{
	auto type = try_consume(TOKEN_IDENT); 
	if (!type) { printf("Expected an identifier.\n"); return {}; }

	Ast_Struct_Decl decl = {};
	decl.type = Ast_Ident { type.value() };
	
	if (!try_consume(TOKEN_BLOCK_START)) { printf("Expected opening '{'.\n"); return {}; }
	while (true)
	{
		auto field = try_consume(TOKEN_IDENT);
		if (!field) break;
		if (!try_consume(TOKEN_COLON)) { printf("Expected ':' with type identifier.\n"); return {}; }
		auto field_type = try_consume_type_ident();
		if (!field_type) 
		{ 
			printf("Expected type idenifier.\n"); return {};
		}

		decl.fields.emplace_back(Ast_Ident_Type_Pair { Ast_Ident { field.value() }, field_type.value() });
		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { printf("Struct Expected closing '}'.\n"); return {}; }

	return decl;
}

std::optional<Ast_Enum_Decl> Parser::parse_enum_decl()
{
	auto type = try_consume(TOKEN_IDENT); 
	if (!type) { printf("Expected an identifier.\n"); return {}; }

	Ast_Enum_Decl decl = {};
	decl.type = Ast_Ident { type.value() };

	if (!try_consume(TOKEN_BLOCK_START)) { printf("Expected opening '{'.\n"); return {}; }
	while (true)
	{
		auto variant = try_consume(TOKEN_IDENT);
		if (!variant) break;

		decl.variants.emplace_back(Ast_Ident_Type_Pair { Ast_Ident { variant.value() }, {} }); //@Notice type is empty token, might support typed enums
		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { printf("Enum Expected closing '}'.\n"); return {}; }

	return decl;
}

std::optional<Ast_Proc_Decl> Parser::parse_proc_decl()
{
	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) { printf("Expected an identifier.\n"); return {}; }

	Ast_Proc_Decl decl = {};
	decl.ident = Ast_Ident { ident.value() };
	
	if (!try_consume(TOKEN_PAREN_START)) { printf("Expected opening '('.\n"); return {}; }
	while (true)
	{
		auto param = try_consume(TOKEN_IDENT);
		if (!param) break;
		if (!try_consume(TOKEN_COLON)) { printf("Expected ':' with type identifier.\n"); return {}; }
		auto param_type = try_consume_type_ident();
		if (!param_type) 
		{
			printf("Expected type idenifier.\n"); return {}; 
		}

		decl.input_params.emplace_back(Ast_Ident_Type_Pair{ param.value(), param_type.value() });
		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_PAREN_END)) { printf("Expected closing ')'.\n"); return {}; }

	if (try_consume(TOKEN_DOUBLE_COLON))
	{
		auto return_type = try_consume_type_ident();
		if (!return_type) { printf("Expected return type identifier.\n"); return {}; }
		decl.return_type = return_type.value();
	}

	Ast_Block* block = parse_block();
	if (block == NULL) return {};
	decl.block = block;

	return decl;
}

Ast_Ident_Chain* Parser::parse_ident_chain()
{
	Ast_Ident_Chain* ident_chain = m_arena.alloc<Ast_Ident_Chain>();
	ident_chain->ident = Ast_Ident{ consume_get() };
	Ast_Ident_Chain* current = ident_chain;

	while (true)
	{
		if (!try_consume(TOKEN_DOT)) break;
		auto ident = try_consume(TOKEN_IDENT);
		if (!ident) { printf("Expected an identifier after '.' in the ident chain.\n"); return NULL; }
		
		current->next = m_arena.alloc<Ast_Ident_Chain>();
		current->next->ident = Ast_Ident { ident.value() };
		current = current->next;
	}

	return ident_chain;
}

Ast_Term* Parser::parse_term()
{
	Ast_Term* term = m_arena.alloc<Ast_Term>();
	Token token = peek();

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
			Token next = peek(1);
			if (next.type == TOKEN_PAREN_START)
			{
				Ast_Proc_Call* proc_call = parse_proc_call();
				if (!proc_call) return NULL;
				term->tag = Ast_Term::Tag::Proc_Call;
				term->as_proc_call = proc_call;
				break;
			}

			Ast_Ident_Chain* ident_chain = parse_ident_chain();
			if (!ident_chain) return NULL;
			term->tag = Ast_Term::Tag::Ident_Chain;
			term->as_ident_chain = ident_chain;
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

Ast_Expr* Parser::parse_expr()
{
	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected ';' after expression.\n"); return NULL; }
	return expr;
}

Ast_Expr* Parser::parse_sub_expr(u32 min_prec)
{
	Ast_Expr* expr_lhs = parse_primary_expr();
	if (!expr_lhs) return NULL;

	while (true)
	{
		Token token_op = peek();
		BinaryOp op = ast_get_binary_op_from_token(token_op.type);
		if (op == BINARY_OP_ERROR) break;
		u32 prec = ast_get_binary_op_precedence(op);
		if (prec < min_prec) break;
		consume();

		u32 next_min_prec = prec + 1;
		Ast_Expr* expr_rhs = parse_sub_expr(next_min_prec);
		if (expr_rhs == NULL) return NULL;

		Ast_Expr* expr_lhs_copy = m_arena.alloc<Ast_Expr>();
		expr_lhs_copy->tag = expr_lhs->tag;
		expr_lhs_copy->as_term = expr_lhs->as_term;
		expr_lhs_copy->as_binary_expr = expr_lhs->as_binary_expr;

		Ast_Binary_Expr* bin_expr = m_arena.alloc<Ast_Binary_Expr>();
		bin_expr->op = op;
		bin_expr->left = expr_lhs_copy;
		bin_expr->right = expr_rhs;

		expr_lhs->tag = Ast_Expr::Tag::Binary_Expr;
		expr_lhs->as_binary_expr = bin_expr;
	}

	return expr_lhs;
}

Ast_Expr* Parser::parse_primary_expr()
{
	if (try_consume(TOKEN_PAREN_START))
	{
		Ast_Expr* expr = parse_sub_expr();

		if (!try_consume(TOKEN_PAREN_END))
		{
			printf("Expected closing ')' after '('.\n");
			return NULL;
		}

		return expr;
	}

	Token token = peek();
	UnaryOp op = ast_get_unary_op_from_token(token.type);
	if (op != UNARY_OP_ERROR)
	{
		consume();
		Ast_Expr* right_expr = parse_primary_expr();
		if (!right_expr) return NULL;

		Ast_Unary_Expr* unary_expr = m_arena.alloc<Ast_Unary_Expr>();
		unary_expr->op = op;
		unary_expr->right = right_expr;

		Ast_Expr* expr = m_arena.alloc<Ast_Expr>();
		expr->tag = Ast_Expr::Tag::Unary_Expr;
		expr->as_unary_expr = unary_expr;
		return expr;
	}

	Ast_Term* term = parse_term();
	if (!term) return NULL;

	Ast_Expr* expr = m_arena.alloc<Ast_Expr>();
	expr->tag = Ast_Expr::Tag::Term;
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
	Ast_Statement* statement = m_arena.alloc<Ast_Statement>();
	Token token = peek();

	switch (token.type)
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
			Token next = peek(1);

			if (next.type == TOKEN_PAREN_START)
			{
				statement->tag = Ast_Statement::Tag::Proc_Call;
				statement->as_proc_call = parse_proc_call();
				if (!statement->as_proc_call) return NULL;
				if (!try_consume(TOKEN_SEMICOLON)) { printf("Expected procedure call statement to be followed by ';'.\n"); return NULL; }
			}
			else if (next.type == TOKEN_COLON)
			{
				statement->tag = Ast_Statement::Tag::Var_Decl;
				statement->as_var_decl = parse_var_decl();
				if (!statement->as_var_decl) return NULL;
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

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	_if->condition_expr = expr;

	Ast_Block* block = parse_block();
	if (!block) return NULL;
	_if->block = block;

	Token next = peek();
	if (next.type == TOKEN_KEYWORD_ELSE)
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

	Token next = peek();

	if (next.type == TOKEN_KEYWORD_IF)
	{
		Ast_If* _if = parse_if();
		if (!_if) return NULL;
		_else->tag = Ast_Else::Tag::If;
		_else->as_if = _if;
	}
	else if (next.type == TOKEN_BLOCK_START)
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
	
	Token curr = peek();
	Token next = peek(1);

	//infinite loop
	if (curr.type == TOKEN_BLOCK_START)
	{
		Ast_Block* block = parse_block();
		if (!block) return NULL;
		_for->block = block;
		
		return _for;
	}

	//optional var declaration
	if (curr.type == TOKEN_IDENT && next.type == TOKEN_COLON)
	{
		Ast_Var_Decl* var_decl = parse_var_decl();
		if (!var_decl) return NULL;
		_for->var_decl = var_decl;
	}

	//conditional expr
	Ast_Expr* condition_expr = parse_sub_expr();
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

	Ast_Expr* expr = parse_expr();
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
	proc_call->ident = Ast_Ident { consume_get() };
	consume();

	while (true)
	{
		if (try_consume(TOKEN_PAREN_END)) return proc_call;

		Ast_Expr* param_expr = parse_sub_expr();
		if (!param_expr) return NULL;
		proc_call->input_exprs.emplace_back(param_expr);

		if (!try_consume(TOKEN_COMMA)) break;
	}

	if (!try_consume(TOKEN_PAREN_END)) { printf("Expected closing ')' after procedure call statement.\n"); return NULL; }
	return proc_call;
}

Ast_Var_Decl* Parser::parse_var_decl()
{
	Ast_Var_Decl* var_decl = m_arena.alloc<Ast_Var_Decl>();
	var_decl->ident = Ast_Ident { consume_get() };
	consume();

	auto type = try_consume_type_ident();
	if (type) var_decl->type = type.value();

	bool default_init = !try_consume(TOKEN_ASSIGN);
	if (default_init)
	{
		bool has_semicolon = try_consume(TOKEN_SEMICOLON).has_value();
		if (type && has_semicolon) return var_decl;
		if (!type && has_semicolon) printf("Expected specified type for default initialized variable.\n");
		else if (type && !has_semicolon) printf("Expected ';'.\n");
		else if (!type && !has_semicolon) printf("Expected specified type and ';' for default initialized variable.\n");
		return NULL;
	}

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_decl->expr = expr;
	return var_decl;
}

Ast_Var_Assign* Parser::parse_var_assign()
{
	Ast_Var_Assign* var_assign = m_arena.alloc<Ast_Var_Assign>();
	
	Ast_Ident_Chain* ident_chain = parse_ident_chain();
	if (!ident_chain) return NULL;
	var_assign->ident_chain = ident_chain;

	Token token = peek();
	AssignOp op = ast_get_assign_op_from_token(token.type);
	if (op == ASSIGN_OP_ERROR)  { printf("Expected assigment operator.\n"); return NULL; }
	consume();
	var_assign->op = op;

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_assign->expr = expr;
	return var_assign;
}

Token Parser::peek(u32 offset)
{
	return tokenizer.tokens[tokenizer.peek_index + offset];
}

std::optional<Token> Parser::try_consume(TokenType token_type)
{
	Token token = peek();
	if (token.type == token_type)
	{
		consume();
		return token;
	}
	return {};
}

std::optional<Ast_Ident> Parser::try_consume_type_ident()
{
	Token token = peek();
	if (token.type == TOKEN_IDENT || (token.type >= TOKEN_TYPE_I8 && token.type <= TOKEN_TYPE_STRING))
	{
		consume();
		return Ast_Ident { token };
	}
	return {};
}

Token Parser::consume_get()
{
	Token token = peek();
	consume();
	return token;
}

void Parser::consume()
{
	tokenizer.peek_index += 1;
	if (tokenizer.peek_index >= (tokenizer.TOKENIZER_BUFFER_SIZE - tokenizer.TOKENIZER_LOOKAHEAD))
	{
		tokenizer.peek_index = 0; 
		tokenizer.tokenize_buffer();
	}
}
