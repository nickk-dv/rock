#include "parser.h"

#include "debug_printer.h"
#include <filesystem>

#define peek() peek_token(parser, 0)
#define peek_next(offset) peek_token(parser, offset)
#define consume() consume_token(parser)
#define consume_get() consume_get_token(parser)
#define try_consume(token_type) try_consume_token(parser, token_type)
#define error(message) parse_error(parser, message, 0);
#define error_next(message, offset) parse_error(parser, message, offset);
#define error_token(message, token) parse_error_token(parser, message, token);

namespace fs = std::filesystem;

Ast_Program* parse_program(Parser* parser, const char* path)
{
	fs::path src = fs::path(path);
	if (!fs::exists(src)) { printf("Path doesnt exist: %s\n", path); return NULL; }
	if (!fs::is_directory(src)) { printf("Path must be a directory: %s\n", path); return NULL; }
	
	tokenizer_init();
	parser->strings.init();
	arena_init(&parser->arena, 4 * 1024 * 1024);
	Ast_Program* program = arena_alloc<Ast_Program>(&parser->arena);
	program->module_map.init(32);

	for (const fs::directory_entry& dir_entry : fs::recursive_directory_iterator(src))
	{
		fs::path entry = dir_entry.path();
		if (!fs::is_regular_file(entry)) continue;
		if (entry.extension() != ".txt") continue;
		
		FILE* file;
		fopen_s(&file, entry.u8string().c_str(), "rb");
		if (!file) { printf("File open failed\n"); return NULL; }
		fseek(file, 0, SEEK_END);
		u64 size = (u64)ftell(file);
		fseek(file, 0, SEEK_SET);
		
		u8* data = arena_alloc_buffer<u8>(&parser->arena, size);
		u64 read_size = fread(data, 1, size, file);
		fclose(file);
		if (read_size != size) { printf("File read failed\n"); return NULL; }
		
		StringView source = StringView { data, size };
		std::string filepath = entry.lexically_relative(src).replace_extension("").string();
		Ast* ast = parse_ast(parser, source, filepath);
		if (ast == NULL) return NULL;
		
		program->modules.emplace_back(ast);
		program->module_map.add(filepath, ast, hash_fnv1a_32(string_view_from_string(filepath)));
	}

	return program;
}

Ast* parse_ast(Parser* parser, StringView source, std::string& filepath)
{
	parser->peek_index = 0;
	parser->tokenizer = tokenizer_create(source, &parser->strings);
	tokenizer_tokenize(&parser->tokenizer, parser->tokens);
	
	Ast* ast = arena_alloc<Ast>(&parser->arena);
	ast->source = source;
	ast->filepath = std::string(filepath);

	while (true) 
	{
		Token token = peek();
		switch (token.type)
		{
		case TokenType::IDENT:
		{
			if (peek_next(1).type == TokenType::DOUBLE_COLON)
			{
				switch (peek_next(2).type)
				{
				case TokenType::KEYWORD_IMPORT:
				{
					Ast_Import_Decl* import_decl = parse_import_decl(parser);
					if (!import_decl) return NULL;
					ast->imports.emplace_back(import_decl);
				} break;
				case TokenType::KEYWORD_USE:
				{
					Ast_Use_Decl* use_decl = parse_use_decl(parser);
					if (!use_decl) return NULL;
					ast->uses.emplace_back(use_decl);
				} break;
				case TokenType::KEYWORD_STRUCT:
				{
					Ast_Struct_Decl* struct_decl = parse_struct_decl(parser);
					if (!struct_decl) return NULL;
					ast->structs.emplace_back(struct_decl);
				} break;
				case TokenType::KEYWORD_ENUM:
				{
					Ast_Enum_Decl* enum_decl = parse_enum_decl(parser);
					if (!enum_decl) return NULL;
					ast->enums.emplace_back(enum_decl);
				} break;
				case TokenType::PAREN_START:
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
		case TokenType::INPUT_END: 
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

option<Ast_Type> parse_type(Parser* parser)
{
	Ast_Type type = {};
	Token token = peek();

	while (token.type == TokenType::TIMES)
	{
		consume();
		token = peek();
		type.pointer_level += 1;
	}

	option<BasicType> basic_type = token_to_basic_type(token.type);
	if (basic_type)
	{
		consume();
		type.tag = Ast_Type_Tag::Basic;
		type.as_basic = basic_type.value();
		return type;
	}

	switch (token.type)
	{
	case TokenType::IDENT:
	{
		Ast_Custom_Type* custom = parse_custom_type(parser);
		if (!custom) return {};
		type.tag = Ast_Type_Tag::Custom;
		type.as_custom = custom;
	} break;
	case TokenType::BRACKET_START:
	{
		Ast_Array_Type* array = parse_array_type(parser);
		if (!array) return {};
		type.tag = Ast_Type_Tag::Array;
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
	Ast_Array_Type* array_type = arena_alloc<Ast_Array_Type>(&parser->arena);
	consume();

	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	array_type->const_expr = expr;

	if (!try_consume(TokenType::BRACKET_END)) { error("Expected ']'"); return NULL; }

	option<Ast_Type> type = parse_type(parser);
	if (!type) return NULL;
	array_type->element_type = type.value();

	return array_type;
}

Ast_Custom_Type* parse_custom_type(Parser* parser)
{
	Ast_Custom_Type* custom = arena_alloc<Ast_Custom_Type>(&parser->arena);

	Ast_Ident import = token_to_ident(consume_get());
	if (try_consume(TokenType::DOT))
	{
		custom->import = import;
		option<Token> ident = try_consume(TokenType::IDENT);
		if (!ident) { error("Expected type identifier"); return NULL; }
		custom->ident = token_to_ident(ident.value());
	}
	else custom->ident = import;

	return custom;
}

Ast_Import_Decl* parse_import_decl(Parser* parser)
{
	Ast_Import_Decl* decl = arena_alloc<Ast_Import_Decl>(&parser->arena);
	decl->alias = token_to_ident(consume_get());
	consume(); consume();

	option<Token> token = try_consume(TokenType::STRING_LITERAL);
	if (!token) { error("Expected path string literal in 'import' declaration"); return NULL; }
	decl->file_path = Ast_Literal { token.value() };

	return decl;
}

Ast_Use_Decl* parse_use_decl(Parser* parser)
{
	Ast_Use_Decl* decl = arena_alloc<Ast_Use_Decl>(&parser->arena);
	decl->alias = token_to_ident(consume_get());
	consume(); consume();

	option<Token> import = try_consume(TokenType::IDENT);
	if (!import) { error("Expected imported namespace identifier"); return NULL; }
	decl->import = token_to_ident(import.value());

	if (!try_consume(TokenType::DOT)) { error("Expected '.' followed by symbol"); return NULL; }
	
	option<Token> symbol = try_consume(TokenType::IDENT);
	if (!symbol) { error("Expected symbol identifier"); return NULL; }
	decl->symbol = token_to_ident(symbol.value());

	if (try_consume(TokenType::DOT)) { error("Expected use declaration like this: 'alias :: use import.symbol'.\nDeep namespace access is not allowed, import necessary namespace instead"); return NULL; }
	return decl;
}

Ast_Struct_Decl* parse_struct_decl(Parser* parser)
{
	Ast_Struct_Decl* decl = arena_alloc<Ast_Struct_Decl>(&parser->arena);
	decl->ident = token_to_ident(consume_get());
	consume(); consume();
	
	if (!try_consume(TokenType::BLOCK_START)) { error("Expected '{' in struct declaration"); return NULL; }
	while (true)
	{
		option<Token> field = try_consume(TokenType::IDENT);
		if (!field) break;
		if (!try_consume(TokenType::COLON)) { error("Expected ':' followed by type signature"); return NULL; }

		option<Ast_Type> type = parse_type(parser);
		if (!type) return NULL;
		decl->fields.emplace_back(Ast_Ident_Type_Pair { token_to_ident(field.value()), type.value() });

		//@Todo optional default value expr indicated by '=', else ';'
		
		if (!try_consume(TokenType::SEMICOLON)) { error("Expected ';' after struct field declaration"); return NULL; }
	}
	if (!try_consume(TokenType::BLOCK_END)) { error("Expected '}' in struct declaration"); return NULL; }

	return decl;
}

Ast_Enum_Decl* parse_enum_decl(Parser* parser)
{
	Ast_Enum_Decl* decl = arena_alloc<Ast_Enum_Decl>(&parser->arena);
	decl->ident = token_to_ident(consume_get());
	consume(); consume();

	if (try_consume(TokenType::DOUBLE_COLON))
	{
		option<BasicType> basic_type = token_to_basic_type(peek().type);
		if (!basic_type) { error("Expected basic type in enum declaration"); return NULL; }
		consume();
		decl->basic_type = basic_type.value();
	}
	else decl->basic_type = BasicType::I32;

	if (!try_consume(TokenType::BLOCK_START)) { error("Expected '{' in enum declaration"); return NULL; }
	while (true)
	{
		option<Token> ident = try_consume(TokenType::IDENT);
		if (!ident) break;

		//@Todo optional default assignment with just ';'

		if (!try_consume(TokenType::ASSIGN)) { error("Expected '=' followed by constant expression"); return NULL; }
		
		Ast_Expr* expr = parse_expr(parser);
		if (!expr) return NULL;
		decl->variants.emplace_back(Ast_Enum_Variant { token_to_ident(ident.value()), expr });
	}
	if (!try_consume(TokenType::BLOCK_END)) { error("Expected '}' in enum declaration"); return NULL; }

	return decl;
}

Ast_Proc_Decl* parse_proc_decl(Parser* parser)
{
	Ast_Proc_Decl* decl = arena_alloc<Ast_Proc_Decl>(&parser->arena);
	decl->ident = token_to_ident(consume_get());
	consume(); consume();
	
	while (true)
	{
		if (try_consume(TokenType::DOUBLE_DOT)) { decl->is_variadic = true; break; }

		option<Token> ident = try_consume(TokenType::IDENT);
		if (!ident) break;
		if (!try_consume(TokenType::COLON)) { error("Expected ':' followed by type signature"); return NULL; }

		option<Ast_Type> type = parse_type(parser);
		if (!type) return NULL;
		decl->input_params.emplace_back(Ast_Ident_Type_Pair { token_to_ident(ident.value()), type.value() });

		if (!try_consume(TokenType::COMMA)) break;
	}
	if (!try_consume(TokenType::PAREN_END)) { error("Expected ')' in procedure declaration"); return NULL; }

	if (try_consume(TokenType::DOUBLE_COLON))
	{
		option<Ast_Type> type = parse_type(parser);
		if (!type) return NULL;
		decl->return_type = type.value();
	}

	if (try_consume(TokenType::AT))
	{
		decl->is_external = true;
	}
	else
	{
		Ast_Block* block = parse_block(parser);
		if (!block) return NULL;
		decl->block = block;
	}

	return decl;
}

Ast_Block* parse_block(Parser* parser)
{
	Ast_Block* block = arena_alloc<Ast_Block>(&parser->arena);

	if (!try_consume(TokenType::BLOCK_START)) { error("Expected '{' before code block"); return NULL; }
	while (true)
	{
		if (try_consume(TokenType::BLOCK_END)) return block;

		Ast_Statement* statement = parse_statement(parser);
		if (!statement) return NULL;
		block->statements.emplace_back(statement);
	}
}

Ast_Block* parse_small_block(Parser* parser)
{
	if (peek().type == TokenType::BLOCK_START) return parse_block(parser);

	Ast_Block* block = arena_alloc<Ast_Block>(&parser->arena);

	Ast_Statement* statement = parse_statement(parser);
	if (!statement) return NULL;
	block->statements.emplace_back(statement);

	return block;
}

Ast_Statement* parse_statement(Parser* parser)
{
	u32 start = get_span_start(parser);
	Ast_Statement* statement = arena_alloc<Ast_Statement>(&parser->arena);
	Token token = peek();

	switch (token.type)
	{
	case TokenType::KEYWORD_IF:
	{
		statement->tag = Ast_Statement_Tag::If;
		statement->as_if = parse_if(parser);
		if (!statement->as_if) return NULL;
	} break;
	case TokenType::KEYWORD_FOR:
	{
		statement->tag = Ast_Statement_Tag::For;
		statement->as_for = parse_for(parser);
		if (!statement->as_for) return NULL;
	} break;
	case TokenType::BLOCK_START:
	{
		statement->tag = Ast_Statement_Tag::Block;
		statement->as_block = parse_block(parser);
		if (!statement->as_block) return NULL;
	} break;
	case TokenType::KEYWORD_DEFER:
	{
		statement->tag = Ast_Statement_Tag::Defer;
		statement->as_defer = parse_defer(parser);
		if (!statement->as_defer) return NULL;
	} break;
	case TokenType::KEYWORD_BREAK:
	{
		statement->tag = Ast_Statement_Tag::Break;
		statement->as_break = parse_break(parser);
		if (!statement->as_break) return NULL;
	} break;
	case TokenType::KEYWORD_RETURN:
	{
		statement->tag = Ast_Statement_Tag::Return;
		statement->as_return = parse_return(parser);
		if (!statement->as_return) return NULL;
	} break;
	case TokenType::KEYWORD_SWITCH:
	{
		statement->tag = Ast_Statement_Tag::Switch;
		statement->as_switch = parse_switch(parser);
		if (!statement->as_switch) return NULL;
	} break;
	case TokenType::KEYWORD_CONTINUE:
	{
		statement->tag = Ast_Statement_Tag::Continue;
		statement->as_continue = parse_continue(parser);
		if (!statement->as_continue) return NULL;
	} break;
	case TokenType::IDENT:
	{
		Token next = peek_next(1);
		Token next_2 = peek_next(2);
		Token next_3 = peek_next(3);
		bool import_prefix = next.type == TokenType::DOT && next_2.type == TokenType::IDENT;
		bool import_proc_call = import_prefix && next_3.type == TokenType::PAREN_START;

		if (next.type == TokenType::PAREN_START || import_proc_call)
		{
			statement->tag = Ast_Statement_Tag::Proc_Call;
			statement->as_proc_call = parse_proc_call(parser, import_proc_call);
			if (!statement->as_proc_call) return NULL;
			if (!try_consume(TokenType::SEMICOLON)) { error("Expected ';' after procedure call"); return NULL; }
		}
		else if (next.type == TokenType::COLON)
		{
			statement->tag = Ast_Statement_Tag::Var_Decl;
			statement->as_var_decl = parse_var_decl(parser);
			if (!statement->as_var_decl) return NULL;
		}
		else
		{
			statement->tag = Ast_Statement_Tag::Var_Assign;
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
	
	statement->span.start = start;
	statement->span.end = get_span_end(parser);
	return statement;
}

Ast_If* parse_if(Parser* parser)
{
	Ast_If* _if = arena_alloc<Ast_If>(&parser->arena);
	_if->token = consume_get();

	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	_if->condition_expr = expr;

	Ast_Block* block = parse_block(parser);
	if (!block) return NULL;
	_if->block = block;

	Token next = peek();
	if (next.type == TokenType::KEYWORD_ELSE)
	{
		Ast_Else* _else = parse_else(parser);
		if (!_else) return NULL;
		_if->_else = _else;
	}

	return _if;
}

Ast_Else* parse_else(Parser* parser)
{
	Ast_Else* _else = arena_alloc<Ast_Else>(&parser->arena);
	_else->token = consume_get();
	Token next = peek();

	if (next.type == TokenType::KEYWORD_IF)
	{
		Ast_If* _if = parse_if(parser);
		if (!_if) return NULL;
		_else->tag = Ast_Else_Tag::If;
		_else->as_if = _if;
	}
	else if (next.type == TokenType::BLOCK_START)
	{
		Ast_Block* block = parse_block(parser);
		if (!block) return NULL;
		_else->tag = Ast_Else_Tag::Block;
		_else->as_block = block;
	}
	else { error("Expected 'if' or code block '{ ... }'"); return NULL; }

	return _else;
}

Ast_For* parse_for(Parser* parser)
{
	Ast_For* _for = arena_alloc<Ast_For>(&parser->arena);
	_for->token = consume_get();
	Token curr = peek();
	Token next = peek_next(1);

	if (curr.type == TokenType::BLOCK_START)
	{
		Ast_Block* block = parse_block(parser);
		if (!block) return NULL;
		_for->block = block;
		
		return _for;
	}

	if (curr.type == TokenType::IDENT && next.type == TokenType::COLON)
	{
		Ast_Var_Decl* var_decl = parse_var_decl(parser);
		if (!var_decl) return NULL;
		_for->var_decl = var_decl;
	}

	Ast_Expr* condition_expr = parse_sub_expr(parser);
	if (!condition_expr) { error("Expected conditional expression"); return NULL; }
	_for->condition_expr = condition_expr;

	if (try_consume(TokenType::SEMICOLON))
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
	Ast_Defer* defer = arena_alloc<Ast_Defer>(&parser->arena);
	defer->token = consume_get();

	Ast_Block* block = parse_small_block(parser);
	if (!block) return NULL;
	defer->block = block;

	return defer;
}

Ast_Break* parse_break(Parser* parser)
{
	Ast_Break* _break = arena_alloc<Ast_Break>(&parser->arena);
	_break->token = consume_get();

	if (!try_consume(TokenType::SEMICOLON)) { error("Expected ';' after 'break'"); return NULL; }
	return _break;
}

Ast_Return* parse_return(Parser* parser)
{
	Ast_Return* _return = arena_alloc<Ast_Return>(&parser->arena);
	_return->token = consume_get();

	if (try_consume(TokenType::SEMICOLON)) return _return;

	Ast_Expr* expr = parse_expr(parser);
	if (!expr) return NULL;
	_return->expr = expr;
	return _return;
}

Ast_Switch* parse_switch(Parser* parser)
{
	Ast_Switch* _switch = arena_alloc<Ast_Switch>(&parser->arena);
	_switch->token = consume_get();

	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	_switch->expr = expr;

	if (!try_consume(TokenType::BLOCK_START)) { error("Expected '{' in switch statement"); return NULL; }
	
	while (true)
	{
		if (try_consume(TokenType::BLOCK_END)) return _switch;

		Ast_Switch_Case switch_case = {};
		
		Ast_Expr* expr = parse_sub_expr(parser);
		if (!expr) return NULL;
		switch_case.const_expr = expr;

		if (!try_consume(TokenType::COLON))
		{
			Ast_Block* block = parse_small_block(parser);
			if (!block) return NULL;
			switch_case.block = block;
		}

		_switch->cases.emplace_back(switch_case);
	}
}

Ast_Continue* parse_continue(Parser* parser)
{
	Ast_Continue* _continue = arena_alloc<Ast_Continue>(&parser->arena);
	_continue->token = consume_get();

	if (!try_consume(TokenType::SEMICOLON)) { error("Expected ';' after 'continue'"); return NULL; }
	return _continue;
}

Ast_Var_Decl* parse_var_decl(Parser* parser)
{
	Ast_Var_Decl* var_decl = arena_alloc<Ast_Var_Decl>(&parser->arena);
	var_decl->ident = token_to_ident(consume_get());
	consume();

	bool infer_type = try_consume(TokenType::ASSIGN).has_value();

	if (!infer_type)
	{
		option<Ast_Type> type = parse_type(parser);
		if (!type) return NULL;
		var_decl->type = type.value();

		if (try_consume(TokenType::SEMICOLON)) return var_decl;
		if (!try_consume(TokenType::ASSIGN)) { error("Expected '=' or ';' in a variable declaration"); return NULL; }
	}

	Ast_Expr* expr = parse_expr(parser);
	if (!expr) return NULL;
	var_decl->expr = expr;
	return var_decl;
}

Ast_Var_Assign* parse_var_assign(Parser* parser)
{
	Ast_Var_Assign* var_assign = arena_alloc<Ast_Var_Assign>(&parser->arena);

	Ast_Var* var = parse_var(parser);
	if (!var) return NULL;
	var_assign->var = var;

	Token token = peek();
	option<AssignOp> op = token_to_assign_op(token.type);
	if (!op) { error("Expected assignment operator"); return NULL; }
	var_assign->op = op.value();
	consume();

	Ast_Expr* expr = parse_expr(parser);
	if (!expr) return NULL;
	var_assign->expr = expr;
	return var_assign;
}

Ast_Proc_Call* parse_proc_call(Parser* parser, bool import)
{
	Ast_Proc_Call* proc_call = arena_alloc<Ast_Proc_Call>(&parser->arena);
	if (import) { proc_call->import = token_to_ident(consume_get()); consume(); }
	proc_call->ident = token_to_ident(consume_get());

	if (!try_consume(TokenType::PAREN_START)) { error("Expected '(' in procedure call"); return NULL; }
	if (!try_consume(TokenType::PAREN_END))
	{
		while (true)
		{
			Ast_Expr* expr = parse_sub_expr(parser);
			if (!expr) return NULL;
			proc_call->input_exprs.emplace_back(expr);
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::PAREN_END)) { error("Expected ')' in procedure call"); return NULL; }
	}

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		proc_call->access = access;
	}

	return proc_call;
}

Ast_Expr* parse_expr(Parser* parser)
{
	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	if (!try_consume(TokenType::SEMICOLON)) { error("Expected ';' after expression"); return NULL; }
	return expr;
}

Ast_Expr* parse_sub_expr(Parser* parser, u32 min_prec)
{
	u32 start = get_span_start(parser);
	Ast_Expr* expr_lhs = parse_primary_expr(parser);
	if (!expr_lhs) return NULL;

	while (true)
	{
		Token token_op = peek();
		option<BinaryOp> op = token_to_binary_op(token_op.type);
		if (!op) break;
		u32 prec = token_binary_op_prec(op.value());
		if (prec < min_prec) break;
		consume();

		u32 next_min_prec = prec + 1;
		Ast_Expr* expr_rhs = parse_sub_expr(parser, next_min_prec);
		if (expr_rhs == NULL) return NULL;

		Ast_Expr* expr_lhs_copy = arena_alloc<Ast_Expr>(&parser->arena);
		expr_lhs_copy->tag = expr_lhs->tag;
		expr_lhs_copy->as_term = expr_lhs->as_term;
		expr_lhs_copy->as_binary_expr = expr_lhs->as_binary_expr;

		Ast_Binary_Expr* bin_expr = arena_alloc<Ast_Binary_Expr>(&parser->arena);
		bin_expr->op = op.value();
		bin_expr->left = expr_lhs_copy;
		bin_expr->right = expr_rhs;

		expr_lhs->tag = Ast_Expr_Tag::Binary_Expr;
		expr_lhs->as_binary_expr = bin_expr;
	}

	expr_lhs->span.start = start;
	expr_lhs->span.end = get_span_end(parser);
	return expr_lhs;
}

Ast_Expr* parse_primary_expr(Parser* parser)
{
	if (try_consume(TokenType::PAREN_START))
	{
		Ast_Expr* expr = parse_sub_expr(parser);
		if (!try_consume(TokenType::PAREN_END))
		{
			error("Expected ')'");
			return NULL;
		}
		return expr;
	}

	Token token = peek();
	option<UnaryOp> op = token_to_unary_op(token.type);
	if (op)
	{
		consume();
		Ast_Expr* right_expr = parse_primary_expr(parser);
		if (!right_expr) return NULL;

		Ast_Unary_Expr* unary_expr = arena_alloc<Ast_Unary_Expr>(&parser->arena);
		unary_expr->op = op.value();
		unary_expr->right = right_expr;

		Ast_Expr* expr = arena_alloc<Ast_Expr>(&parser->arena);
		expr->tag = Ast_Expr_Tag::Unary_Expr;
		expr->as_unary_expr = unary_expr;
		return expr;
	}

	Ast_Term* term = parse_term(parser);
	if (!term) return NULL;

	Ast_Expr* expr = arena_alloc<Ast_Expr>(&parser->arena);
	expr->tag = Ast_Expr_Tag::Term;
	expr->as_term = term;

	return expr;
}

Ast_Term* parse_term(Parser* parser)
{
	Ast_Term* term = arena_alloc<Ast_Term>(&parser->arena);
	Token token = peek();

	switch (token.type)
	{
	case TokenType::BOOL_LITERAL:
	case TokenType::FLOAT_LITERAL:
	case TokenType::INTEGER_LITERAL:
	case TokenType::STRING_LITERAL:
	{
		term->tag = Ast_Term_Tag::Literal;
		term->as_literal = Ast_Literal{ token };
		consume();
	} break;
	case TokenType::DOT:
	{
		Ast_Struct_Init* struct_init = parse_struct_init(parser, false, false);
		if (!struct_init) return NULL;
		term->tag = Ast_Term_Tag::Struct_Init;
		term->as_struct_init = struct_init;
	} break;
	case TokenType::BLOCK_START:
	case TokenType::BRACKET_START:
	{
		Ast_Array_Init* array_init = parse_array_init(parser);
		if (!array_init) return NULL;
		term->tag = Ast_Term_Tag::Array_Init;
		term->as_array_init = array_init;
	} break;
	case TokenType::KEYWORD_SIZEOF:
	{
		Ast_Sizeof* _sizeof = parse_sizeof(parser);
		if (!_sizeof) return NULL;
		term->tag = Ast_Term_Tag::Sizeof;
		term->as_sizeof = _sizeof;
	} break;
	case TokenType::IDENT:
	{
		Token next = peek_next(1);
		Token next_2 = peek_next(2);
		Token next_3 = peek_next(3);
		bool import_prefix = next.type == TokenType::DOT && next_2.type == TokenType::IDENT;
		bool import_enum = import_prefix && next_3.type == TokenType::DOUBLE_COLON;
		bool import_proc_call = import_prefix && next_3.type == TokenType::PAREN_START;

		if (next.type == TokenType::DOUBLE_COLON || import_enum)
		{
			Ast_Enum* _enum = parse_enum(parser, import_enum);
			if (!_enum) return NULL;
			term->tag = Ast_Term_Tag::Enum;
			term->as_enum = _enum;
		}
		else if (next.type == TokenType::PAREN_START || import_proc_call)
		{
			Ast_Proc_Call* proc_call = parse_proc_call(parser, import_proc_call);
			if (!proc_call) return NULL;
			term->tag = Ast_Term_Tag::Proc_Call;
			term->as_proc_call = proc_call;
		}
		else
		{
			Token next_4 = peek_next(4);
			bool si_just_type = next.type == TokenType::DOT && next_2.type == TokenType::BLOCK_START;
			bool si_with_import = import_prefix && next_3.type == TokenType::DOT && next_4.type == TokenType::BLOCK_START;

			if (si_just_type || si_with_import)
			{
				Ast_Struct_Init* struct_init = parse_struct_init(parser, si_with_import, true);
				if (!struct_init) return NULL;
				term->tag = Ast_Term_Tag::Struct_Init;
				term->as_struct_init = struct_init;
			}
			else
			{
				Ast_Var* var = parse_var(parser);
				if (!var) return NULL;
				term->tag = Ast_Term_Tag::Var;
				term->as_var = var;
			}
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

Ast_Var* parse_var(Parser* parser)
{
	Ast_Var* var = arena_alloc<Ast_Var>(&parser->arena);
	var->ident = token_to_ident(consume_get());

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		var->access = access;
	}

	return var;
}

Ast_Access* parse_access(Parser* parser)
{
	Ast_Access* access = arena_alloc<Ast_Access>(&parser->arena);
	Token token = peek();

	if (token.type == TokenType::DOT)
	{
		consume();
		Ast_Var_Access* var_access = parse_var_access(parser);
		if (!var_access) return NULL;
		access->tag = Ast_Access_Tag::Var;
		access->as_var = var_access;
	}
	else if (token.type == TokenType::BRACKET_START)
	{
		consume();
		Ast_Array_Access* array_access = parse_array_access(parser);
		if (!array_access) return NULL;
		access->tag = Ast_Access_Tag::Array;
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
	Ast_Var_Access* var_access = arena_alloc<Ast_Var_Access>(&parser->arena);

	option<Token> ident = try_consume(TokenType::IDENT);
	if (!ident) { error("Expected field identifier"); return NULL; }
	var_access->ident = token_to_ident(ident.value());

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		var_access->next = access;
	}

	return var_access;
}

Ast_Array_Access* parse_array_access(Parser* parser)
{
	Ast_Array_Access* array_access = arena_alloc<Ast_Array_Access>(&parser->arena);

	Ast_Expr* expr = parse_sub_expr(parser);
	if (!expr) return NULL;
	array_access->index_expr = expr;

	if (!try_consume(TokenType::BRACKET_END)) { error("Expected ']'"); return NULL; }

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access(parser);
		if (!access) return NULL;
		array_access->next = access;
	}

	return array_access;
}

Ast_Enum* parse_enum(Parser* parser, bool import)
{
	Ast_Enum* _enum = arena_alloc<Ast_Enum>(&parser->arena);
	if (import) { _enum->import = token_to_ident(consume_get()); consume(); }

	option<Token> ident = try_consume(TokenType::IDENT);
	if (!ident) { error("Expected enum type identifier"); return NULL; }
	_enum->ident = token_to_ident(ident.value());
	consume();
	option<Token> variant = try_consume(TokenType::IDENT);
	if (!variant) { error("Expected enum variant identifier"); return NULL; }
	_enum->variant = token_to_ident(variant.value());

	return _enum;
}

Ast_Sizeof* parse_sizeof(Parser* parser)
{
	Ast_Sizeof* _sizeof = arena_alloc<Ast_Sizeof>(&parser->arena);
	consume();

	if (!try_consume(TokenType::PAREN_START)) { error("Expected '(' in sizeof"); return NULL; }
	
	option<Ast_Type> type = parse_type(parser);
	if (!type) return NULL;
	_sizeof->type = type.value();

	if (!try_consume(TokenType::PAREN_END)) { error("Expected ')' in sizeof"); return NULL; }

	return _sizeof;
}

Ast_Struct_Init* parse_struct_init(Parser* parser, bool import, bool type)
{
	Ast_Struct_Init* struct_init = arena_alloc<Ast_Struct_Init>(&parser->arena);
	if (import) { struct_init->import = token_to_ident(consume_get()); consume(); }
	if (type) { struct_init->ident = token_to_ident(consume_get()); }
	consume();

	if (!try_consume(TokenType::BLOCK_START)) { error("Expected '{' in struct initializer"); return NULL; }
	if (!try_consume(TokenType::BLOCK_END))
	{
		while (true)
		{
			Ast_Expr* expr = parse_sub_expr(parser);
			if (!expr) return NULL;
			struct_init->input_exprs.emplace_back(expr);
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::BLOCK_END)) { error("Expected '}' in struct initializer"); return NULL; }
	}

	return struct_init;
}

Ast_Array_Init* parse_array_init(Parser* parser)
{
	Ast_Array_Init* array_init = arena_alloc<Ast_Array_Init>(&parser->arena);

	if (peek().type == TokenType::BRACKET_START)
	{
		array_init->type = parse_type(parser);
		if (!array_init->type) return NULL;
	}

	if (!try_consume(TokenType::BLOCK_START)) { error("Expected '{' in array initializer"); return NULL; }
	if (!try_consume(TokenType::BLOCK_END))
	{
		while (true)
		{
			Ast_Expr* expr = parse_sub_expr(parser);
			if (!expr) return NULL;
			array_init->input_exprs.emplace_back(expr);
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::BLOCK_END)) { error("Expected '}' in array initializer"); return NULL; }
	}

	return array_init;
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
		parser->prev_last = parser->tokens[TOKENIZER_BUFFER_SIZE - TOKENIZER_LOOKAHEAD - 1]; //@Hack
		tokenizer_tokenize(&parser->tokenizer, parser->tokens);
	}
}

Token consume_get_token(Parser* parser)
{
	Token token = peek();
	consume();
	return token;
}

option<Token> try_consume_token(Parser* parser, TokenType token_type)
{
	Token token = peek();
	if (token.type == token_type)
	{
		consume();
		return token;
	}
	return {};
}

u32 get_span_start(Parser* parser)
{
	return parser->tokens[parser->peek_index].span.start;
}

u32 get_span_end(Parser* parser)
{
	if (parser->peek_index == 0) return parser->prev_last.span.end; //@Hack saving last on tokenization
	return parser->tokens[parser->peek_index - 1].span.end;
}

void parse_error(Parser* parser, const char* message, u32 offset)
{
	printf("%s.\n", message);
	debug_print_token(peek_next(offset), true, true);
}

void parse_error_token(Parser* parser, const char* message, Token token)
{
	printf("%s.\n", message);
	debug_print_token(token, true, true);
}
