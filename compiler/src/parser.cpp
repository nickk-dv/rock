#include "parser.h"

#include "debug_printer.h"

#define peek() peek_token(parser, 0)
#define peek_next(offset) peek_token(parser, offset)
#define consume() consume_token(parser)
#define consume_get() consume_get_token(parser)
#define try_consume(token_type) try_consume_token(parser, token_type)
#define error(message) parse_error(parser, message, 0);
#define error_next(message, offset) parse_error(parser, message, offset);

bool parser_init(Parser* parser, const char* filepath)
{
	parser->arena.init(1024 * 1024);
	return tokenizer_set_input(&parser->tokenizer, filepath);
}

Ast* parser_parse(Parser* parser)
{
	Ast* ast = parser->arena.alloc<Ast>();
	tokenizer_tokenize(&parser->tokenizer, parser->tokens);

	while (true) 
	{
		Token token = peek();
		switch (token.type)
		{
		case TOKEN_IDENT:
		{
			if (peek_next(1).type == TOKEN_DOUBLE_COLON)
			{
				switch (peek_next(2).type)
				{
				case TOKEN_KEYWORD_IMPORT:
				{
					Ast_Import_Decl* import_decl = parse_import_decl(parser);
					if (!import_decl) return NULL;
					ast->imports.emplace_back(import_decl);
				} break;
				case TOKEN_KEYWORD_USE:
				{
					Ast_Use_Decl* use_decl = parse_use_decl(parser);
					if (!use_decl) return NULL;
					ast->uses.emplace_back(use_decl);
				} break;
				case TOKEN_KEYWORD_STRUCT:
				{
					Ast_Struct_Decl* struct_decl = parse_struct_decl(parser);
					if (!struct_decl) return NULL;
					ast->structs.emplace_back(struct_decl);
				} break;
				case TOKEN_KEYWORD_ENUM:
				{
					Ast_Enum_Decl* enum_decl = parse_enum_decl(parser);
					if (!enum_decl) return NULL;
					ast->enums.emplace_back(enum_decl);
				} break;
				case TOKEN_PAREN_START:
				{
					Ast_Proc_Decl* proc_decl = parse_proc_decl(parser);
					if (!proc_decl) return NULL;
					ast->procs.emplace_back(proc_decl);
				} break;
				default:
				{
					error_next("Expected import, use, struct, enum or procedure declaration", 2);
					return NULL;
				}
				}
			}
			else
			{
				error_next("Expected '::'", 1);
				return NULL;
			}
		} break;
		case TOKEN_EOF: 
		{ 
			return ast; 
		}
		default: 
		{ 
			error("Expected global declaration identifier"); 
			return NULL; 
		}
		}
	}

	return ast;
}

Ast_Import_Decl* parse_import_decl(Parser* parser)
{
	Ast_Import_Decl* decl = parser->arena.alloc<Ast_Import_Decl>();
	decl->alias = token_to_ident(consume_get());
	consume(); consume();

	auto token = try_consume(TOKEN_STRING_LITERAL);
	if (!token) { error("Expected path string literal in 'import' declaration"); return NULL; }
	decl->file_path = Ast_Literal { token.value() };

	return decl;
}

Ast_Use_Decl* parse_use_decl(Parser* parser)
{
	Ast_Use_Decl* decl = parser->arena.alloc<Ast_Use_Decl>();
	decl->alias = token_to_ident(consume_get());
	consume(); consume();

	auto import = try_consume(TOKEN_IDENT);
	if (!import) { error("Expected imported namespace identifier"); return NULL; }
	decl->import = token_to_ident(import.value());

	if (!try_consume(TOKEN_DOT)) { error("Expected '.' followed by symbol"); return NULL; }
	
	auto symbol = try_consume(TOKEN_IDENT);
	if (!symbol) { error("Expected symbol identifier"); return NULL; }
	decl->symbol = token_to_ident(symbol.value());

	if (try_consume(TOKEN_DOT)) { error("Expected use declaration like this: 'alias :: use import.symbol'.\nDeep namespace access is not allowed, import necessary namespace instead"); return NULL; }
	return decl;
}

Ast_Struct_Decl* parse_struct_decl(Parser* parser)
{
	Ast_Struct_Decl* decl = parser->arena.alloc<Ast_Struct_Decl>();
	decl->type = token_to_ident(consume_get());
	consume(); consume();
	
	if (!try_consume(TOKEN_BLOCK_START)) { error("Expected '{'"); return NULL; }
	while (true)
	{
		auto field = try_consume(TOKEN_IDENT);
		if (!field) break;
		if (!try_consume(TOKEN_COLON)) { error("Expected ':' followed by type identifier"); return NULL; }

		Ast_Type type = parse_type(parser);
		if (type.as_custom == NULL) return NULL;
		decl->fields.emplace_back(Ast_Ident_Type_Pair { token_to_ident(field.value()), type });

		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { error("Expected '}' after struct declaration"); return NULL; }

	return decl;
}

Ast_Enum_Decl* parse_enum_decl(Parser* parser)
{
	Ast_Enum_Decl* decl = parser->arena.alloc<Ast_Enum_Decl>();
	decl->type = token_to_ident(consume_get());
	consume(); consume();

	if (try_consume(TOKEN_DOUBLE_COLON))
	{
		BasicType basic_type = token_to_basic_type(peek().type);
		if (basic_type == BASIC_TYPE_ERROR) { error("Expected basic type in enum declaration"); return NULL; }
		consume();
		decl->basic_type = basic_type;
	}

	if (!try_consume(TOKEN_BLOCK_START)) { error("Expected '{'"); return NULL; }
	while (true)
	{
		auto ident = try_consume(TOKEN_IDENT);
		if (!ident) break;

		if (!try_consume(TOKEN_ASSIGN)) { error("Expected '=' followed by value of enum variant"); return NULL; }
		bool is_negative = try_consume(TOKEN_MINUS).has_value();
		Token token = peek();
		if (!token_is_literal(token.type)) { error("Expected literal value: number, bool, string"); return NULL; }
		consume();
		decl->variants.emplace_back(Ast_Ident_Literal_Pair { token_to_ident(ident.value()), Ast_Literal { token }, is_negative });
		
		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_BLOCK_END)) { error("Expected '}' after enum declaration"); return NULL; }

	return decl;
}

Ast_Proc_Decl* parse_proc_decl(Parser* parser)
{
	Ast_Proc_Decl* decl = parser->arena.alloc<Ast_Proc_Decl>();
	decl->ident = token_to_ident(consume_get());
	consume(); consume();
	
	while (true)
	{
		auto param = try_consume(TOKEN_IDENT);
		if (!param) break;
		if (!try_consume(TOKEN_COLON)) { error("Expected ':' followed by type identifier"); return NULL; }

		Ast_Type type = parse_type(parser);
		if (type.as_custom == NULL) return NULL;
		decl->input_params.emplace_back(Ast_Ident_Type_Pair { token_to_ident(param.value()), type });

		if (!try_consume(TOKEN_COMMA)) break;
	}
	if (!try_consume(TOKEN_PAREN_END)) { error("Expected ')'"); return NULL; }

	if (try_consume(TOKEN_DOUBLE_COLON))
	{
		Ast_Type type = parse_type(parser);
		if (type.as_custom == NULL) return NULL;
		decl->return_type = type;
	}

	if (try_consume(TOKEN_AT))
	{
		decl->is_external = true;
	}
	else
	{
		Ast_Block* block = parse_block(parser);
		if (!block) return {};
		decl->block = block;
	}

	return decl;
}

Ast_Type parse_type(Parser* parser)
{
	Ast_Type type = {};
	Token token = peek();

	while (token.type == TOKEN_TIMES)
	{
		consume();
		token = peek();
		type.pointer_level += 1;
	}

	BasicType basic_type = token_to_basic_type(token.type);
	if (basic_type != BASIC_TYPE_ERROR)
	{
		consume();
		type.tag = Ast_Type::Tag::Basic;
		type.as_basic = basic_type;
		return type;
	}

	switch (token.type)
	{
	case TOKEN_IDENT:
	{
		Ast_Custom_Type* custom = parse_custom_type(parser);
		if (!custom) return {};
		type.tag = Ast_Type::Tag::Custom;
		type.as_custom = custom;
	} break;
	case TOKEN_BRACKET_START:
	{
		Ast_Array_Type* array = parse_array_type(parser);
		if (!array) return {};
		type.tag = Ast_Type::Tag::Array;
		type.as_array = array;
	} break;
	default:
	{
		error("Expected basic type, type identifier or array type");
		return {};
	}
	}

	return type;
}

Ast_Array_Type* parse_array_type(Parser* parser)
{
	Ast_Array_Type* array = parser->arena.alloc<Ast_Array_Type>();
	consume();

	Token token = peek();
	if (token.type == TOKEN_INTEGER_LITERAL)
	{
		consume();
		array->fixed_size = token.integer_value;
	}
	else if (try_consume(TOKEN_DOUBLE_DOT))
	{
		array->is_dynamic = true;
	}
	else { error("Expected '..' or integer size specifier"); return NULL; }
	if (!try_consume(TOKEN_BRACKET_END)) { error("Expected ']'"); return NULL; }

	Ast_Type type = parse_type(parser);
	if (type.as_custom == NULL) return NULL;
	array->element_type = type;

	return array;
}

Ast_Custom_Type* parse_custom_type(Parser* parser)
{
	Ast_Custom_Type* custom = parser->arena.alloc<Ast_Custom_Type>();

	Ast_Ident ident = token_to_ident(consume_get());
	if (try_consume(TOKEN_DOT))
	{
		custom->import = ident;
		auto type = try_consume(TOKEN_IDENT);
		if (!type) { error("Expected type identifier"); return NULL; }
		custom->type = token_to_ident(type.value());
	}
	else custom->type = ident;
	
	return custom;
}

Ast_Var* parse_var(Parser* parser)
{
	Ast_Var* var = parser->arena.alloc<Ast_Var>();
	var->ident = token_to_ident(consume_get());

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		var->access = access;
	}

	return var;
}

Ast_Access* parse_access(Parser* parser)
{
	Ast_Access* access = parser->arena.alloc<Ast_Access>();
	Token token = peek();
	
	if (token.type == TOKEN_DOT)
	{
		consume();
		Ast_Var_Access* var_access = parse_var_access(parser);
		if (!var_access) return NULL;
		access->tag = Ast_Access::Tag::Var;
		access->as_var = var_access;
	}
	else if (token.type == TOKEN_BRACKET_START)
	{
		consume();
		Ast_Array_Access* array_access = parse_array_access(parser);
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

Ast_Var_Access* parse_var_access(Parser* parser)
{
	Ast_Var_Access* var_access = parser->arena.alloc<Ast_Var_Access>();

	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) { error("Expected field identifier"); return NULL; }
	var_access->ident = token_to_ident(ident.value());

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		var_access->next = access;
	}

	return var_access;
}

Ast_Array_Access* parse_array_access(Parser* parser)
{
	Ast_Array_Access* array_access = parser->arena.alloc<Ast_Array_Access>();

	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	array_access->index_expr = expr;

	if (!try_consume(TOKEN_BRACKET_END)) { error("Expected ']'"); return NULL; }

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		array_access->next = access;
	}

	return array_access;
}

Ast_Enum* parse_enum(Parser* parser, bool import)
{
	Ast_Enum* _enum = parser->arena.alloc<Ast_Enum>();
	if (import) { _enum->import = token_to_ident(consume_get()); consume(); }
	
	auto ident = try_consume(TOKEN_IDENT);
	if (!ident) { error("Expected enum type identifier"); return NULL; }
	_enum->type = token_to_ident(ident.value());
	consume();
	auto variant = try_consume(TOKEN_IDENT);
	if (!variant) { error("Expected enum variant identifier"); return NULL; }
	_enum->variant = token_to_ident(variant.value());

	return _enum;
}

Ast_Term* parse_term(Parser* parser)
{
	Ast_Term* term = parser->arena.alloc<Ast_Term>();
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
		Token next = peek_next(1);
		Token next_2 = peek_next(2);
		Token next_3 = peek_next(3);
		bool import_prefix = next.type == TOKEN_DOT && next_2.type == TOKEN_IDENT;
		bool import_proc_call = import_prefix && next_3.type == TOKEN_PAREN_START;
		bool import_enum = import_prefix && next_3.type == TOKEN_DOUBLE_COLON;

		if (next.type == TOKEN_PAREN_START || import_proc_call)
		{
			Ast_Proc_Call* proc_call = parse_proc_call(parser, import_proc_call);
			if (!proc_call) return NULL;
			term->tag = Ast_Term::Tag::Proc_Call;
			term->as_proc_call = proc_call;
		}
		else if (next.type == TOKEN_DOUBLE_COLON || import_enum)
		{
			Ast_Enum* _enum = parse_enum(parser, import_enum);
			if (!_enum) return NULL;
			term->tag = Ast_Term::Tag::Enum;
			term->as_enum = _enum;
		}
		else
		{
			Ast_Var* var = parse_var(parser);
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

Ast_Expr* parse_expr(Parser* parser)
{
	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after expression"); return NULL; }
	return expr;
}

Ast_Expr* parse_sub_expr(Parser* parser, u32 min_prec)
{
	Ast_Expr* expr_lhs = parse_primary_expr(parser);
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
		Ast_Expr* expr_rhs = parse_sub_expr(parser, next_min_prec);
		if (expr_rhs == NULL) return NULL;

		Ast_Expr* expr_lhs_copy = parser->arena.alloc<Ast_Expr>();
		expr_lhs_copy->tag = expr_lhs->tag;
		expr_lhs_copy->as_term = expr_lhs->as_term;
		expr_lhs_copy->as_binary_expr = expr_lhs->as_binary_expr;

		Ast_Binary_Expr* bin_expr = parser->arena.alloc<Ast_Binary_Expr>();
		bin_expr->op = op;
		bin_expr->left = expr_lhs_copy;
		bin_expr->right = expr_rhs;

		expr_lhs->tag = Ast_Expr::Tag::Binary_Expr;
		expr_lhs->as_binary_expr = bin_expr;
	}

	return expr_lhs;
}

Ast_Expr* parse_primary_expr(Parser* parser)
{
	if (try_consume(TOKEN_PAREN_START))
	{
		Ast_Expr* expr = parse_sub_expr(parser);

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
		Ast_Expr* right_expr = parse_primary_expr(parser);
		if (!right_expr) return NULL;

		Ast_Unary_Expr* unary_expr = parser->arena.alloc<Ast_Unary_Expr>();
		unary_expr->op = op;
		unary_expr->right = right_expr;

		Ast_Expr* expr = parser->arena.alloc<Ast_Expr>();
		expr->tag = Ast_Expr::Tag::Unary_Expr;
		expr->as_unary_expr = unary_expr;
		return expr;
	}

	Ast_Term* term = parse_term(parser);
	if (!term) return NULL;

	Ast_Expr* expr = parser->arena.alloc<Ast_Expr>();
	expr->tag = Ast_Expr::Tag::Term;
	expr->as_term = term;

	return expr;
}

Ast_Block* parse_block(Parser* parser)
{
	Ast_Block* block = parser->arena.alloc<Ast_Block>();

	if (!try_consume(TOKEN_BLOCK_START)) { error("Expected '{' before code block"); return NULL; }
	while (true)
	{
		if (try_consume(TOKEN_BLOCK_END)) return block;

		Ast_Statement* statement = parse_statement(parser);
		if (!statement) return NULL;
		block->statements.emplace_back(statement);
	}
}

Ast_Statement* parse_statement(Parser* parser)
{
	Ast_Statement* statement = parser->arena.alloc<Ast_Statement>();
	Token token = peek();

	switch (token.type)
	{
	case TOKEN_KEYWORD_IF:
	{
		statement->tag = Ast_Statement::Tag::If;
		statement->as_if = parse_if(parser);
		if (!statement->as_if) return NULL;
	} break;
	case TOKEN_KEYWORD_FOR:
	{
		statement->tag = Ast_Statement::Tag::For;
		statement->as_for = parse_for(parser);
		if (!statement->as_for) return NULL;
	} break;
	case TOKEN_BLOCK_START:
	{
		statement->tag = Ast_Statement::Tag::Block;
		statement->as_block = parse_block(parser);
		if (!statement->as_block) return NULL;
	} break;
	case TOKEN_KEYWORD_DEFER:
	{
		statement->tag = Ast_Statement::Tag::Defer;
		statement->as_defer = parse_defer(parser);
		if (!statement->as_defer) return NULL;
	} break;
	case TOKEN_KEYWORD_BREAK:
	{
		statement->tag = Ast_Statement::Tag::Break;
		statement->as_break = parse_break(parser);
		if (!statement->as_break) return NULL;
	} break;
	case TOKEN_KEYWORD_RETURN:
	{
		statement->tag = Ast_Statement::Tag::Return;
		statement->as_return = parse_return(parser);
		if (!statement->as_return) return NULL;
	} break;
	case TOKEN_KEYWORD_CONTINUE:
	{
		statement->tag = Ast_Statement::Tag::Continue;
		statement->as_continue = parse_continue(parser);
		if (!statement->as_continue) return NULL;
	} break;
	case TOKEN_IDENT:
	{
		Token next = peek_next(1);
		Token next_2 = peek_next(2);
		Token next_3 = peek_next(3);
		bool import_prefix = next.type == TOKEN_DOT && next_2.type == TOKEN_IDENT;
		bool import_proc_call = import_prefix && next_3.type == TOKEN_PAREN_START;
			
		if (next.type == TOKEN_PAREN_START || import_proc_call)
		{
			statement->tag = Ast_Statement::Tag::Proc_Call;
			statement->as_proc_call = parse_proc_call(parser, import_proc_call);
			if (!statement->as_proc_call) return NULL;
			if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after procedure call"); return NULL; }
		}
		else if (next.type == TOKEN_COLON)
		{
			statement->tag = Ast_Statement::Tag::Var_Decl;
			statement->as_var_decl = parse_var_decl(parser);
			if (!statement->as_var_decl) return NULL;
		}
		else
		{
			statement->tag = Ast_Statement::Tag::Var_Assign;
			statement->as_var_assign = parse_var_assign(parser);
			if (!statement->as_var_assign) return NULL;
		}
	} break;
	default: 
	{ 
		error("Expected valid statement or '}' after code block"); 
		return NULL; 
	}
	}
	
	return statement;
}

Ast_If* parse_if(Parser* parser)
{
	Ast_If* _if = parser->arena.alloc<Ast_If>();
	_if->token = consume_get();

	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	_if->condition_expr = expr;

	Ast_Block* block = parse_block(parser);
	if (!block) return NULL;
	_if->block = block;

	Token next = peek();
	if (next.type == TOKEN_KEYWORD_ELSE)
	{
		Ast_Else* _else = parse_else(parser);
		if (!_else) return NULL;
		_if->_else = _else;
	}

	return _if;
}

Ast_Else* parse_else(Parser* parser)
{
	Ast_Else* _else = parser->arena.alloc<Ast_Else>();
	_else->token = consume_get();
	Token next = peek();

	if (next.type == TOKEN_KEYWORD_IF)
	{
		Ast_If* _if = parse_if(parser);
		if (!_if) return NULL;
		_else->tag = Ast_Else::Tag::If;
		_else->as_if = _if;
	}
	else if (next.type == TOKEN_BLOCK_START)
	{
		Ast_Block* block = parse_block(parser);
		if (!block) return NULL;
		_else->tag = Ast_Else::Tag::Block;
		_else->as_block = block;
	}
	else { error("Expected 'if' or code block '{ ... }'"); return NULL; }

	return _else;
}

Ast_For* parse_for(Parser* parser)
{
	Ast_For* _for = parser->arena.alloc<Ast_For>();
	_for->token = consume_get();
	Token curr = peek();
	Token next = peek_next(1);

	if (curr.type == TOKEN_BLOCK_START)
	{
		Ast_Block* block = parse_block(parser);
		if (!block) return NULL;
		_for->block = block;
		
		return _for;
	}

	if (curr.type == TOKEN_IDENT && next.type == TOKEN_COLON)
	{
		Ast_Var_Decl* var_decl = parse_var_decl(parser);
		if (!var_decl) return NULL;
		_for->var_decl = var_decl;
	}

	Ast_Expr* condition_expr = parse_sub_expr(parser);
	if (!condition_expr) { error("Expected conditional expression"); return NULL; }
	_for->condition_expr = condition_expr;

	if (try_consume(TOKEN_SEMICOLON))
	{
		Ast_Var_Assign* var_assignment = parse_var_assign(parser);
		if (!var_assignment) return NULL;
		_for->var_assign = var_assignment;
	}

	Ast_Block* block = parse_block(parser);
	if (!block) return NULL;
	_for->block = block;

	return _for;
}

Ast_Defer* parse_defer(Parser* parser)
{
	Ast_Defer* defer = parser->arena.alloc<Ast_Defer>();
	defer->token = consume_get();

	Ast_Block* block = parse_block(parser);
	if (!block) return NULL;
	defer->block = block;

	return defer;
}

Ast_Break* parse_break(Parser* parser)
{
	Ast_Break* _break = parser->arena.alloc<Ast_Break>();
	_break->token = consume_get();

	if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after 'break'"); return NULL; }
	return _break;
}

Ast_Return* parse_return(Parser* parser)
{
	Ast_Return* _return = parser->arena.alloc<Ast_Return>();
	_return->token = consume_get();

	if (try_consume(TOKEN_SEMICOLON)) return _return;

	Ast_Expr* expr = parse_expr(parser);
	if (!expr) return NULL;
	_return->expr = expr;
	return _return;
}

Ast_Continue* parse_continue(Parser* parser)
{
	Ast_Continue* _continue = parser->arena.alloc<Ast_Continue>();
	_continue->token = consume_get();

	if (!try_consume(TOKEN_SEMICOLON)) { error("Expected ';' after 'continue'"); return NULL; }
	return _continue;
}

Ast_Proc_Call* parse_proc_call(Parser* parser, bool import)
{
	Ast_Proc_Call* proc_call = parser->arena.alloc<Ast_Proc_Call>();

	if (import) { proc_call->import = token_to_ident(consume_get()); consume(); }
	proc_call->ident = token_to_ident(consume_get());
	consume();

	while (true)
	{
		if (try_consume(TOKEN_PAREN_END)) return proc_call;

		Ast_Expr* param_expr = parse_sub_expr(parser);
		if (!param_expr) return NULL;
		proc_call->input_exprs.emplace_back(param_expr);

		if (!try_consume(TOKEN_COMMA)) break;
	}

	if (!try_consume(TOKEN_PAREN_END)) { error("Expected ')' after procedure call"); return NULL; }

	Token token = peek();
	if (token.type == TOKEN_DOT || token.type == TOKEN_BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		proc_call->access = access;
	}

	return proc_call;
}

Ast_Var_Decl* parse_var_decl(Parser* parser)
{
	Ast_Var_Decl* var_decl = parser->arena.alloc<Ast_Var_Decl>();
	var_decl->ident = token_to_ident(consume_get());
	consume();

	bool infer_type = try_consume(TOKEN_ASSIGN).has_value();

	if (!infer_type)
	{
		Ast_Type type = parse_type(parser);
		if (type.as_custom == NULL) return NULL;
		var_decl->type = type;

		if (try_consume(TOKEN_SEMICOLON)) return var_decl;
		if (!try_consume(TOKEN_ASSIGN)) { error("Expected '=' or ';' in a variable declaration"); return NULL; }
	}

	Ast_Expr* expr = parse_expr(parser);
	if (!expr) return NULL;
	var_decl->expr = expr;
	return var_decl;
}

Ast_Var_Assign* parse_var_assign(Parser* parser)
{
	Ast_Var_Assign* var_assign = parser->arena.alloc<Ast_Var_Assign>();

	Ast_Var* var = parse_var(parser);
	if (!var) return NULL;
	var_assign->var = var;

	Token token = peek();
	AssignOp op = token_to_assign_op(token.type);
	if (op == ASSIGN_OP_ERROR) { error("Expected assignment operator"); return NULL; }
	var_assign->op = op;
	consume();

	Ast_Expr* expr = parse_expr(parser);
	if (!expr) return NULL;
	var_assign->expr = expr;
	return var_assign;
}

Token peek_token(Parser* parser, u32 offset)
{
	return parser->tokens[parser->peek_index + offset];
}

void consume_token(Parser* parser)
{
	parser->peek_index += 1;
	if (parser->peek_index >= (TOKENIZER_BUFFER_SIZE - TOKENIZER_LOOKAHEAD))
	{
		parser->peek_index = 0;
		tokenizer_tokenize(&parser->tokenizer, parser->tokens);
	}
}

Token consume_get_token(Parser* parser)
{
	Token token = peek();
	consume();
	return token;
}

std::optional<Token> try_consume_token(Parser* parser, TokenType token_type)
{
	Token token = peek();
	if (token.type == token_type)
	{
		consume();
		return token;
	}
	return {};
}

void parse_error(Parser* parser, const char* message, u32 offset)
{
	printf("%s.\n", message);
	debug_print_token(peek_next(offset), true, true);
}
