#include "parser.h"

#include <filesystem>

#include <chrono>

class ScopedTimer {
public:
	ScopedTimer(const char* scopeName) : m_ScopeName(scopeName) {
		m_StartTimepoint = std::chrono::high_resolution_clock::now();
	}

	~ScopedTimer() {
		auto endTimepoint = std::chrono::high_resolution_clock::now();
		auto start = std::chrono::time_point_cast<std::chrono::microseconds>(m_StartTimepoint).time_since_epoch().count();
		auto end = std::chrono::time_point_cast<std::chrono::microseconds>(endTimepoint).time_since_epoch().count();

		auto duration = end - start;
		double ms = duration * 0.001;
		printf("%s: %f ms\n", m_ScopeName, ms);
	}

private:
	const char* m_ScopeName;
	std::chrono::time_point<std::chrono::high_resolution_clock> m_StartTimepoint;
};

#define span_start() u32 start = get_span_start()
#define span_end(node) node->span.start = start; node->span.end = get_span_end()
#define span_end_dot(node) node.span.start = start; node.span.end = get_span_end()

namespace fs = std::filesystem;

Ast_Program* Parser::parse_program()
{
	ScopedTimer timer = ScopedTimer("parse files");
	
	fs::path src = fs::path("src");
	if (!fs::exists(src)) { err_report(Error::PARSE_SRC_DIR_NOT_FOUND); return NULL; }
	
	this->strings.init();
	arena_init(&this->arena, 4 * 1024 * 1024);
	Ast_Program* program = arena_alloc<Ast_Program>(&this->arena);
	program->module_map.init(64);

	for (const fs::directory_entry& dir_entry : fs::recursive_directory_iterator(src))
	{
		fs::path entry = dir_entry.path();
		if (!fs::is_regular_file(entry)) continue;
		//if (entry.extension() != ".txt") continue; //@Branding check extension
		
		FILE* file;
		fopen_s(&file, (const char*)entry.u8string().c_str(), "rb");
		if (!file) { err_report(Error::OS_FILE_OPEN_FAILED); return NULL; } //@add context
		fseek(file, 0, SEEK_END);
		u64 size = (u64)ftell(file);
		fseek(file, 0, SEEK_SET);
		
		u8* data = arena_alloc_buffer<u8>(&this->arena, size);
		u64 read_size = fread(data, 1, size, file);
		fclose(file);
		if (read_size != size) { err_report(Error::OS_FILE_READ_FAILED); return NULL; } //@add context
		
		StringView source = StringView { data, size };
		std::string filepath = entry.lexically_relative(src).replace_extension("").string();
		Ast* ast = parse_ast(source, filepath);
		if (ast == NULL) return NULL;
		
		program->modules.emplace_back(ast);
		program->module_map.add(filepath, ast, hash_fnv1a_32(string_view_from_string(filepath)));
	}

	if (!fs::exists("build") && !fs::create_directory("build")) 
	{ err_report(Error::OS_DIR_CREATE_FAILED); return NULL; } //@add context
	fs::current_path("build");

	return program;
}

Ast* Parser::parse_ast(StringView source, std::string& filepath)
{
	Ast* ast = arena_alloc<Ast>(&this->arena);
	ast->source = source;
	ast->filepath = std::string(filepath);
	
	this->ast = ast;
	this->peek_index = 0;
	this->lexer.init(source, &this->strings, &ast->line_spans);
	this->lexer.lex_token_buffer(this->tokens);

	while (true) 
	{
		Token token = peek();
		switch (token.type)
		{
		case TokenType::IDENT:
		{
			if (peek(1).type == TokenType::DOUBLE_COLON)
			{
				switch (peek(2).type)
				{
				case TokenType::KEYWORD_IMPL:
				{
					Ast_Decl_Impl* impl_decl = parse_decl_impl();
					if (!impl_decl) return NULL;
					ast->impls.emplace_back(impl_decl);
				} break;
				case TokenType::KEYWORD_IMPORT:
				{
					Ast_Decl_Import* import_decl = parse_decl_import();
					if (!import_decl) return NULL;
					ast->imports.emplace_back(import_decl);
				} break;
				case TokenType::KEYWORD_USE:
				{
					Ast_Decl_Use* use_decl = parse_decl_use();
					if (!use_decl) return NULL;
					ast->uses.emplace_back(use_decl);
				} break;
				case TokenType::KEYWORD_STRUCT:
				{
					Ast_Decl_Struct* struct_decl = parse_decl_struct();
					if (!struct_decl) return NULL;
					ast->structs.emplace_back(struct_decl);
				} break;
				case TokenType::KEYWORD_ENUM:
				{
					Ast_Decl_Enum* enum_decl = parse_decl_enum();
					if (!enum_decl) return NULL;
					ast->enums.emplace_back(enum_decl);
				} break;
				case TokenType::PAREN_START:
				{
					Ast_Decl_Proc* proc_decl = parse_decl_proc(false);
					if (!proc_decl) return NULL;
					ast->procs.emplace_back(proc_decl);
				} break;
				default:
				{
					Ast_Decl_Global* global_decl = parse_decl_global();
					if (!global_decl) return NULL;
					ast->globals.emplace_back(global_decl);
				} break;
				}
			}
			else
			{
				err_parse(TokenType::DOUBLE_COLON, "global declaration", 1);
				return NULL;
			}
		} break;
		case TokenType::KEYWORD_IMPORT:
		{
			Ast_Decl_Import_New* import_decl = parse_decl_import_new();
			if (!import_decl) return NULL;
		} break;
		case TokenType::INPUT_END: 
		{
			return ast;
		}
		default: 
		{
			err_parse(TokenType::IDENT, "global declaration");
			return NULL;
		}
		}
	}

	return ast;
}

option<Ast_Type> Parser::parse_type()
{
	Ast_Type type = {};
	span_start();

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
		span_end_dot(type);
		return type;
	}

	switch (token.type)
	{
	case TokenType::BRACKET_START:
	{
		consume();
		Ast_Type_Array* array = parse_type_array();
		if (!array) return {};
		type.tag = Ast_Type_Tag::Array;
		type.as_array = array;
	} break;
	case TokenType::PAREN_START:
	{
		consume();
		Ast_Type_Procedure* procedure = parse_type_procedure();
		if (!procedure) return {};
		type.tag = Ast_Type_Tag::Procedure;
		type.as_procedure = procedure;
	} break;
	case TokenType::IDENT:
	{
		Ast_Type_Unresolved* unresolved = parse_type_unresolved();
		if (!unresolved) return {};
		type.tag = Ast_Type_Tag::Unresolved;
		type.as_unresolved = unresolved;
	} break;
	default:
	{
		//@Err was: Expected basic type, type identifier or array type
		//sets arent supported by err reporting system
		err_parse(TokenType::IDENT, "type signature");
		return {};
	}
	}

	span_end_dot(type);
	return type;
}

Ast_Type_Array* Parser::parse_type_array()
{
	Ast_Type_Array* array_type = arena_alloc<Ast_Type_Array>(&this->arena);

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	array_type->size_expr = expr;

	if (!try_consume(TokenType::BRACKET_END)) { err_parse(TokenType::BRACKET_END, "array type signature"); return NULL; }

	option<Ast_Type> type = parse_type();
	if (!type) return NULL;
	array_type->element_type = type.value();

	return array_type;
}

Ast_Type_Procedure* Parser::parse_type_procedure()
{
	Ast_Type_Procedure* procedure = arena_alloc<Ast_Type_Procedure>(&this->arena);

	if (!try_consume(TokenType::PAREN_END))
	{
		while (true)
		{
			option<Ast_Type> type = parse_type();
			if (!type) return NULL;
			procedure->input_types.emplace_back(type.value());
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "procedure type signature"); return NULL; }
	}

	if (try_consume(TokenType::ARROW))
	{
		option<Ast_Type> type = parse_type();
		if (!type) return NULL;
		procedure->return_type = type.value();
	}

	return procedure;
}

Ast_Type_Unresolved* Parser::parse_type_unresolved()
{
	Ast_Type_Unresolved* unresolved = arena_alloc<Ast_Type_Unresolved>(&this->arena);

	Ast_Ident import = token_to_ident(consume_get());
	if (try_consume(TokenType::DOT))
	{
		unresolved->import = import;
		option<Token> ident = try_consume(TokenType::IDENT);
		if (!ident) { err_parse(TokenType::IDENT, "custom type signature"); return NULL; }
		unresolved->ident = token_to_ident(ident.value());
	}
	else unresolved->ident = import;

	return unresolved;
}

Ast_Decl_Impl* Parser::parse_decl_impl()
{
	Ast_Decl_Impl* impl_decl = arena_alloc<Ast_Decl_Impl>(&this->arena);

	//@maybe parse type unresolved (import + ident) instead of full type
	//to limit the use of basic types or arrays, etc
	option<Ast_Type> type = parse_type();
	if (!type) return NULL;
	impl_decl->type = type.value();

	if (!try_consume(TokenType::DOUBLE_COLON)) { err_parse(TokenType::DOUBLE_COLON, "impl block"); return NULL; }
	if (!try_consume(TokenType::KEYWORD_IMPL)) { err_parse(TokenType::KEYWORD_IMPL, "impl block"); return NULL; }
	if (!try_consume(TokenType::BLOCK_START))  { err_parse(TokenType::BLOCK_START, "impl block"); return NULL; }
	
	while (!try_consume(TokenType::BLOCK_END))
	{
		//@this is checked externally instead of parse_proc_decl checking this
		//@same pattern inside file level decl parsing, might change in the future
		if (peek().type != TokenType::IDENT)        { err_parse(TokenType::IDENT, "procedure declaration inside impl block"); return NULL; }
		if (peek(1).type != TokenType::DOUBLE_COLON) { err_parse(TokenType::DOUBLE_COLON, "procedure declaration inside impl block"); return NULL; }
		if (peek(2).type != TokenType::PAREN_START)  { err_parse(TokenType::PAREN_START, "procedure declaration inside impl block"); return NULL; }

		Ast_Decl_Proc* member_procedure = parse_decl_proc(true);
		if (!member_procedure) return NULL;
		impl_decl->member_procedures.emplace_back(member_procedure);
	}

	return impl_decl;
}

Ast_Decl_Use* Parser::parse_decl_use()
{
	Ast_Decl_Use* decl = arena_alloc<Ast_Decl_Use>(&this->arena);
	decl->alias = token_to_ident(consume_get());
	consume(); consume();

	option<Token> import = try_consume(TokenType::IDENT);
	if (!import) { err_parse(TokenType::IDENT, "import module of 'use' declaration"); return NULL; }
	decl->import = token_to_ident(import.value());

	if (!try_consume(TokenType::DOT)) { err_parse(TokenType::DOT, "'use' declaration"); return NULL; }
	
	option<Token> symbol = try_consume(TokenType::IDENT);
	if (!symbol) { err_parse(TokenType::IDENT, "symbol of 'use' declaration"); return NULL; }
	decl->symbol = token_to_ident(symbol.value());

	return decl;
}

Ast_Decl_Proc* Parser::parse_decl_proc(bool in_impl)
{
	Ast_Decl_Proc* decl = arena_alloc<Ast_Decl_Proc>(&this->arena);
	decl->is_member = in_impl;
	decl->ident = token_to_ident(consume_get());
	consume(); consume();

	while (true)
	{
		if (try_consume(TokenType::DOUBLE_DOT)) { decl->is_variadic = true; break; }

		Ast_Proc_Param param = {};

		if (peek().type == TokenType::KEYWORD_SELF)
		{
			param.self = true;
			param.ident = token_to_ident(consume_get());
			decl->input_params.emplace_back(param);
		}
		else
		{
			option<Token> ident = try_consume(TokenType::IDENT);
			if (!ident) break;
			param.ident = token_to_ident(ident.value());
			
			if (!try_consume(TokenType::COLON)) { err_parse(TokenType::COLON, "procedure parameter type definition"); return NULL; }
			
			option<Ast_Type> type = parse_type();
			if (!type) return NULL;
			param.type = type.value();
			
			decl->input_params.emplace_back(param);
		}
		
		if (!try_consume(TokenType::COMMA)) break;
	}
	if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "procedure declaration"); return NULL; }

	if (try_consume(TokenType::ARROW))
	{
		option<Ast_Type> type = parse_type();
		if (!type) return NULL;
		decl->return_type = type.value();
	}

	if (try_consume(TokenType::AT))
	{
		decl->is_external = true;
	}
	else
	{
		Ast_Stmt_Block* block = parse_stmt_block();
		if (!block) return NULL;
		decl->block = block;
	}

	return decl;
}

Ast_Decl_Enum* Parser::parse_decl_enum()
{
	Ast_Decl_Enum* decl = arena_alloc<Ast_Decl_Enum>(&this->arena);
	decl->ident = token_to_ident(consume_get());
	consume(); consume();

	if (try_consume(TokenType::DOUBLE_COLON))
	{
		option<BasicType> basic_type = token_to_basic_type(peek().type);
		//@Err need basic type set
		if (!basic_type) { err_parse(TokenType::TYPE_BOOL, "enum declaration"); return NULL; }
		consume();
		decl->basic_type = basic_type.value();
	}
	else decl->basic_type = BasicType::I32;

	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "enum declaration"); return NULL; }
	while (true)
	{
		option<Token> ident = try_consume(TokenType::IDENT);
		if (!ident) break;

		//@Todo optional default assignment with just ';'

		if (!try_consume(TokenType::ASSIGN)) { err_parse(TokenType::ASSIGN, "enum variant expression"); return NULL; }

		Ast_Expr* expr = parse_expr();
		if (!expr) return NULL;
		Ast_Consteval_Expr* const_expr = parse_consteval_expr(expr);
		decl->variants.emplace_back(Ast_Enum_Variant{ token_to_ident(ident.value()), const_expr });
	}
	if (!try_consume(TokenType::BLOCK_END)) { err_parse(TokenType::BLOCK_END, "enum declaration"); return NULL; }

	return decl;
}

Ast_Decl_Struct* Parser::parse_decl_struct()
{
	Ast_Decl_Struct* decl = arena_alloc<Ast_Decl_Struct>(&this->arena);
	decl->ident = token_to_ident(consume_get());
	consume(); consume();
	
	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "struct declaration"); return NULL; }
	while (true)
	{
		option<Token> field = try_consume(TokenType::IDENT);
		if (!field) break;
		if (!try_consume(TokenType::COLON)) { err_parse(TokenType::COLON, "struct field type definition"); return NULL; }

		option<Ast_Type> type = parse_type();
		if (!type) return NULL;

		if (try_consume(TokenType::ASSIGN))
		{
			Ast_Expr* expr = parse_expr();
			if (!expr) return NULL;
			decl->fields.emplace_back(Ast_Struct_Field { token_to_ident(field.value()), type.value(), expr });
		}
		else
		{
			decl->fields.emplace_back(Ast_Struct_Field { token_to_ident(field.value()), type.value(), {} });
			if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "struct field declaration"); return NULL; }
		}
	}
	if (!try_consume(TokenType::BLOCK_END)) { err_parse(TokenType::BLOCK_END, "struct declaration"); return NULL; }

	return decl;
}

Ast_Decl_Global* Parser::parse_decl_global()
{
	Ast_Decl_Global* decl = arena_alloc<Ast_Decl_Global>(&this->arena);
	decl->ident = token_to_ident(consume_get());
	consume();

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	decl->consteval_expr = parse_consteval_expr(expr);

	return decl;
}

Ast_Decl_Import* Parser::parse_decl_import()
{
	Ast_Decl_Import* decl = arena_alloc<Ast_Decl_Import>(&this->arena);
	decl->alias = token_to_ident(consume_get());
	consume(); consume();

	option<Token> token = try_consume(TokenType::STRING_LITERAL);
	if (!token) { err_parse(TokenType::STRING_LITERAL, "import path of 'import' declaration"); return NULL; }
	decl->file_path = Ast_Literal{ token.value() };

	return decl;
}

Ast_Stmt* Parser::parse_stmt()
{
	Ast_Stmt* statement = arena_alloc<Ast_Stmt>(&this->arena);
	Token token = peek();

	switch (token.type)
	{
	case TokenType::KEYWORD_IF:
	{
		statement->tag = Ast_Stmt_Tag::If;
		statement->as_if = parse_stmt_if();
		if (!statement->as_if) return NULL;
	} break;
	case TokenType::KEYWORD_FOR:
	{
		statement->tag = Ast_Stmt_Tag::For;
		statement->as_for = parse_stmt_for();
		if (!statement->as_for) return NULL;
	} break;
	case TokenType::BLOCK_START:
	{
		statement->tag = Ast_Stmt_Tag::Block;
		statement->as_block = parse_stmt_block();
		if (!statement->as_block) return NULL;
	} break;
	case TokenType::KEYWORD_DEFER:
	{
		statement->tag = Ast_Stmt_Tag::Defer;
		statement->as_defer = parse_stmt_defer();
		if (!statement->as_defer) return NULL;
	} break;
	case TokenType::KEYWORD_BREAK:
	{
		statement->tag = Ast_Stmt_Tag::Break;
		statement->as_break = parse_stmt_break();
		if (!statement->as_break) return NULL;
	} break;
	case TokenType::KEYWORD_RETURN:
	{
		statement->tag = Ast_Stmt_Tag::Return;
		statement->as_return = parse_stmt_return();
		if (!statement->as_return) return NULL;
	} break;
	case TokenType::KEYWORD_SWITCH:
	{
		statement->tag = Ast_Stmt_Tag::Switch;
		statement->as_switch = parse_stmt_switch();
		if (!statement->as_switch) return NULL;
	} break;
	case TokenType::KEYWORD_CONTINUE:
	{
		statement->tag = Ast_Stmt_Tag::Continue;
		statement->as_continue = parse_stmt_continue();
		if (!statement->as_continue) return NULL;
	} break;
	case TokenType::IDENT:
	{
		Token next = peek(1);
		Token next_2 = peek(2);
		Token next_3 = peek(3);
		bool import_prefix = next.type == TokenType::DOT && next_2.type == TokenType::IDENT;
		bool import_proc_call = import_prefix && next_3.type == TokenType::PAREN_START;

		if (next.type == TokenType::PAREN_START || import_proc_call)
		{
			statement->tag = Ast_Stmt_Tag::Proc_Call;
			statement->as_proc_call = parse_proc_call(import_proc_call);
			if (!statement->as_proc_call) return NULL;
			if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "procedure call statement"); return NULL; }
		}
		else if (next.type == TokenType::COLON)
		{
			statement->tag = Ast_Stmt_Tag::Var_Decl;
			statement->as_var_decl = parse_stmt_var_decl();
			if (!statement->as_var_decl) return NULL;
		}
		else
		{
			statement->tag = Ast_Stmt_Tag::Var_Assign;
			statement->as_var_assign = parse_stmt_var_assign();
			if (!statement->as_var_assign) return NULL;
		}
	} break;
	default: 
	{
		//@Err set Expected valid statement or '}' after code block
		err_parse(TokenType::BLOCK_END, "code block or a valid statement");
		return NULL;
	}
	}
	
	return statement;
}

Ast_Stmt_If* Parser::parse_stmt_if()
{
	Ast_Stmt_If* _if = arena_alloc<Ast_Stmt_If>(&this->arena);
	span_start();
	consume();

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	_if->condition_expr = expr;

	Ast_Stmt_Block* block = parse_stmt_block();
	if (!block) return NULL;
	_if->block = block;

	Token next = peek();
	if (next.type == TokenType::KEYWORD_ELSE)
	{
		Ast_Else* _else = parse_else();
		if (!_else) return NULL;
		_if->_else = _else;
	}

	span_end(_if);
	return _if;
}

Ast_Else* Parser::parse_else()
{
	Ast_Else* _else = arena_alloc<Ast_Else>(&this->arena);
	span_start();
	consume();
	
	Token token = peek();

	if (token.type == TokenType::KEYWORD_IF)
	{
		Ast_Stmt_If* _if = parse_stmt_if();
		if (!_if) return NULL;
		_else->tag = Ast_Else_Tag::If;
		_else->as_if = _if;
	}
	else if (token.type == TokenType::BLOCK_START)
	{
		Ast_Stmt_Block* block = parse_stmt_block();
		if (!block) return NULL;
		_else->tag = Ast_Else_Tag::Block;
		_else->as_block = block;
	}
	//@Err set Expected 'if' or code block '{ ... }
	else { err_parse(TokenType::KEYWORD_IF, "branch chain"); return NULL; }

	span_end(_else);
	return _else;
}

Ast_Stmt_For* Parser::parse_stmt_for()
{
	Ast_Stmt_For* _for = arena_alloc<Ast_Stmt_For>(&this->arena);
	span_start();
	consume();
	
	Token curr = peek();
	Token next = peek(1);

	if (curr.type == TokenType::BLOCK_START)
	{
		Ast_Stmt_Block* block = parse_stmt_block();
		if (!block) return NULL;
		_for->block = block;
		
		span_end(_for);
		return _for;
	}

	if (curr.type == TokenType::IDENT && next.type == TokenType::COLON)
	{
		Ast_Stmt_Var_Decl* var_decl = parse_stmt_var_decl();
		if (!var_decl) return NULL;
		_for->var_decl = var_decl;
	}

	Ast_Expr* condition_expr = parse_sub_expr();
	if (!condition_expr) return NULL; //@Err this was just more context "Expected conditional expression"
	_for->condition_expr = condition_expr;

	if (try_consume(TokenType::SEMICOLON))
	{
		Ast_Stmt_Var_Assign* var_assignment = parse_stmt_var_assign();
		if (!var_assignment) return NULL;
		_for->var_assign = var_assignment;
	}

	Ast_Stmt_Block* block = parse_stmt_block();
	if (!block) return NULL;
	_for->block = block;

	span_end(_for);
	return _for;
}

Ast_Stmt_Block* Parser::parse_stmt_block()
{
	Ast_Stmt_Block* block = arena_alloc<Ast_Stmt_Block>(&this->arena);

	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "code block"); return NULL; }
	while (true)
	{
		if (try_consume(TokenType::BLOCK_END)) return block;

		Ast_Stmt* statement = parse_stmt();
		if (!statement) return NULL;
		block->statements.emplace_back(statement);
	}
}

Ast_Stmt_Block* Parser::parse_stmt_block_short()
{
	if (peek().type == TokenType::BLOCK_START) return parse_stmt_block();

	Ast_Stmt_Block* block = arena_alloc<Ast_Stmt_Block>(&this->arena);

	Ast_Stmt* statement = parse_stmt();
	if (!statement) return NULL;
	block->statements.emplace_back(statement);

	return block;
}

Ast_Stmt_Defer* Parser::parse_stmt_defer()
{
	Ast_Stmt_Defer* defer = arena_alloc<Ast_Stmt_Defer>(&this->arena);
	span_start();
	consume();

	Ast_Stmt_Block* block = parse_stmt_block_short();
	if (!block) return NULL;
	defer->block = block;

	span_end(defer);
	return defer;
}

Ast_Stmt_Break* Parser::parse_stmt_break()
{
	Ast_Stmt_Break* _break = arena_alloc<Ast_Stmt_Break>(&this->arena);
	span_start();
	consume();

	if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "break statement"); return NULL; }
	
	span_end(_break);
	return _break;
}

Ast_Stmt_Return* Parser::parse_stmt_return()
{
	Ast_Stmt_Return* _return = arena_alloc<Ast_Stmt_Return>(&this->arena);
	span_start();
	consume();

	if (!try_consume(TokenType::SEMICOLON))
	{
		Ast_Expr* expr = parse_expr();
		if (!expr) return NULL;
		_return->expr = expr;
	}

	span_end(_return);
	return _return;
}

Ast_Stmt_Switch* Parser::parse_stmt_switch()
{
	Ast_Stmt_Switch* _switch = arena_alloc<Ast_Stmt_Switch>(&this->arena);
	span_start();
	consume();

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	_switch->expr = expr;

	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "switch statement"); return NULL; }
	
	while (true)
	{
		if (try_consume(TokenType::BLOCK_END)) break;

		Ast_Switch_Case switch_case = {};
		
		Ast_Expr* case_expr = parse_sub_expr();
		if (!case_expr) return NULL;
		switch_case.case_expr = case_expr;
		
		if (!try_consume(TokenType::COLON))
		{
			Ast_Stmt_Block* block = parse_stmt_block_short();
			if (!block) return NULL;
			switch_case.block = block;
		}

		_switch->cases.emplace_back(switch_case);
	}

	span_end(_switch);
	return _switch;
}

Ast_Stmt_Continue* Parser::parse_stmt_continue()
{
	Ast_Stmt_Continue* _continue = arena_alloc<Ast_Stmt_Continue>(&this->arena);
	span_start();
	consume();

	if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "continue statement"); return NULL; }
	
	span_end(_continue);
	return _continue;
}

Ast_Stmt_Var_Decl* Parser::parse_stmt_var_decl()
{
	Ast_Stmt_Var_Decl* var_decl = arena_alloc<Ast_Stmt_Var_Decl>(&this->arena);
	span_start();
	var_decl->ident = token_to_ident(consume_get());
	consume();

	bool infer_type = try_consume(TokenType::ASSIGN).has_value();

	if (!infer_type)
	{
		option<Ast_Type> type = parse_type();
		if (!type) return NULL;
		var_decl->type = type.value();

		if (try_consume(TokenType::SEMICOLON)) 
		{
			span_end(var_decl);
			return var_decl;
		}
		if (!try_consume(TokenType::ASSIGN)) { err_parse(TokenType::ASSIGN, "continue statement"); return NULL; } //@Err Expected '=' or ';' in a variable declaration
	}

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_decl->expr = expr;

	span_end(var_decl);
	return var_decl;
}

Ast_Stmt_Var_Assign* Parser::parse_stmt_var_assign()
{
	Ast_Stmt_Var_Assign* var_assign = arena_alloc<Ast_Stmt_Var_Assign>(&this->arena);
	span_start();

	Ast_Var* var = parse_var();
	if (!var) return NULL;
	var_assign->var = var;

	Token token = peek();
	option<AssignOp> op = token_to_assign_op(token.type);
	if (!op) { err_parse(TokenType::ASSIGN, "variable assignment statement"); return NULL; } //@Err set of assignment operators
	var_assign->op = op.value();
	consume();

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_assign->expr = expr;

	span_end(var_assign);
	return var_assign;
}

Ast_Proc_Call* Parser::parse_proc_call(bool import)
{
	Ast_Proc_Call* proc_call = arena_alloc<Ast_Proc_Call>(&this->arena);
	span_start();

	if (import) { proc_call->unresolved.import = token_to_ident(consume_get()); consume(); }
	proc_call->unresolved.ident = token_to_ident(consume_get());

	if (!try_consume(TokenType::PAREN_START)) { err_parse(TokenType::PAREN_START, "procedure call"); return NULL; }
	if (!try_consume(TokenType::PAREN_END))
	{
		while (true)
		{
			Ast_Expr* expr = parse_sub_expr();
			if (!expr) return NULL;
			proc_call->input_exprs.emplace_back(expr);
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "procedure call"); return NULL; }
	}

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		proc_call->access = access;
	}

	span_end(proc_call);
	return proc_call;
}

Ast_Expr* Parser::parse_expr()
{
	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "expression"); return NULL; }
	return expr;
}

Ast_Expr* Parser::parse_sub_expr(u32 min_prec)
{
	span_start();
	Ast_Expr* expr_lhs = parse_primary_expr();
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
		Ast_Expr* expr_rhs = parse_sub_expr(next_min_prec);
		if (expr_rhs == NULL) return NULL;

		Ast_Expr* expr_lhs_copy = arena_alloc<Ast_Expr>(&this->arena);
		expr_lhs_copy->tag = expr_lhs->tag;
		expr_lhs_copy->as_term = expr_lhs->as_term; //@Why copy both
		expr_lhs_copy->as_binary_expr = expr_lhs->as_binary_expr;

		Ast_Binary_Expr* bin_expr = arena_alloc<Ast_Binary_Expr>(&this->arena);
		bin_expr->op = op.value();
		bin_expr->left = expr_lhs_copy;
		bin_expr->right = expr_rhs;

		expr_lhs->tag = Ast_Expr_Tag::Binary;
		expr_lhs->as_binary_expr = bin_expr;
	}

	span_end(expr_lhs);
	return expr_lhs;
}

Ast_Expr* Parser::parse_primary_expr()
{
	if (try_consume(TokenType::PAREN_START))
	{
		Ast_Expr* expr = parse_sub_expr();
		if (!try_consume(TokenType::PAREN_END))
		{
			err_parse(TokenType::PAREN_END, "parenthesised expression");
			return NULL;
		}
		return expr;
	}

	Token token = peek();
	option<UnaryOp> op = token_to_unary_op(token.type);
	if (op)
	{
		consume();
		Ast_Expr* right_expr = parse_primary_expr();
		if (!right_expr) return NULL;

		Ast_Unary_Expr* unary_expr = arena_alloc<Ast_Unary_Expr>(&this->arena);
		unary_expr->op = op.value();
		unary_expr->right = right_expr;

		Ast_Expr* expr = arena_alloc<Ast_Expr>(&this->arena);
		expr->tag = Ast_Expr_Tag::Unary;
		expr->as_unary_expr = unary_expr;
		return expr;
	}

	Ast_Term* term = parse_term();
	if (!term) return NULL;

	Ast_Expr* expr = arena_alloc<Ast_Expr>(&this->arena);
	expr->tag = Ast_Expr_Tag::Term;
	expr->as_term = term;

	return expr;
}

Ast_Consteval_Expr* Parser::parse_consteval_expr(Ast_Expr* expr)
{
	Ast_Consteval_Expr* consteval_expr = arena_alloc<Ast_Consteval_Expr>(&this->arena);
	consteval_expr->eval = Consteval::Not_Evaluated;
	consteval_expr->expr = expr;
	expr->flags |= AST_EXPR_FLAG_CONST_BIT;
	return consteval_expr;
}

Ast_Term* Parser::parse_term()
{
	Ast_Term* term = arena_alloc<Ast_Term>(&this->arena);
	Token token = peek();

	switch (token.type)
	{
	case TokenType::BOOL_LITERAL:
	case TokenType::FLOAT_LITERAL:
	case TokenType::INTEGER_LITERAL:
	case TokenType::STRING_LITERAL:
	{
		Ast_Literal* literal = arena_alloc<Ast_Literal>(&this->arena);
		literal->token = token;
		term->tag = Ast_Term_Tag::Literal;
		term->as_literal = literal;
		consume();
	} break;
	case TokenType::DOT:
	{
		Ast_Struct_Init* struct_init = parse_struct_init(false, false);
		if (!struct_init) return NULL;
		term->tag = Ast_Term_Tag::Struct_Init;
		term->as_struct_init = struct_init;
	} break;
	case TokenType::BLOCK_START:
	case TokenType::BRACKET_START:
	{
		Ast_Array_Init* array_init = parse_array_init();
		if (!array_init) return NULL;
		term->tag = Ast_Term_Tag::Array_Init;
		term->as_array_init = array_init;
	} break;
	case TokenType::KEYWORD_CAST:
	{
		Ast_Cast* cast = parse_cast();
		if (!cast) return NULL;
		term->tag = Ast_Term_Tag::Cast;
		term->as_cast = cast;
	} break;
	case TokenType::KEYWORD_SIZEOF:
	{
		Ast_Sizeof* _sizeof = parse_sizeof();
		if (!_sizeof) return NULL;
		term->tag = Ast_Term_Tag::Sizeof;
		term->as_sizeof = _sizeof;
	} break;
	case TokenType::KEYWORD_SELF:
	{
		Ast_Var* var = parse_var();
		if (!var) return NULL;
		term->tag = Ast_Term_Tag::Var;
		term->as_var = var;
	} break;
	case TokenType::IDENT:
	{
		Token next = peek(1);
		Token next_2 = peek(2);
		Token next_3 = peek(3);
		bool import_prefix = next.type == TokenType::DOT && next_2.type == TokenType::IDENT;
		bool import_enum = import_prefix && next_3.type == TokenType::DOUBLE_COLON;
		bool import_proc_call = import_prefix && next_3.type == TokenType::PAREN_START;

		if (next.type == TokenType::DOUBLE_COLON || import_enum)
		{
			Ast_Enum* _enum = parse_enum(import_enum);
			if (!_enum) return NULL;
			term->tag = Ast_Term_Tag::Enum;
			term->as_enum = _enum;
		}
		else if (next.type == TokenType::PAREN_START || import_proc_call)
		{
			Ast_Proc_Call* proc_call = parse_proc_call(import_proc_call);
			if (!proc_call) return NULL;
			term->tag = Ast_Term_Tag::Proc_Call;
			term->as_proc_call = proc_call;
		}
		else
		{
			Token next_4 = peek(4);
			bool si_just_type = next.type == TokenType::DOT && next_2.type == TokenType::BLOCK_START;
			bool si_with_import = import_prefix && next_3.type == TokenType::DOT && next_4.type == TokenType::BLOCK_START;

			if (si_just_type || si_with_import)
			{
				Ast_Struct_Init* struct_init = parse_struct_init(si_with_import, true);
				if (!struct_init) return NULL;
				term->tag = Ast_Term_Tag::Struct_Init;
				term->as_struct_init = struct_init;
			}
			else
			{
				Ast_Var* var = parse_var();
				if (!var) return NULL;
				term->tag = Ast_Term_Tag::Var;
				term->as_var = var;
			}
		}
	} break;
	default:
	{
		//@Err set: Expected a valid expression term
		err_parse(TokenType::IDENT, "expression term");
		return NULL;
	}
	}

	return term;
}

Ast_Var* Parser::parse_var()
{
	Ast_Var* var = arena_alloc<Ast_Var>(&this->arena);

	if (peek().type == TokenType::KEYWORD_SELF)
	{
		var->unresolved.ident = token_to_ident(consume_get());
	}
	else
	{
		option<Token> ident = try_consume(TokenType::IDENT);
		if (!ident) { err_parse(TokenType::IDENT, "variable"); return NULL; }
		var->unresolved.ident = token_to_ident(ident.value());
	}

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		var->access = access;
	}

	return var;
}

Ast_Access* Parser::parse_access()
{
	Ast_Access* access = arena_alloc<Ast_Access>(&this->arena);
	Token token = peek();

	if (token.type == TokenType::DOT)
	{
		consume();
		Ast_Access_Var* var_access = parse_access_var(access);
		if (!var_access) return NULL;
		access->tag = Ast_Access_Tag::Var;
		access->as_var = var_access;
	}
	else if (token.type == TokenType::BRACKET_START)
	{
		consume();
		Ast_Access_Array* array_access = parse_access_array(access);
		if (!array_access) return NULL;
		access->tag = Ast_Access_Tag::Array;
		access->as_array = array_access;
	}
	else
	{
		//@Err set dot or bracker start
		err_parse(TokenType::DOT, "variable access");
		return NULL;
	}

	return access;
}

Ast_Access_Var* Parser::parse_access_var(Ast_Access* target)
{
	Ast_Access_Var* var_access = arena_alloc<Ast_Access_Var>(&this->arena);

	option<Token> ident = try_consume(TokenType::IDENT);
	if (!ident) { err_parse(TokenType::IDENT, "struct field access"); return NULL; }
	var_access->unresolved.ident = token_to_ident(ident.value());

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		target->next = access;
	}

	return var_access;
}

Ast_Access_Array* Parser::parse_access_array(Ast_Access* target)
{
	Ast_Access_Array* array_access = arena_alloc<Ast_Access_Array>(&this->arena);

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	array_access->index_expr = expr;

	if (!try_consume(TokenType::BRACKET_END)) { err_parse(TokenType::BRACKET_END, "array access"); return NULL; }

	Token token = peek();
	if (token.type == TokenType::DOT || token.type == TokenType::BRACKET_START)
	{
		Ast_Access* access = parse_access();
		if (!access) return NULL;
		target->next = access;
	}

	return array_access;
}

Ast_Enum* Parser::parse_enum(bool import)
{
	Ast_Enum* _enum = arena_alloc<Ast_Enum>(&this->arena);
	if (import) { _enum->unresolved.import = token_to_ident(consume_get()); consume(); }

	option<Token> ident = try_consume(TokenType::IDENT);
	if (!ident) { err_parse(TokenType::IDENT, "enum type declaration"); return NULL; }
	_enum->unresolved.ident = token_to_ident(ident.value());
	consume();
	
	option<Token> variant = try_consume(TokenType::IDENT);
	if (!variant) { err_parse(TokenType::IDENT, "enum variant declaration"); return NULL; }
	_enum->unresolved.variant = token_to_ident(variant.value());

	return _enum;
}

Ast_Cast* Parser::parse_cast()
{
	Ast_Cast* cast = arena_alloc<Ast_Cast>(&this->arena);
	consume();

	if (!try_consume(TokenType::PAREN_START)) { err_parse(TokenType::PAREN_START, "cast statement"); return NULL; }

	Token token = peek();
	option<BasicType> basic_type = token_to_basic_type(token.type);
	if (!basic_type) { err_parse(TokenType::TYPE_I8, "cast statement"); return NULL; } //@Error basic type set
	cast->basic_type = basic_type.value();
	consume();

	if (!try_consume(TokenType::COMMA)) { err_parse(TokenType::COMMA, "cast statement"); return NULL; }

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	cast->expr = expr;

	if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "cast statement"); return NULL; }

	return cast;
}

Ast_Sizeof* Parser::parse_sizeof()
{
	Ast_Sizeof* _sizeof = arena_alloc<Ast_Sizeof>(&this->arena);
	consume();

	if (!try_consume(TokenType::PAREN_START)) { err_parse(TokenType::PAREN_START, "sizeof statement"); return NULL; }
	
	option<Ast_Type> type = parse_type();
	if (!type) return NULL;
	_sizeof->type = type.value();

	if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "sizeof statement"); return NULL; }

	return _sizeof;
}

Ast_Struct_Init* Parser::parse_struct_init(bool import, bool type)
{
	Ast_Struct_Init* struct_init = arena_alloc<Ast_Struct_Init>(&this->arena);

	if (import) { struct_init->unresolved.import = token_to_ident(consume_get()); consume(); }
	if (type) { struct_init->unresolved.ident = token_to_ident(consume_get()); }
	consume();

	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "struct initializer"); return NULL; }
	if (!try_consume(TokenType::BLOCK_END))
	{
		while (true)
		{
			Ast_Expr* expr = parse_sub_expr();
			if (!expr) return NULL;
			struct_init->input_exprs.emplace_back(expr);
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::BLOCK_END)) { err_parse(TokenType::BLOCK_END, "struct initializer"); return NULL; }
	}

	return struct_init;
}

Ast_Array_Init* Parser::parse_array_init()
{
	Ast_Array_Init* array_init = arena_alloc<Ast_Array_Init>(&this->arena);

	if (peek().type == TokenType::BRACKET_START)
	{
		array_init->type = parse_type();
		if (!array_init->type) return NULL;
	}

	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "array initializer"); return NULL; }
	if (!try_consume(TokenType::BLOCK_END))
	{
		while (true)
		{
			Ast_Expr* expr = parse_sub_expr();
			if (!expr) return NULL;
			array_init->input_exprs.emplace_back(expr);
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::BLOCK_END)) { err_parse(TokenType::BLOCK_END, "array initializer"); return NULL; }
	}

	return array_init;
}

option<Ast_Module_Access*> Parser::parse_module_access()
{
	if (peek().type != TokenType::IDENT) return {};
	if (peek(1).type != TokenType::DOUBLE_COLON) return {};

	Ast_Module_Access* module_access = arena_alloc<Ast_Module_Access>(&this->arena);
	module_access->modules.emplace_back(token_to_ident(consume_get()));
	consume();

	while (peek().type == TokenType::IDENT && peek(1).type == TokenType::DOUBLE_COLON)
	{
		module_access->modules.emplace_back(token_to_ident(consume_get()));
		consume();
	}

	return module_access;
}

Ast_Decl_Import_New* Parser::parse_decl_import_new()
{
	if (!try_consume(TokenType::KEYWORD_IMPORT)) { err_parse(TokenType::KEYWORD_IMPORT, "import declaration"); return NULL; }

	Ast_Decl_Import_New* import_decl = arena_alloc<Ast_Decl_Import_New>(&this->arena);
	import_decl->module_access = parse_module_access();

	switch (peek().type)
	{
	case TokenType::IDENT:
	{
		import_decl->tag = Ast_Resolve_Import_Tag::Unresolved;
		import_decl->unresolved.ident = token_to_ident(consume_get());
	} break;
	case TokenType::TIMES: //@require module access to exist
	{
		import_decl->tag = Ast_Resolve_Import_Tag::Resolved_Wildcard;
		consume();
	} break;
	case TokenType::BLOCK_START: //@require module access to exist
	{
		import_decl->tag = Ast_Resolve_Import_Tag::Resolved_Symbol_List;
		consume();
		while (peek().type == TokenType::IDENT)
		{
			import_decl->resolved_symbol_list.symbols.emplace_back(token_to_ident(consume_get()));
			if (!try_consume(TokenType::COMMA)) break; //@disallow trailing comma {ident, ident, }
		}
		if (!try_consume(TokenType::BLOCK_END)) { err_parse(TokenType::BLOCK_END, "import declaration"); return NULL; }
	} break;
	default: { err_parse(TokenType::IDENT, "import declaration"); return NULL; } //@err set ident, *, {
	}
	
	if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "import declaration"); return NULL; }
	return import_decl;
}

Token Parser::peek(u32 offset)
{
	return this->tokens[this->peek_index + offset];
}

void Parser::consume()
{
	this->peek_index += 1;
	if (this->peek_index >= (Lexer::TOKEN_BUFFER_SIZE - Lexer::TOKEN_LOOKAHEAD))
	{
		this->peek_index = 0;
		this->prev_last = this->tokens[Lexer::TOKEN_BUFFER_SIZE - Lexer::TOKEN_LOOKAHEAD - 1]; //@Hack
		this->lexer.lex_token_buffer(this->tokens);
	}
}

Token Parser::consume_get()
{
	Token token = peek();
	consume();
	return token;
}

option<Token> Parser::try_consume(TokenType token_type)
{
	Token token = peek();
	if (token.type == token_type)
	{
		consume();
		return token;
	}
	return {};
}

u32 Parser::get_span_start()
{
	return this->tokens[this->peek_index].span.start;
}

u32 Parser::get_span_end()
{
	if (this->peek_index == 0) return this->prev_last.span.end; //@Hack saving last on tokenization
	return this->tokens[this->peek_index - 1].span.end;
}

void Parser::err_parse(TokenType expected, option<const char*> in, u32 offset)
{
	err_report_parse(this->ast, expected, in, peek(offset));
}
