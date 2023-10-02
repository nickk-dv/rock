#include "parser.h"

#include "debug_printer.h"

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
		switch (token.type)
		{
			case TOKEN_IDENT:
			{
				if (peek(1).type == TOKEN_DOUBLE_COLON)
				{
					switch (peek(2).type)
					{
						case TOKEN_KEYWORD_STRUCT:
						{
							Ast_Struct_Decl* struct_decl = parse_struct_decl();
							if (!struct_decl) return NULL;
							ast->structs.emplace_back(struct_decl);
						} break;
						case TOKEN_KEYWORD_ENUM:
						{
							Ast_Enum_Decl* enum_decl = parse_enum_decl();
							if (!enum_decl) return NULL;
							ast->enums.emplace_back(enum_decl);
						} break;
						case TOKEN_PAREN_START:
						{
							Ast_Proc_Decl* proc_decl = parse_proc_decl();
							if (!proc_decl) return NULL;
							ast->procs.emplace_back(proc_decl);
						} break;
						default:
						{
							error("Expected struct, enum or procedure declaration", 2);
							return NULL;
						}
					}
				}
				else
				{
					error("Expected '::'", 1);
					return NULL;
				}
			} break;
			case TOKEN_EOF: { return ast; }
			default: { error("Expected global declaration identifier"); return NULL; }
		}
	}

	return ast;
}

Ast_Struct_Decl* Parser::parse_struct_decl()
{
	Ast_Struct_Decl* decl = m_arena.alloc<Ast_Struct_Decl>();
	decl->type = Ast_Ident { consume_get() };
	consume(); consume();
	
	if (!try_consume(TOKEN_BLOCK_START)) { error("Expected '{'"); return {}; }
	while (true)
	{
		auto field = try_consume(TOKEN_IDENT);
		if (!field) break;
		if (!try_consume(TOKEN_COLON)) { error("Expected ':' followed by type identifier"); return {}; }

		Ast_Type* type = parse_type();
		if (!type) return NULL;
		decl->fields.emplace_back(Ast_Ident_Type_Pair { Ast_Ident { field.value() }, type });

		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { error("Expected '}' after struct declaration"); return {}; }

	return decl;
}

Ast_Enum_Decl* Parser::parse_enum_decl()
{
	Ast_Enum_Decl* decl = m_arena.alloc<Ast_Enum_Decl>();
	decl->type = Ast_Ident { consume_get() };
	consume(); consume();
	int constant = 0;

	if (!try_consume(TOKEN_BLOCK_START)) { error("Expected '{'"); return {}; }
	while (true)
	{
		auto variant = try_consume(TOKEN_IDENT);
		if (!variant) break;

		if (try_consume(TOKEN_ASSIGN))
		{
			Token int_lit = peek();
			if (int_lit.type != TOKEN_INTEGER_LITERAL) //@Notice only i32 numbers allowed
			{
				error("Expected constant integer literal.\n"); return {};
			}
			consume();
			constant = int_lit.integer_value; //@Notice negative not supported by token integer_value
		}

		decl->variants.emplace_back(Ast_Ident { variant.value() });
		decl->constants.emplace_back(constant);
		constant += 1;
		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { error("Expected '}' after enum declaration"); return {}; }

	return decl;
}

Ast_Proc_Decl* Parser::parse_proc_decl()
{
	Ast_Proc_Decl* decl = m_arena.alloc<Ast_Proc_Decl>();
	decl->ident = Ast_Ident { consume_get() };
	consume();
	
	if (!try_consume(TOKEN_PAREN_START)) { error("Expected '('"); return {}; }
	while (true)
	{
		auto param = try_consume(TOKEN_IDENT);
		if (!param) break;
		if (!try_consume(TOKEN_COLON)) { error("Expected ':' followed by type identifier"); return {}; }

		Ast_Type* type = parse_type();
		if (!type) return NULL;
		decl->input_params.emplace_back(Ast_Ident_Type_Pair{ param.value(), type });

		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_PAREN_END)) { error("Expected ')'"); return {}; }

	if (try_consume(TOKEN_DOUBLE_COLON))
	{
		Ast_Type* type = parse_type();
		if (!type) return NULL;
		decl->return_type = type;
	}

	if (try_consume(TOKEN_AT))
	{
		decl->external = true;
	}
	else
	{
		Ast_Block* block = parse_block();
		if (!block) return {};
		decl->block = block;
	}

	return decl;
}

Ast_Type* Parser::parse_type()
{
	Ast_Type* type = m_arena.alloc<Ast_Type>();
	Token token = peek();

	BasicType basic_type = token_to_basic_type(token.type);
	if (basic_type != BASIC_TYPE_ERROR)
	{
		consume();
		type->tag = Ast_Type::Tag::Basic;
		type->as_basic = basic_type;
		return type;
	}

	switch (token.type)
	{
		case TOKEN_IDENT:
		{
			type->tag = Ast_Type::Tag::Custom;
			type->as_custom = Ast_Ident { consume_get() };
		} break;
		case TOKEN_TIMES:
		{
			consume();
			Ast_Type* pointer_type = parse_type();
			if (!pointer_type) return NULL;
			type->tag = Ast_Type::Tag::Pointer;
			type->as_pointer = pointer_type;
		} break;
		case TOKEN_BRACKET_START:
		{
			Ast_Array_Type* array = parse_array_type();
			if (!array) return NULL;
			type->tag = Ast_Type::Tag::Array;
			type->as_array = array;
		} break;
		default:
		{
			error("Expected basic type, type identifier, pointer or array");
			return NULL;
		}
	}

	return type;
}

Ast_Array_Type* Parser::parse_array_type()
{
	Ast_Array_Type* array = m_arena.alloc<Ast_Array_Type>();
	consume();

	Token token = peek();
	if (token.type == TOKEN_INTEGER_LITERAL)
	{
		consume();
		array->fixed_size = token.integer_value;
	}
	else if (try_consume(TOKEN_DOT)) //@Hack define '..' token
	{
		if (try_consume(TOKEN_DOT)) { array->is_dynamic = true; }
		else { error("Expected '..'"); return NULL; }
	}
	else { error("Expected '..' or integer size specifier"); return NULL; }
	if (!try_consume(TOKEN_BRACKET_END)) { error("Expected ']'"); return NULL; }

	Ast_Type* type = parse_type();
	if (!type) return NULL;
	array->element_type = type;

	return array;
}

Ast_Var* Parser::parse_var()
{
	Ast_Var* var = m_arena.alloc<Ast_Var>();
	var->ident = Ast_Ident { consume_get() };

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		var->access = access;
	}

	return var;
}

Ast_Access* Parser::parse_access()
{
	Ast_Access* access = m_arena.alloc<Ast_Access>();
	Token token = peek();
	
	if (token.type == TOKEN_DOT)
	{
		consume();
		Ast_Var_Access* var_access = parse_var_access();
		if (!var_access) return NULL;
		access->tag = Ast_Access::Tag::Var;
		access->as_var = var_access;
	}
	else if (token.type == TOKEN_BRACKET_START)
	{
		consume();
		Ast_Array_Access* array_access = parse_array_access();
		if (!array_access) return NULL;
		access->tag = Ast_Access::Tag::Array;
		access->as_array = array_access;
	}
	else
	{
		error("Fatal parse error in parse_access");
		return NULL;
	}

	return access;
}

Ast_Var_Access* Parser::parse_var_access()
{
	Ast_Var_Access* var_access = m_arena.alloc<Ast_Var_Access>();

	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) { error("Expected field identifier"); return NULL; }
	var_access->ident = Ast_Ident { ident.value() };

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		var_access->next = access;
	}

	return var_access;
}

Ast_Array_Access* Parser::parse_array_access()
{
	Ast_Array_Access* array_access = m_arena.alloc<Ast_Array_Access>();

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	array_access->index_expr = expr;

	if (!try_consume(TOKEN_BRACKET_END)) { error("Expected ']'"); return NULL; }

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		array_access->next = access;
	}

	return array_access;
}

Ast_Term* Parser::parse_term()
{
	Ast_Term* term = m_arena.alloc<Ast_Term>();
	Token token = peek();

	switch (token.type)
	{
		case TOKEN_BOOL_LITERAL:
		case TOKEN_FLOAT_LITERAL:
		case TOKEN_INTEGER_LITERAL:
		case TOKEN_STRING_LITERAL:
		{
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
			}
			else
			{
				Ast_Var* var = parse_var();
				if (!var) return NULL;
				term->tag = Ast_Term::Tag::Var;
				term->as_var = var;
			}
		} break;
		default:
		{
			error("Expected a valid expression term");
			return NULL;
		}
	}

	return term;
}

Ast_Expr* Parser::parse_expr()
{
	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after expression"); return NULL; }
	return expr;
}

Ast_Expr* Parser::parse_sub_expr(u32 min_prec)
{
	Ast_Expr* expr_lhs = parse_primary_expr();
	if (!expr_lhs) return NULL;

	while (true)
	{
		Token token_op = peek();
		BinaryOp op = token_to_binary_op(token_op.type);
		if (op == BINARY_OP_ERROR) break;
		u32 prec = token_binary_op_prec(op);
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
			error("Expected ')'");
			return NULL;
		}

		return expr;
	}

	Token token = peek();
	UnaryOp op = token_to_unary_op(token.type);
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

	if (!try_consume(TOKEN_BLOCK_START)) { error("Expected '{' before code block"); return NULL; }
	while (true)
	{
		if (try_consume(TOKEN_BLOCK_END)) return block;

		Ast_Statement* statement = parse_statement();
		if (!statement) return NULL;
		block->statements.emplace_back(statement);
	}
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
				if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after procedure call"); return NULL; }
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
		default: { error("Expected valid statement or '}' after code block"); return NULL; }
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
	else { error("Expected 'if' or code block '{ ... }'"); return NULL; }

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
	if (!condition_expr) { error("Expected conditional expression"); return NULL; }
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

	if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after 'break'"); return NULL; }
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

	if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after 'continue'"); return NULL; }
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

	if (!try_consume(TOKEN_PAREN_END)) { error("Expected ')' after procedure call"); return NULL; }

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		proc_call->access = access;
	}

	return proc_call;
}

Ast_Var_Decl* Parser::parse_var_decl()
{
	Ast_Var_Decl* var_decl = m_arena.alloc<Ast_Var_Decl>();
	var_decl->ident = Ast_Ident { consume_get() };
	consume();

	bool infer_type = try_consume(TOKEN_ASSIGN).has_value();

	if (!infer_type)
	{
		Ast_Type* type = parse_type();
		if (!type) return NULL;
		var_decl->type = type;

		if (try_consume(TOKEN_SEMICOLON)) return var_decl;
		if (!try_consume(TOKEN_ASSIGN)) { error("Expected '=' or ';' in a variable declaration"); return NULL; }
	}

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_decl->expr = expr;
	return var_decl;
}

Ast_Var_Assign* Parser::parse_var_assign()
{
	Ast_Var_Assign* var_assign = m_arena.alloc<Ast_Var_Assign>();

	Ast_Var* var = parse_var();
	if (!var) return NULL;
	var_assign->var = var;

	Token token = peek();
	AssignOp op = token_to_assign_op(token.type);
	if (op == ASSIGN_OP_ERROR) { error("Expected assignment operator"); return NULL; }
	var_assign->op = op;
	consume();

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

void Parser::error(const char* message, u32 peek_offset)
{
	printf("%s.\n", message);
	debug_print_token(peek(peek_offset), true, true);
}
