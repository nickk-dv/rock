export module parser;

import general;
import ast;
import token;
import lexer;
import err_handler;
import <chrono>;
import <filesystem>;

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

Ast_Ident token_to_ident(const Token& token)
{
	return Ast_Ident{ token.span, token.string_value };
}

namespace fs = std::filesystem;

export struct Parser
{
private:
	Ast* ast;
	Arena arena;
	Lexer lexer;
	StringStorage strings;
	u32 peek_index = 0;
	Token prev_last;
	Token tokens[Lexer::TOKEN_BUFFER_SIZE];

public:
	Ast_Program* parse_program();
	void populate_module_tree(Ast_Program* program, Ast_Module_Tree* parent, fs::path& path, fs::path& src);

private:
	Ast* parse_ast(StringView source, std::string& filepath);
	option<Ast_Type> parse_type();
	Ast_Type_Array* parse_type_array();
	Ast_Type_Procedure* parse_type_procedure();
	Ast_Type_Unresolved* parse_type_unresolved();

	Ast_Decl* parse_decl();
	Ast_Decl_Impl* parse_decl_impl();
	Ast_Decl_Proc* parse_decl_proc(bool in_impl);
	Ast_Decl_Enum* parse_decl_enum();
	Ast_Decl_Struct* parse_decl_struct();
	Ast_Decl_Global* parse_decl_global();
	Ast_Decl_Import* parse_decl_import();
	Ast_Import_Target* parse_import_target();
	option<Ast_Module_Access*> parse_module_access();

	Ast_Stmt* parse_stmt();
	Ast_Stmt_If* parse_stmt_if();
	Ast_Else* parse_else();
	Ast_Stmt_For* parse_stmt_for();
	Ast_Stmt_Block* parse_stmt_block();
	Ast_Stmt_Block* parse_stmt_block_short();
	Ast_Stmt_Defer* parse_stmt_defer();
	Ast_Stmt_Break* parse_stmt_break();
	Ast_Stmt_Return* parse_stmt_return();
	Ast_Stmt_Switch* parse_stmt_switch();
	Ast_Stmt_Continue* parse_stmt_continue();
	Ast_Stmt_Var_Decl* parse_stmt_var_decl();

	Ast_Expr* parse_expr();
	Ast_Expr* parse_sub_expr(u32 min_prec = 0);
	Ast_Expr* parse_primary_expr();
	Ast_Expr_List* parse_expr_list(TokenType start, TokenType end, const char* in);
	Ast_Consteval_Expr* parse_consteval_expr(Ast_Expr* expr);
	Ast_Term* parse_term();
	Ast_Enum* parse_enum();
	Ast_Cast* parse_cast();
	Ast_Sizeof* parse_sizeof();
	Ast_Something* parse_something(option<Ast_Module_Access*> module_access);
	Ast_Array_Init* parse_array_init();
	Ast_Struct_Init* parse_struct_init(option<Ast_Module_Access*> module_access);
	Ast_Access* parse_access_first();
	bool parse_access(Ast_Access* prev);

	TokenType peek(u32 offset = 0);
	Token peek_token(u32 offset = 0);
	void consume();
	Token consume_get();
	option<Token> try_consume(TokenType token_type);
	u32 get_span_start();
	u32 get_span_end();
	void err_parse(TokenType expected, option<const char*> in, u32 offset = 0);
};

constexpr option<UnaryOp> token_to_unary_op(TokenType type)
{
	switch (type)
	{
	case TokenType::MINUS:         return UnaryOp::MINUS;
	case TokenType::LOGIC_NOT:     return UnaryOp::LOGIC_NOT;
	case TokenType::BITWISE_NOT:   return UnaryOp::BITWISE_NOT;
	case TokenType::TIMES:         return UnaryOp::ADDRESS_OF;
	case TokenType::BITSHIFT_LEFT: return UnaryOp::DEREFERENCE;
	default: return {};
	}
}

constexpr option<BinaryOp> token_to_binary_op(TokenType type)
{
	switch (type)
	{
	case TokenType::LOGIC_AND:      return BinaryOp::LOGIC_AND;
	case TokenType::LOGIC_OR:       return BinaryOp::LOGIC_OR;
	case TokenType::LESS:           return BinaryOp::LESS;
	case TokenType::GREATER:        return BinaryOp::GREATER;
	case TokenType::LESS_EQUALS:    return BinaryOp::LESS_EQUALS;
	case TokenType::GREATER_EQUALS: return BinaryOp::GREATER_EQUALS;
	case TokenType::IS_EQUALS:      return BinaryOp::IS_EQUALS;
	case TokenType::NOT_EQUALS:     return BinaryOp::NOT_EQUALS;
	case TokenType::PLUS:           return BinaryOp::PLUS;
	case TokenType::MINUS:          return BinaryOp::MINUS;
	case TokenType::TIMES:          return BinaryOp::TIMES;
	case TokenType::DIV:            return BinaryOp::DIV;
	case TokenType::MOD:            return BinaryOp::MOD;
	case TokenType::BITWISE_AND:    return BinaryOp::BITWISE_AND;
	case TokenType::BITWISE_OR:     return BinaryOp::BITWISE_OR;
	case TokenType::BITWISE_XOR:    return BinaryOp::BITWISE_XOR;
	case TokenType::BITSHIFT_LEFT:  return BinaryOp::BITSHIFT_LEFT;
	case TokenType::BITSHIFT_RIGHT: return BinaryOp::BITSHIFT_RIGHT;
	default: return {};
	}
}

constexpr option<AssignOp> token_to_assign_op(TokenType type)
{
	switch (type)
	{
	case TokenType::ASSIGN:                return AssignOp::ASSIGN;
	case TokenType::PLUS_EQUALS:           return AssignOp::PLUS;
	case TokenType::MINUS_EQUALS:          return AssignOp::MINUS;
	case TokenType::TIMES_EQUALS:          return AssignOp::TIMES;
	case TokenType::DIV_EQUALS:            return AssignOp::DIV;
	case TokenType::MOD_EQUALS:            return AssignOp::MOD;
	case TokenType::BITWISE_AND_EQUALS:    return AssignOp::BITWISE_AND;
	case TokenType::BITWISE_OR_EQUALS:     return AssignOp::BITWISE_OR;
	case TokenType::BITWISE_XOR_EQUALS:    return AssignOp::BITWISE_XOR;
	case TokenType::BITSHIFT_LEFT_EQUALS:  return AssignOp::BITSHIFT_LEFT;
	case TokenType::BITSHIFT_RIGHT_EQUALS: return AssignOp::BITSHIFT_RIGHT;
	default: return {};
	}
}

constexpr option<BasicType> token_to_basic_type(TokenType type)
{
	switch (type)
	{
	case TokenType::TYPE_I8:     return BasicType::I8;
	case TokenType::TYPE_U8:     return BasicType::U8;
	case TokenType::TYPE_I16:    return BasicType::I16;
	case TokenType::TYPE_U16:    return BasicType::U16;
	case TokenType::TYPE_I32:    return BasicType::I32;
	case TokenType::TYPE_U32:    return BasicType::U32;
	case TokenType::TYPE_I64:    return BasicType::I64;
	case TokenType::TYPE_U64:    return BasicType::U64;
	case TokenType::TYPE_F32:    return BasicType::F32;
	case TokenType::TYPE_F64:    return BasicType::F64;
	case TokenType::TYPE_BOOL:   return BasicType::BOOL;
	case TokenType::TYPE_STRING: return BasicType::STRING;
	default: return {};
	}
}

constexpr u32 token_binary_op_prec(BinaryOp binary_op)
{
	switch (binary_op)
	{
	case BinaryOp::LOGIC_AND:
	case BinaryOp::LOGIC_OR:
		return 0;
	case BinaryOp::LESS:
	case BinaryOp::GREATER:
	case BinaryOp::LESS_EQUALS:
	case BinaryOp::GREATER_EQUALS:
	case BinaryOp::IS_EQUALS:
	case BinaryOp::NOT_EQUALS:
		return 1;
	case BinaryOp::PLUS:
	case BinaryOp::MINUS:
		return 2;
	case BinaryOp::TIMES:
	case BinaryOp::DIV:
	case BinaryOp::MOD:
		return 3;
	case BinaryOp::BITWISE_AND:
	case BinaryOp::BITWISE_OR:
	case BinaryOp::BITWISE_XOR:
		return 4;
	case BinaryOp::BITSHIFT_LEFT:
	case BinaryOp::BITSHIFT_RIGHT:
		return 5;
	default:
	{
		err_report(Error::COMPILER_INTERNAL);
		err_context("unexpected switch case in token_binary_op_prec");
		return 0;
	}
	}
}

namespace fs = std::filesystem;

//@todo same module name within same path shoundt be allowed
//core::mem (as folder)
//core::mem (as file) cannot exist at the same time
void Parser::populate_module_tree(Ast_Program* program, Ast_Module_Tree* parent, fs::path& path, fs::path& src)
{
	for (const fs::directory_entry& dir_entry : fs::directory_iterator(path))
	{
		parent->submodules.emplace_back(Ast_Module_Tree {});
		Ast_Module_Tree* module = &parent->submodules[parent->submodules.size() - 1];
		
		fs::path entry = dir_entry.path();
		module->name = entry.lexically_relative(path).replace_extension("").filename().string();

		if (dir_entry.is_directory())
		{
			module->leaf_ast = {};
			populate_module_tree(program, module, entry, src);
		}
		else if (fs::is_regular_file(entry))
		{
			FILE* file;
			fopen_s(&file, (const char*)entry.u8string().c_str(), "rb");
			if (!file) { err_report(Error::OS_FILE_OPEN_FAILED); continue; } //@add context
			fseek(file, 0, SEEK_END);
			u64 size = (u64)ftell(file);
			fseek(file, 0, SEEK_SET);

			u8* data = arena_alloc_buffer<u8>(&this->arena, size);
			u64 read_size = fread(data, 1, size, file);
			fclose(file);
			if (read_size != size) { err_report(Error::OS_FILE_READ_FAILED); continue; } //@add context

			StringView source = StringView{ data, size };
			std::string filepath = entry.lexically_relative(src).replace_extension("").string();
			Ast* ast = parse_ast(source, filepath);
			if (ast == NULL) continue;

			module->leaf_ast = ast;
			program->modules.emplace_back(ast);
		}
		else
		{
			//@error ?
		}
	}
}

void module_tree_finalize_idents(Ast_Module_Tree* parent)
{
	for (Ast_Module_Tree& module : parent->submodules) module.ident = Ast_Ident { .str = string_view_from_string(module.name) };
	for (Ast_Module_Tree& module : parent->submodules) module_tree_finalize_idents(&module);
}

Ast_Program* Parser::parse_program()
{
	ScopedTimer timer = ScopedTimer("parse files");
	
	fs::path src = fs::path("src");
	if (!fs::exists(src)) { err_report(Error::PARSE_SRC_DIR_NOT_FOUND); return NULL; }
	
	this->strings.init();
	arena_init(&this->arena, 4 * 1024 * 1024);
	Ast_Program* program = arena_alloc<Ast_Program>(&this->arena);

	populate_module_tree(program, &program->root, src, src);
	module_tree_finalize_idents(&program->root);

	/*
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
		printf("parse file: %s\n", filepath.c_str());
		Ast* ast = parse_ast(source, filepath);
		if (ast == NULL) return NULL;
		
		program->modules.emplace_back(ast);
	}
	*/

	if (!fs::exists("build") && !fs::create_directory("build")) 
	{ err_report(Error::OS_DIR_CREATE_FAILED); return NULL; } //@add context
	fs::current_path("build");

	return program;
}

Ast* Parser::parse_ast(StringView source, std::string& filepath)
{
	Ast* ast = arena_alloc<Ast>(&this->arena);
	ast->source = arena_alloc<Ast_Source>(&this->arena);
	ast->source->str = source;
	ast->source->filepath = std::string(filepath);
	
	this->ast = ast;
	this->peek_index = 0;
	this->lexer.init(source, &this->strings, &ast->source->line_spans);
	this->lexer.lex_token_buffer(this->tokens);

	while (peek() != TokenType::INPUT_END)
	{
		Ast_Decl* decl = parse_decl();
		if (!decl) return NULL;
		ast->decls.emplace_back(decl);
	}

	return ast;
}

option<Ast_Type> Parser::parse_type()
{
	Ast_Type type = {};

	while (peek() == TokenType::TIMES)
	{
		consume();
		type.pointer_level += 1;
	}

	option<BasicType> basic_type = token_to_basic_type(peek());
	if (basic_type)
	{
		consume();
		type.set_basic(basic_type.value());
		return type;
	}

	switch (peek())
	{
	case TokenType::BRACKET_START:
	{
		consume();
		Ast_Type_Array* type_array = parse_type_array();
		if (!type_array) return {};
		type.set_array(type_array);
	} break;
	case TokenType::PAREN_START:
	{
		consume();
		Ast_Type_Procedure* type_procedure = parse_type_procedure();
		if (!type_procedure) return {};
		type.set_procedure(type_procedure);
	} break;
	case TokenType::IDENT:
	{
		Ast_Type_Unresolved* type_unresolved = parse_type_unresolved();
		if (!type_unresolved) return {};
		type.set_unresolved(type_unresolved);
	} break;
	default:
	{
		//@Err: Expected basic type, type identifier or array type
		//err sets arent supported by err reporting system
		err_parse(TokenType::IDENT, "type signature");
		return {};
	}
	}

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
			if (peek() == TokenType::IDENT && peek(1) == TokenType::COLON) { consume(); consume(); }
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
	unresolved->module_access = parse_module_access();

	option<Token> ident = try_consume(TokenType::IDENT);
	if (!ident) { err_parse(TokenType::IDENT, "custom type signature"); return NULL; }
	unresolved->ident = token_to_ident(ident.value());

	return unresolved;
}

Ast_Decl* Parser::parse_decl()
{
	Ast_Decl* decl = arena_alloc<Ast_Decl>(&this->arena);
	
	switch (peek())
	{
	case TokenType::IDENT:
	{
		if (peek(1) == TokenType::DOUBLE_COLON)
		{
			switch (peek(2))
			{
			case TokenType::PAREN_START:
			{
				Ast_Decl_Proc* decl_proc = parse_decl_proc(false);
				if (!decl_proc) return NULL;
				decl->set_proc(decl_proc);
			} break;
			case TokenType::KEYWORD_ENUM:
			{
				Ast_Decl_Enum* decl_enum = parse_decl_enum();
				if (!decl_enum) return NULL;
				decl->set_enum(decl_enum);
			} break;
			case TokenType::KEYWORD_STRUCT:
			{
				Ast_Decl_Struct* decl_struct = parse_decl_struct();
				if (!decl_struct) return NULL;
				decl->set_struct(decl_struct);
			} break;
			default:
			{
				Ast_Decl_Global* decl_global = parse_decl_global();
				if (!decl_global) return NULL;
				decl->set_global(decl_global);
			} break;
			}
		}
		else { err_parse(TokenType::DOUBLE_COLON, "global declaration", 1); return NULL; }
	} break;
	case TokenType::KEYWORD_IMPL: //@when using :: style the :: of decl was parsed as part of the type, causing a missing ident error
	{
		Ast_Decl_Impl* decl_impl = parse_decl_impl();
		if (!decl_impl) return NULL;
		decl->set_impl(decl_impl);
	} break;
	case TokenType::KEYWORD_IMPORT:
	{
		Ast_Decl_Import* decl_import = parse_decl_import();
		if (!decl_import) return NULL;
		decl->set_import(decl_import);
	} break;
	default: { err_parse(TokenType::IDENT, "global declaration", 1); return NULL; } //@err set ident or impl or import
	}

	return decl;
}

Ast_Decl_Impl* Parser::parse_decl_impl()
{
	if (!try_consume(TokenType::KEYWORD_IMPL)) { err_parse(TokenType::KEYWORD_IMPL, "impl block"); return NULL; }
	Ast_Decl_Impl* impl_decl = arena_alloc<Ast_Decl_Impl>(&this->arena);

	//@maybe parse type unresolved (import + ident) instead of full type
	//to limit the use of basic types or arrays, etc
	option<Ast_Type> type = parse_type();
	if (!type) return NULL;
	impl_decl->type = type.value();

	//if (!try_consume(TokenType::DOUBLE_COLON)) { err_parse(TokenType::DOUBLE_COLON, "impl block"); return NULL; }
	if (!try_consume(TokenType::BLOCK_START))  { err_parse(TokenType::BLOCK_START, "impl block"); return NULL; }
	
	while (!try_consume(TokenType::BLOCK_END))
	{
		//@this is checked externally instead of parse_proc_decl checking this
		//@same pattern inside file level decl parsing, might change in the future
		if (peek() != TokenType::IDENT)        { err_parse(TokenType::IDENT, "procedure declaration inside impl block"); return NULL; }
		if (peek(1) != TokenType::DOUBLE_COLON) { err_parse(TokenType::DOUBLE_COLON, "procedure declaration inside impl block"); return NULL; }
		if (peek(2) != TokenType::PAREN_START)  { err_parse(TokenType::PAREN_START, "procedure declaration inside impl block"); return NULL; }

		Ast_Decl_Proc* member_procedure = parse_decl_proc(true);
		if (!member_procedure) return NULL;
		impl_decl->member_procedures.emplace_back(member_procedure);
	}

	return impl_decl;
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

		if (peek() == TokenType::KEYWORD_SELF)
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
		option<BasicType> basic_type = token_to_basic_type(peek());
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
	if (!try_consume(TokenType::KEYWORD_IMPORT)) { err_parse(TokenType::KEYWORD_IMPORT, "import declaration"); return NULL; }

	option<Token> first_module = try_consume(TokenType::IDENT);
	if (!first_module) { err_parse(TokenType::IDENT, "import declaration"); return NULL; }
	decl->modules.emplace_back(token_to_ident(first_module.value()));

	if (try_consume(TokenType::SEMICOLON)) return decl;
	if (!try_consume(TokenType::DOUBLE_COLON)) { err_parse(TokenType::DOUBLE_COLON, "import declaration"); return NULL; }

	while (peek() == TokenType::IDENT && peek(1) == TokenType::DOUBLE_COLON)
	{
		decl->modules.emplace_back(token_to_ident(consume_get()));
		consume();
	}

	Ast_Import_Target* target = parse_import_target();
	if (!target) return NULL;
	decl->target = target;

	if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "import declaration"); return NULL; }
	return decl;
}

Ast_Import_Target* Parser::parse_import_target()
{
	Ast_Import_Target* target = arena_alloc<Ast_Import_Target>(&this->arena);

	switch (peek())
	{
	case TokenType::TIMES:
	{
		consume();
		target->set_wildcard();
	} break;
	case TokenType::BLOCK_START:
	{
		consume();
		target->set_symbol_list();
		if (!try_consume(TokenType::BLOCK_END))
		{
			while (true)
			{
				option<Token> symbol = try_consume(TokenType::IDENT);
				if (!symbol) return NULL;
				target->as_symbol_list.symbols.emplace_back(token_to_ident(symbol.value()));
				if (!try_consume(TokenType::COMMA)) break;
			}
			if (!try_consume(TokenType::BLOCK_END)) { err_parse(TokenType::BLOCK_END, "import declaration"); return NULL; }
		}
	} break;
	case TokenType::IDENT:
	{
		target->set_symbol_or_module(token_to_ident(consume_get()));
	} break;
	default: { err_parse(TokenType::IDENT, "import declaration"); return NULL; } //@err set ident, *, {
	}

	return target;
}

option<Ast_Module_Access*> Parser::parse_module_access()
{
	if (peek() != TokenType::IDENT) return {};
	if (peek(1) != TokenType::DOUBLE_COLON) return {};

	Ast_Module_Access* module_access = arena_alloc<Ast_Module_Access>(&this->arena);
	module_access->modules.emplace_back(token_to_ident(consume_get()));
	consume();

	while (peek() == TokenType::IDENT && peek(1) == TokenType::DOUBLE_COLON)
	{
		module_access->modules.emplace_back(token_to_ident(consume_get()));
		consume();
	}

	return module_access;
}

Ast_Stmt* Parser::parse_stmt()
{
	Ast_Stmt* stmt = arena_alloc<Ast_Stmt>(&this->arena);

	switch (peek())
	{
	case TokenType::KEYWORD_IF:
	{
		Ast_Stmt_If* _if = parse_stmt_if();
		if (!_if) return NULL;
		stmt->set_if(_if);
	} break;
	case TokenType::KEYWORD_FOR:
	{
		Ast_Stmt_For* _for = parse_stmt_for();
		if (!_for) return NULL;
		stmt->set_for(_for);
	} break;
	case TokenType::BLOCK_START:
	{
		Ast_Stmt_For* _for = parse_stmt_for();
		if (!_for) return NULL;
		stmt->set_for(_for);
	} break;
	case TokenType::KEYWORD_DEFER:
	{
		Ast_Stmt_Defer* defer = parse_stmt_defer();
		if (!defer) return NULL;
		stmt->set_defer(defer);
	} break;
	case TokenType::KEYWORD_BREAK:
	{
		Ast_Stmt_Break* _break = parse_stmt_break();
		if (!_break) return NULL;
		stmt->set_break(_break);
	} break;
	case TokenType::KEYWORD_RETURN:
	{
		Ast_Stmt_Return* _return = parse_stmt_return();
		if (!_return) return NULL;
		stmt->set_return(_return);
	} break;
	case TokenType::KEYWORD_SWITCH:
	{
		Ast_Stmt_Switch* _switch = parse_stmt_switch();
		if (!_switch) return NULL;
		stmt->set_switch(_switch);
	} break;
	case TokenType::KEYWORD_CONTINUE:
	{
		Ast_Stmt_Continue* _continue = parse_stmt_continue();
		if (!_continue) return NULL;
		stmt->set_continue(_continue);
	} break;
	default: 
	{
		if (peek() == TokenType::IDENT && peek(1) == TokenType::COLON)
		{
			Ast_Stmt_Var_Decl* var_decl = parse_stmt_var_decl();
			if (!var_decl) return NULL;
			stmt->set_var_decl(var_decl);
			break;
		}

		span_start();
		Ast_Something* something = parse_something(parse_module_access());
		if (!something) return NULL;

		if (try_consume(TokenType::SEMICOLON))
		{
			Ast_Stmt_Proc_Call* proc_call = arena_alloc<Ast_Stmt_Proc_Call>(&this->arena);
			proc_call->something = something;
			stmt->set_proc_call(proc_call);
			break;
		}
		else
		{
			Ast_Stmt_Var_Assign* var_assign = arena_alloc<Ast_Stmt_Var_Assign>(&this->arena);
			var_assign->something = something;

			option<AssignOp> op = token_to_assign_op(peek());
			if (!op) { err_parse(TokenType::ASSIGN, "variable assignment statement"); return NULL; } //@Err set of assignment operators
			var_assign->op = op.value();
			consume();

			Ast_Expr* expr = parse_expr();
			if (!expr) return NULL;
			var_assign->expr = expr;

			stmt->set_var_assign(var_assign);
			span_end(var_assign);
		}
	} break;
	}
	
	return stmt;
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

	if (peek() == TokenType::KEYWORD_ELSE)
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
	
	switch (peek())
	{
	case TokenType::KEYWORD_IF:
	{
		Ast_Stmt_If* _if = parse_stmt_if();
		if (!_if) return NULL;
		_else->set_if(_if);
	} break;
	case TokenType::BLOCK_START:
	{
		Ast_Stmt_Block* block = parse_stmt_block();
		if (!block) return NULL;
		_else->set_block(block);
	} break;
	default: { err_parse(TokenType::KEYWORD_IF, "branch chain"); return NULL; } //@Err set Expected 'if' or code block '{ ... }
	}

	span_end(_else);
	return _else;
}

Ast_Stmt_For* Parser::parse_stmt_for()
{
	Ast_Stmt_For* _for = arena_alloc<Ast_Stmt_For>(&this->arena);
	span_start();
	consume();
	
	if (peek() == TokenType::BLOCK_START)
	{
		Ast_Stmt_Block* block = parse_stmt_block();
		if (!block) return NULL;
		_for->block = block;
		
		span_end(_for);
		return _for;
	}

	if (peek() == TokenType::IDENT && peek(1) == TokenType::COLON)
	{
		Ast_Stmt_Var_Decl* var_decl = parse_stmt_var_decl();
		if (!var_decl) return NULL;
		_for->var_decl = var_decl;
	}

	Ast_Expr* condition_expr = parse_expr(); //@using full expr with ; temp
	if (!condition_expr) return NULL; //@Err this was just more context "Expected conditional expression"
	_for->condition_expr = condition_expr;

	//@hardcoded var assign parsing same as in parse_stmt
	u32 start_2 = get_span_start();
	Ast_Something* something = parse_something(parse_module_access());
	if (!something) return NULL;
	Ast_Stmt_Var_Assign* var_assign = arena_alloc<Ast_Stmt_Var_Assign>(&this->arena);
	var_assign->something = something;

	option<AssignOp> op = token_to_assign_op(peek());
	if (!op) { err_parse(TokenType::ASSIGN, "variable assignment statement"); return NULL; } //@Err set of assignment operators
	var_assign->op = op.value();
	consume();

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_assign->expr = expr;

	var_assign->span.start = start_2;
	var_assign->span.end = get_span_end();
	_for->var_assign = var_assign;
	//@end

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
	if (peek() == TokenType::BLOCK_START) return parse_stmt_block();

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
		if (!try_consume(TokenType::ASSIGN)) { err_parse(TokenType::ASSIGN, "var decl statement"); return NULL; } //@Err Expected '=' or ';' in a variable declaration
	}

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_decl->expr = expr;

	span_end(var_decl);
	return var_decl;
}

Ast_Expr* Parser::parse_expr()
{
	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "expression"); return NULL; }
	return expr;
}

Ast_Expr* Parser::parse_sub_expr(u32 min_prec) //@incorect spans
{
	span_start();
	Ast_Expr* expr_lhs = parse_primary_expr();
	if (!expr_lhs) return NULL;

	while (true)
	{
		option<BinaryOp> op = token_to_binary_op(peek());
		if (!op) break;
		u32 prec = token_binary_op_prec(op.value());
		if (prec < min_prec) break;
		consume();

		Ast_Expr* expr_rhs = parse_sub_expr(prec + 1);
		if (expr_rhs == NULL) return NULL;

		Ast_Expr* expr_lhs_copy = arena_alloc<Ast_Expr>(&this->arena);
		expr_lhs_copy->ptr_tag_copy(expr_lhs);

		Ast_Binary_Expr* bin_expr = arena_alloc<Ast_Binary_Expr>(&this->arena);
		bin_expr->op = op.value();
		bin_expr->left = expr_lhs_copy;
		bin_expr->right = expr_rhs;

		expr_lhs->set_binary(bin_expr);
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

	option<UnaryOp> op = token_to_unary_op(peek());
	if (op)
	{
		consume();
		Ast_Expr* right_expr = parse_primary_expr();
		if (!right_expr) return NULL;

		Ast_Unary_Expr* unary_expr = arena_alloc<Ast_Unary_Expr>(&this->arena);
		unary_expr->op = op.value();
		unary_expr->right = right_expr;

		Ast_Expr* expr = arena_alloc<Ast_Expr>(&this->arena);
		expr->set_unary(unary_expr);
		return expr;
	}

	Ast_Term* term = parse_term();
	if (!term) return NULL;
	
	Ast_Expr* expr = arena_alloc<Ast_Expr>(&this->arena);
	expr->set_term(term);
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
	
	switch (peek())
	{
	case TokenType::KEYWORD_CAST:
	{
		Ast_Cast* cast = parse_cast();
		if (!cast) return NULL;
		term->set_cast(cast);
	} break;
	case TokenType::KEYWORD_SIZEOF:
	{
		Ast_Sizeof* size_of = parse_sizeof();
		if (!size_of) return NULL;
		term->set_sizeof(size_of);
	} break;
	case TokenType::BOOL_LITERAL:
	case TokenType::FLOAT_LITERAL:
	case TokenType::INTEGER_LITERAL:
	case TokenType::STRING_LITERAL:
	{
		Ast_Literal* literal = arena_alloc<Ast_Literal>(&this->arena);
		literal->token = consume_get();
		term->set_literal(literal);
	} break;
	case TokenType::BLOCK_START:
	case TokenType::BRACKET_START:
	{
		Ast_Array_Init* array_init = parse_array_init();
		if (!array_init) return NULL;
		term->set_array_init(array_init);
	} break;
	default:
	{
		if (peek() == TokenType::DOT
			&& peek(1) != TokenType::BLOCK_START
			&& peek(2) != TokenType::BLOCK_START)
		{
			Ast_Enum* _enum = parse_enum();
			if (!_enum) return NULL;
			term->set_enum(_enum);
			break;
		}

		//@might be wrong with struct init: some::module::.{1, 2} this is invalid (maybe parse unresolved type?)
		option<Ast_Module_Access*> module_access = parse_module_access();
		
		if ((peek() == TokenType::DOT && peek(1) == TokenType::BLOCK_START) 
		|| (peek() == TokenType::IDENT && peek(1) == TokenType::DOT && peek(2) == TokenType::BLOCK_START))
		{
			Ast_Struct_Init* struct_init = parse_struct_init(module_access);
			if (!struct_init) return NULL;
			term->set_struct_init(struct_init);
			break;
		}

		Ast_Something* something = parse_something(module_access);
		if (!something) return NULL;
		term->set_something(something);
	} break;
	}

	return term;
}

Ast_Enum* Parser::parse_enum()
{
	Ast_Enum* _enum = arena_alloc<Ast_Enum>(&this->arena);

	if (!try_consume(TokenType::DOT)) { err_parse(TokenType::DOT, "enum literal"); return NULL; }

	option<Token> ident = try_consume(TokenType::IDENT);
	if (!ident) { err_parse(TokenType::IDENT, "enum literal"); return NULL; }
	_enum->unresolved.variant_ident = token_to_ident(ident.value());

	return _enum;
}

Ast_Cast* Parser::parse_cast()
{
	Ast_Cast* cast = arena_alloc<Ast_Cast>(&this->arena);
	consume();

	if (!try_consume(TokenType::PAREN_START)) { err_parse(TokenType::PAREN_START, "cast statement"); return NULL; }

	option<BasicType> basic_type = token_to_basic_type(peek());
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

Ast_Struct_Init* Parser::parse_struct_init(option<Ast_Module_Access*> module_access)
{
	Ast_Struct_Init* struct_init = arena_alloc<Ast_Struct_Init>(&this->arena);
	struct_init->unresolved.module_access = module_access;

	option<Token> token = try_consume(TokenType::IDENT);
	if (token) struct_init->unresolved.struct_ident = token_to_ident(token.value());
	if (!try_consume(TokenType::DOT)) { err_parse(TokenType::DOT, "struct initializer"); return NULL; }
	
	Ast_Expr_List* expr_list = parse_expr_list(TokenType::BLOCK_START, TokenType::BLOCK_END, "struct initializer");
	if (!expr_list) return NULL;
	struct_init->input = expr_list;

	return struct_init;
}

Ast_Array_Init* Parser::parse_array_init()
{
	Ast_Array_Init* array_init = arena_alloc<Ast_Array_Init>(&this->arena);

	if (peek() == TokenType::BRACKET_START)
	{
		array_init->type = parse_type();
		if (!array_init->type) return NULL;
	}

	Ast_Expr_List* expr_list = parse_expr_list(TokenType::BLOCK_START, TokenType::BLOCK_END, "array initializer");
	if (!expr_list) return NULL;
	array_init->input = expr_list;

	return array_init;
}

Ast_Something* Parser::parse_something(option<Ast_Module_Access*> module_access)
{
	Ast_Something* something = arena_alloc<Ast_Something>(&this->arena);
	something->module_access = module_access;
	
	Ast_Access* access = parse_access_first();
	if (!access) return NULL;
	something->access = access;

	bool result = parse_access(access);
	if (!result) return NULL;

	return something;
}

Ast_Access* Parser::parse_access_first()
{
	Ast_Access* access = arena_alloc<Ast_Access>(&this->arena);

	option<Token> token = try_consume(TokenType::IDENT);
	if (!token) { err_parse(TokenType::IDENT, "access chain"); return NULL; }
	Ast_Ident ident = token_to_ident(token.value());

	if (peek() == TokenType::PAREN_START)
	{
		Ast_Expr_List* expr_list = parse_expr_list(TokenType::PAREN_START, TokenType::PAREN_END, "procedure call");
		if (!expr_list) return NULL;
		access->set_call(ident, expr_list);
	}
	else access->set_ident(ident);

	return access;
}

bool Parser::parse_access(Ast_Access* prev)
{
	switch (peek())
	{
	case TokenType::DOT: break;
	case TokenType::BRACKET_START: break;
	default: return true;
	}

	Ast_Access* access = arena_alloc<Ast_Access>(&this->arena);

	switch (peek())
	{
	case TokenType::DOT:
	{
		consume();

		option<Token> token = try_consume(TokenType::IDENT);
		if (!token) { err_parse(TokenType::IDENT, "access chain"); return false; }
		Ast_Ident ident = token_to_ident(token.value());

		if (peek() == TokenType::PAREN_START)
		{
			Ast_Expr_List* expr_list = parse_expr_list(TokenType::PAREN_START, TokenType::PAREN_END, "procedure call");
			if (!expr_list) return NULL;
			access->set_call(ident, expr_list);
		}
		else access->set_ident(ident);
	} break;
	case TokenType::BRACKET_START:
	{
		consume();
		
		Ast_Expr* index_expr = parse_sub_expr();
		if (!index_expr) return false;
		access->set_array(index_expr);
		
		if (!try_consume(TokenType::BRACKET_END)) { err_parse(TokenType::BRACKET_END, "array access"); return false; }
	} break;
	default: { err_internal("parse_access: unexpected token type"); return false; }
	}

	prev->next = access;
	
	bool result = parse_access(access);
	if (!result) return false;
	
	return true;
}

Ast_Expr_List* Parser::parse_expr_list(TokenType start, TokenType end, const char* in)
{
	if (!try_consume(start)) { err_parse(start, in); return NULL; }
	Ast_Expr_List* expr_list = arena_alloc<Ast_Expr_List>(&this->arena);
	if (try_consume(end)) return expr_list;
	while (true)
	{
		Ast_Expr* expr = parse_sub_expr();
		if (!expr) return NULL;
		expr_list->exprs.emplace_back(expr);
		if (!try_consume(TokenType::COMMA)) break;
	}
	if (!try_consume(end)) { err_parse(end, in); return NULL; }
	return expr_list;
}

TokenType Parser::peek(u32 offset)
{
	return this->tokens[this->peek_index + offset].type;
}

Token Parser::peek_token(u32 offset)
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
	Token token = peek_token();
	consume();
	return token;
}

option<Token> Parser::try_consume(TokenType token_type)
{
	Token token = peek_token();
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
	err_report_parse(this->ast, expected, in, peek_token(offset));
}
