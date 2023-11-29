export module parser;

import general;
import ast;
import token;
import lexer;
import err_handler;
import <filesystem>;
namespace fs = std::filesystem;

#define span_start() u32 start = get_span_start()
#define span_end(node) node->span.start = start; node->span.end = get_span_end()
#define span_end_dot(node) node.span.start = start; node.span.end = get_span_end()

#define node_span_start(node) node->span.start = get_span_start()
#define node_span_end(node) node->span.end = get_span_end()
#define require_token(token_type) if (!try_consume(token_type)) { err_parse(token_type, context); return nullptr; }

#define parse_assign_or_return(variable, parse_call) \
{ \
	auto parse_node = parse_call; \
	if (parse_node == nullptr) return nullptr; \
	variable = parse_node; \
}

#define parse_assign_or_return_opt(variable, parse_call) \
{ \
	auto parse_node = parse_call; \
	if (parse_node == nullptr) return std::nullopt; \
	variable = parse_node; \
}

#define parse_or_return(node_name, parse_call) \
auto node_name = parse_call; \
if (!node_name) return nullptr;

Ast_Ident token_to_ident(Token token);
option<UnaryOp> token_to_unary_op(TokenType type);
option<BinaryOp> token_to_binary_op(TokenType type);
option<AssignOp> token_to_assign_op(TokenType type);
option<BasicType> token_to_basic_type(TokenType type);
u32 binary_op_prec(BinaryOp op);

export struct Parser
{
private:
	Ast_Module* curr_module;
	Arena arena;
	Lexer lexer;
	StringStorage strings;
	u32 peek_index = 0;
	Token prev_last;
	Token tokens[Lexer::TOKEN_BUFFER_SIZE];

public:
	Ast* parse_ast();

private:
	Ast_Module* parse_module(option<Ast_Module*> parent, fs::path& path);
	option<Ast_Type> parse_type();
	Ast_Type_Array* parse_type_array();
	Ast_Type_Procedure* parse_type_procedure();
	Ast_Type_Unresolved* parse_type_unresolved();

	Ast_Decl* parse_decl();
	Ast_Decl_Mod* parse_decl_mod();
	Ast_Decl_Proc* parse_decl_proc(bool in_impl);
	option<Ast_Proc_Param> parse_proc_param();
	Ast_Decl_Impl* parse_decl_impl();
	Ast_Decl_Enum* parse_decl_enum();
	option<Ast_Enum_Variant> parse_enum_variant();
	Ast_Decl_Struct* parse_decl_struct();
	option<Ast_Struct_Field> parse_struct_field();
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
	option<Ast_Switch_Case> parse_switch_case();
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
	Ast_Literal* parse_literal();
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
	option<Ast_Ident> try_consume_ident();
	option<UnaryOp> try_consume_unary_op();
	option<AssignOp> try_consume_assign_op();
	option<BasicType> try_consume_basic_type();
	u32 get_span_start();
	u32 get_span_end();
	void err_parse(TokenType expected, option<const char*> in, u32 offset = 0);
};

Ast_Ident token_to_ident(Token token)
{
	u32 hash = token.source_str.hash_fnv1a_32();
	return Ast_Ident{ token.span, token.source_str, hash };
}

option<UnaryOp> token_to_unary_op(TokenType type)
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

option<BinaryOp> token_to_binary_op(TokenType type)
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

option<AssignOp> token_to_assign_op(TokenType type)
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

option<BasicType> token_to_basic_type(TokenType type)
{
	switch (type)
	{
	case TokenType::KEYWORD_I8:     return BasicType::I8;
	case TokenType::KEYWORD_I16:    return BasicType::I16;
	case TokenType::KEYWORD_I32:    return BasicType::I32;
	case TokenType::KEYWORD_I64:    return BasicType::I64;
	case TokenType::KEYWORD_U8:     return BasicType::U8;
	case TokenType::KEYWORD_U16:    return BasicType::U16;
	case TokenType::KEYWORD_U32:    return BasicType::U32;
	case TokenType::KEYWORD_U64:    return BasicType::U64;
	case TokenType::KEYWORD_F32:    return BasicType::F32;
	case TokenType::KEYWORD_F64:    return BasicType::F64;
	case TokenType::KEYWORD_BOOL:   return BasicType::BOOL;
	case TokenType::KEYWORD_STRING: return BasicType::STRING;
	default: return {};
	}
}

u32 binary_op_prec(BinaryOp binary_op)
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
	default: return 0; //@add enum err
	}
}

struct Parse_Task
{
	fs::path path;
	option<Ast_Module*> parent;
};

Ast* Parser::parse_ast()
{
	//@require manifest to determine lib or bin package kind
	//@after look for   src/main - src/lib
	//@now only src/main is the target

	fs::path src = fs::path("src");
	if (!fs::exists(src)) { err_report(Error::PARSE_SRC_DIR_NOT_FOUND); return NULL; }
	
	this->strings.init();
	this->arena.init(4 * 1024 * 1024);
	Ast* ast = this->arena.alloc<Ast>();
	bool root_assigned = false;
	std::vector<Parse_Task> task_stack = {};
	task_stack.emplace_back(Parse_Task{ .path = src / "main.txt", .parent = {} }); //@replace ext

	while (!task_stack.empty())
	{
		Parse_Task task = task_stack.back();
		task_stack.pop_back();

		Ast_Module* module = parse_module(task.parent, task.path);
		if (!module) continue;
		if (!root_assigned)
		{
			ast->root = module;
			root_assigned = true;
		}

		for (Ast_Decl* decl : module->decls)
		{
			if (decl->tag() != Ast_Decl::Tag::Mod) continue;
			
			Ast_Decl_Mod* decl_mod = decl->as_mod;
			Ast_Ident name = decl_mod->ident;
			std::string module_name((char*)name.str.data, name.str.count);
			std::string module_name_ext = module_name;
			module_name_ext.append(".txt"); //@replace ext

			fs::path parent_path = fs::path(module->source->filepath).parent_path();
			fs::path path_same_level = fs::path(parent_path / module_name_ext);
			fs::path path_dir_level = fs::path(parent_path / module_name / module_name_ext);
			bool same_level_exists = fs::exists(path_same_level);
			bool dir_level_exists = fs::exists(path_dir_level);

			if (same_level_exists && dir_level_exists) //@create err report for this
			{
				err_internal("module with name exists on both levels, remove or rename one of them to make it not ambigiuos");
				err_context(module->source, name.span);
				printf("expected path: %s\n", (char*)path_same_level.u8string().c_str());
				printf("or path:       %s\n", (char*)path_dir_level.u8string().c_str());
				continue;
			}
			else if (!same_level_exists && !dir_level_exists) //@create err report for this
			{
				err_internal("module with name isnt found, expected module to have one of those paths:");
				err_context(module->source, name.span);
				printf("expected path: %s\n", (char*)path_same_level.u8string().c_str());
				printf("or path:       %s\n", (char*)path_dir_level.u8string().c_str());
				continue;
			}

			if (same_level_exists)
				task_stack.emplace_back(Parse_Task{ .path = path_same_level, .parent = module });
			else task_stack.emplace_back(Parse_Task{ .path = path_dir_level, .parent = module });
		}
	}

	if (!fs::exists("build") && !fs::create_directory("build")) 
	{ err_report(Error::OS_DIR_CREATE_FAILED); return NULL; } //@add context
	fs::current_path("build");

	if (err_get_status()) return NULL;
	return ast;
}

Ast_Module* Parser::parse_module(option<Ast_Module*> parent, fs::path& path)
{
	FILE* file;
	fopen_s(&file, (const char*)path.u8string().c_str(), "rb");
	if (!file) { err_report(Error::OS_FILE_OPEN_FAILED); printf("filepath: %s\n", (char*)path.u8string().c_str()); return NULL; } //@add context
	fseek(file, 0, SEEK_END);
	u64 size = (u64)ftell(file);
	fseek(file, 0, SEEK_SET);

	u8* data = this->arena.alloc_buffer<u8>(size);
	u64 read_size = fread(data, 1, size, file);
	fclose(file);
	if (read_size != size) { err_report(Error::OS_FILE_READ_FAILED); return NULL; } //@add context
	StringView str = StringView{ data, size };

	Ast_Source* source = this->arena.alloc<Ast_Source>();
	source->str = str;
	source->filepath = path.string();
	source->line_spans = {};
	
	Ast_Module* module = this->arena.alloc<Ast_Module>();
	module->source = source;
	
	std::string name = path.replace_extension("").filename().string();
	u8* copy = this->arena.alloc_buffer<u8>(name.size());
	for (u32 i = 0; i < name.size(); i += 1) copy[i] = name.at(i);
	StringView name_str = StringView{ .data = copy, .count = name.size() };
	module->name = Ast_Ident{ .str = name_str, .hash = name_str.hash_fnv1a_32() };

	this->curr_module = module;
	this->peek_index = 0;
	this->lexer.init(str, &this->strings, &module->source->line_spans);
	this->lexer.lex_token_buffer(this->tokens);

	while (peek() != TokenType::INPUT_END)
	{
		parse_or_return(decl, parse_decl());
		module->decls.emplace_back(decl);
	}
	
	module->parent = parent;
	if (parent) parent.value()->submodules.emplace_back(module);
	return module;
}

option<Ast_Type> Parser::parse_type()
{
	Ast_Type type = {};
	while (try_consume(TokenType::TIMES)) type.pointer_level += 1;
	
	option<BasicType> basic_type = try_consume_basic_type();
	if (basic_type) { type.set_basic(basic_type.value());  return type;  }

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
	const char* context = "array type signature";
	Ast_Type_Array* array_type = this->arena.alloc<Ast_Type_Array>();

	parse_assign_or_return(array_type->size_expr, parse_sub_expr());
	require_token(TokenType::BRACKET_END);
	parse_or_return(type, parse_type());
	array_type->element_type = type.value();

	return array_type;
}

Ast_Type_Procedure* Parser::parse_type_procedure()
{
	const char* context = "procedure type signature";
	Ast_Type_Procedure* procedure = this->arena.alloc<Ast_Type_Procedure>();

	if (!try_consume(TokenType::PAREN_END))
	{
		while (true)
		{
			//@allow mut without name + mut name : type @also store and fmt from ast
			if (peek() == TokenType::IDENT && peek(1) == TokenType::COLON) { consume(); consume(); } //@mut not specified in proc type
			parse_or_return(type, parse_type());
			procedure->input_types.emplace_back(type.value());
			if (!try_consume(TokenType::COMMA)) break;
		}
		require_token(TokenType::PAREN_END);
	}

	if (try_consume(TokenType::ARROW_THIN))
	{
		parse_or_return(type, parse_type());
		procedure->return_type = type.value();
	}

	return procedure;
}

Ast_Type_Unresolved* Parser::parse_type_unresolved()
{
	Ast_Type_Unresolved* unresolved = this->arena.alloc<Ast_Type_Unresolved>();
	unresolved->module_access = parse_module_access();

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "custom type signature"); return NULL; }
	unresolved->ident = ident.value();

	return unresolved;
}

Ast_Decl* Parser::parse_decl()
{
	Ast_Decl* decl = this->arena.alloc<Ast_Decl>();
	
	switch (peek())
	{
	case TokenType::IDENT:
	case TokenType::KEYWORD_PUB:
	{
		if (peek(1) == TokenType::KEYWORD_MOD)
		{
			parse_or_return(decl_mod, parse_decl_mod());
			decl->set_mod(decl_mod);
			break;
		}

		u32 offset = 0;
		if (peek() == TokenType::KEYWORD_PUB) offset = 1;
		if (peek(offset) != TokenType::IDENT)            { err_parse(TokenType::IDENT, "global declaration", 1); return NULL; }
		if (peek(offset + 1) != TokenType::DOUBLE_COLON) { err_parse(TokenType::DOUBLE_COLON, "global declaration", 1); return NULL; }

		switch (peek(offset + 2))
		{
		case TokenType::PAREN_START:    { parse_or_return(decl_proc, parse_decl_proc(false)); decl->set_proc(decl_proc); } break;
		case TokenType::KEYWORD_ENUM:   { parse_or_return(decl_enum, parse_decl_enum());      decl->set_enum(decl_enum); } break;
		case TokenType::KEYWORD_STRUCT: { parse_or_return(decl_struct, parse_decl_struct());  decl->set_struct(decl_struct); } break;
		default:                        { parse_or_return(decl_global, parse_decl_global());  decl->set_global(decl_global); } break;
		}
	} break;
	case TokenType::KEYWORD_MOD:    { parse_or_return(decl_mod, parse_decl_mod());       decl->set_mod(decl_mod); } break;
	case TokenType::KEYWORD_IMPL:   { parse_or_return(decl_impl, parse_decl_impl());     decl->set_impl(decl_impl); } break;
	case TokenType::KEYWORD_IMPORT: { parse_or_return(decl_import, parse_decl_import()); decl->set_import(decl_import);} break;
	default: { err_parse(TokenType::IDENT, "global declaration"); return NULL; } //@err set ident or impl or import
	}

	return decl;
}

Ast_Decl_Mod* Parser::parse_decl_mod()
{
	const char* context = "mod declaration";
	Ast_Decl_Mod* decl = this->arena.alloc<Ast_Decl_Mod>();

	if (try_consume(TokenType::KEYWORD_PUB)) decl->is_public = true;
	require_token(TokenType::KEYWORD_MOD);
	
	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, context); return NULL; }
	decl->ident = ident.value();

	require_token(TokenType::SEMICOLON);
	return decl;
}

Ast_Decl_Proc* Parser::parse_decl_proc(bool in_impl)
{
	Ast_Decl_Proc* decl = this->arena.alloc<Ast_Decl_Proc>();
	decl->is_member = in_impl;
	
	if (try_consume(TokenType::KEYWORD_PUB)) decl->is_public = true;

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "proc declaration"); return NULL; }
	decl->ident = ident.value();
	
	if (!try_consume(TokenType::DOUBLE_COLON)) { err_parse(TokenType::DOUBLE_COLON, "procedure declaration"); return NULL; }
	if (!try_consume(TokenType::PAREN_START))  { err_parse(TokenType::PAREN_START, "procedure declaration"); return NULL; }

	if (!try_consume(TokenType::PAREN_END))
	{
		while (true)
		{
			if (try_consume(TokenType::DOUBLE_DOT)) { decl->is_variadic = true; break; }
			option<Ast_Proc_Param> param = parse_proc_param();
			if (!param) return NULL;
			decl->input_params.emplace_back(param.value());
			if (!try_consume(TokenType::COMMA)) break;
		}
		if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "procedure declaration"); return NULL; }
	}

	if (try_consume(TokenType::ARROW_THIN))
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

option<Ast_Proc_Param> Parser::parse_proc_param()
{
	Ast_Proc_Param param = {};

	if (try_consume(TokenType::KEYWORD_MUT)) param.is_mutable = true;

	if (peek() == TokenType::KEYWORD_SELF)
	{
		param.self = true;
		param.ident = token_to_ident(consume_get());
	}
	else
	{
		option<Ast_Ident> ident = try_consume_ident();
		if (!ident) { err_parse(TokenType::IDENT, "procedure parameter"); return {}; };
		param.ident = ident.value();

		if (!try_consume(TokenType::COLON)) { err_parse(TokenType::COLON, "procedure parameter"); return {}; }

		option<Ast_Type> type = parse_type();
		if (!type) return {};
		param.type = type.value();
	}

	return param;
}

Ast_Decl_Impl* Parser::parse_decl_impl()
{
	if (!try_consume(TokenType::KEYWORD_IMPL)) { err_parse(TokenType::KEYWORD_IMPL, "impl block"); return NULL; }
	Ast_Decl_Impl* impl_decl = this->arena.alloc<Ast_Decl_Impl>();

	//@maybe parse type unresolved (import + ident) instead of full type
	//to limit the use of basic types or arrays, etc
	option<Ast_Type> type = parse_type();
	if (!type) return NULL;
	impl_decl->type = type.value();

	//if (!try_consume(TokenType::DOUBLE_COLON)) { err_parse(TokenType::DOUBLE_COLON, "impl block"); return NULL; }
	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "impl block"); return NULL; }

	while (!try_consume(TokenType::BLOCK_END))
	{
		//@this is checked externally instead of parse_proc_decl checking this
		//@same pattern inside file level decl parsing, might change in the future
		if (peek() != TokenType::IDENT) { err_parse(TokenType::IDENT, "procedure declaration inside impl block"); return NULL; }
		if (peek(1) != TokenType::DOUBLE_COLON) { err_parse(TokenType::DOUBLE_COLON, "procedure declaration inside impl block"); return NULL; }
		if (peek(2) != TokenType::PAREN_START) { err_parse(TokenType::PAREN_START, "procedure declaration inside impl block"); return NULL; }

		Ast_Decl_Proc* member_procedure = parse_decl_proc(true);
		if (!member_procedure) return NULL;
		impl_decl->member_procedures.emplace_back(member_procedure);
	}

	return impl_decl;
}

Ast_Decl_Enum* Parser::parse_decl_enum()
{
	Ast_Decl_Enum* decl = this->arena.alloc<Ast_Decl_Enum>();

	if (try_consume(TokenType::KEYWORD_PUB)) decl->is_public = true;

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "enum declaration"); return NULL; }
	decl->ident = ident.value();

	if (!try_consume(TokenType::DOUBLE_COLON)) { err_parse(TokenType::DOUBLE_COLON, "enum declaration"); return NULL; }
	if (!try_consume(TokenType::KEYWORD_ENUM)) { err_parse(TokenType::KEYWORD_ENUM, "enum declaration"); return NULL; }

	option<BasicType> basic_type = try_consume_basic_type();
	if (basic_type) decl->basic_type = basic_type.value();

	if (!try_consume(TokenType::BLOCK_START)) { err_parse(TokenType::BLOCK_START, "enum declaration"); return NULL; }
	while (!try_consume(TokenType::BLOCK_END))
	{
		option<Ast_Enum_Variant> variant = parse_enum_variant();
		if (!variant) return NULL;
		decl->variants.emplace_back(variant.value());
	}

	return decl;
}

option<Ast_Enum_Variant> Parser::parse_enum_variant()
{
	Ast_Enum_Variant variant = {};

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "enum variant"); return {}; }
	variant.ident = ident.value();

	if (!try_consume(TokenType::ASSIGN)) { err_parse(TokenType::ASSIGN, "enum variant"); return {}; }

	Ast_Expr* expr = parse_expr();
	if (!expr) return {};
	variant.consteval_expr = parse_consteval_expr(expr);

	return variant;
}

Ast_Decl_Struct* Parser::parse_decl_struct()
{
	Ast_Decl_Struct* decl = this->arena.alloc<Ast_Decl_Struct>();

	if (try_consume(TokenType::KEYWORD_PUB)) decl->is_public = true;
	
	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "struct declaration"); return NULL; }
	decl->ident = ident.value();

	if (!try_consume(TokenType::DOUBLE_COLON))   { err_parse(TokenType::DOUBLE_COLON, "struct declaration"); return NULL; }
	if (!try_consume(TokenType::KEYWORD_STRUCT)) { err_parse(TokenType::KEYWORD_STRUCT, "struct declaration"); return NULL; }
	if (!try_consume(TokenType::BLOCK_START))    { err_parse(TokenType::BLOCK_START, "struct declaration"); return NULL; }
	
	while (!try_consume(TokenType::BLOCK_END))
	{
		option<Ast_Struct_Field> field = parse_struct_field();
		if (!field) return NULL;
		decl->fields.emplace_back(field.value());
	}

	return decl;
}

option<Ast_Struct_Field> Parser::parse_struct_field()
{
	Ast_Struct_Field field = {};

	if (try_consume(TokenType::KEYWORD_PUB)) field.is_public = true;

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "struct field"); return {}; }
	field.ident = ident.value();

	if (!try_consume(TokenType::COLON)) { err_parse(TokenType::COLON, "struct field"); return {}; }

	option<Ast_Type> type = parse_type();
	if (!type) return {};
	field.type = type.value();

	if (try_consume(TokenType::ASSIGN))
	{
		Ast_Expr* expr = parse_expr();
		if (!expr) return {};
		field.default_expr = expr;
	}
	else if (!try_consume(TokenType::SEMICOLON)) { err_parse(TokenType::SEMICOLON, "struct field"); return {}; }

	return field;
}

Ast_Decl_Global* Parser::parse_decl_global()
{
	Ast_Decl_Global* decl = this->arena.alloc<Ast_Decl_Global>();

	if (try_consume(TokenType::KEYWORD_PUB)) decl->is_public = true;

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "global declaration"); return NULL; }
	decl->ident = ident.value();

	if (!try_consume(TokenType::DOUBLE_COLON)) { err_parse(TokenType::DOUBLE_COLON, "global declaration"); return NULL; }

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	decl->consteval_expr = parse_consteval_expr(expr);

	return decl;
}

Ast_Decl_Import* Parser::parse_decl_import()
{
	Ast_Decl_Import* decl = this->arena.alloc<Ast_Decl_Import>();
	
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
	Ast_Import_Target* target = this->arena.alloc<Ast_Import_Target>();

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
				option<Ast_Ident> ident = try_consume_ident();
				if (!ident) return NULL;
				target->as_symbol_list.symbols.emplace_back(ident.value());
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

	Ast_Module_Access* module_access = this->arena.alloc<Ast_Module_Access>();
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
	Ast_Stmt* stmt = this->arena.alloc<Ast_Stmt>();

	switch (peek())
	{
	case TokenType::KEYWORD_IF:       { parse_or_return(_if, parse_stmt_if()); stmt->set_if(_if); } break;
	case TokenType::KEYWORD_FOR:      { parse_or_return(_for, parse_stmt_for()); stmt->set_for(_for); } break;
	case TokenType::BLOCK_START:      { parse_or_return(block, parse_stmt_block()); stmt->set_block(block); } break;
	case TokenType::KEYWORD_DEFER:    { parse_or_return(defer, parse_stmt_defer()); stmt->set_defer(defer); } break;
	case TokenType::KEYWORD_BREAK:    { parse_or_return(_break, parse_stmt_break()); stmt->set_break(_break); } break;
	case TokenType::KEYWORD_RETURN:   { parse_or_return(_return, parse_stmt_return()); stmt->set_return(_return); } break;
	case TokenType::KEYWORD_SWITCH:   { parse_or_return(_switch, parse_stmt_switch()); stmt->set_switch(_switch); } break;
	case TokenType::KEYWORD_CONTINUE: { parse_or_return(_continue, parse_stmt_continue()); stmt->set_continue(_continue); } break;
	default:
	{
		if (peek() == TokenType::KEYWORD_MUT || (peek() == TokenType::IDENT && peek(1) == TokenType::COLON))
		{
			parse_or_return(var_decl, parse_stmt_var_decl());
			stmt->set_var_decl(var_decl);
			break;
		}

		span_start();
		parse_or_return(something, parse_something(parse_module_access()));

		if (try_consume(TokenType::SEMICOLON))
		{
			Ast_Stmt_Proc_Call* proc_call = this->arena.alloc<Ast_Stmt_Proc_Call>();
			proc_call->something = something;
			stmt->set_proc_call(proc_call);
			break;
		}
		else
		{
			Ast_Stmt_Var_Assign* var_assign = this->arena.alloc<Ast_Stmt_Var_Assign>();
			var_assign->something = something;

			option<AssignOp> assign_op = try_consume_assign_op();
			if (!assign_op) { err_parse(TokenType::ASSIGN, "variable assignment statement"); return NULL; } //@Err set of assignment operators
			var_assign->op = assign_op.value();

			parse_assign_or_return(var_assign->expr, parse_expr());
			stmt->set_var_assign(var_assign);
			span_end(var_assign);
		}
	} break;
	}
	
	return stmt;
}

Ast_Stmt_If* Parser::parse_stmt_if()
{
	const char* context = "if statement";
	Ast_Stmt_If* _if = this->arena.alloc<Ast_Stmt_If>();
	node_span_start(_if);

	require_token(TokenType::KEYWORD_IF);
	parse_assign_or_return(_if->condition_expr, parse_sub_expr());
	parse_assign_or_return(_if->block, parse_stmt_block());
	if (peek() == TokenType::KEYWORD_ELSE) parse_assign_or_return(_if->_else, parse_else());

	node_span_end(_if);
	return _if;
}

Ast_Else* Parser::parse_else()
{
	const char* context = "else statement";
	Ast_Else* _else = this->arena.alloc<Ast_Else>();
	node_span_start(_else);

	require_token(TokenType::KEYWORD_ELSE);

	switch (peek())
	{
	case TokenType::KEYWORD_IF:  { parse_or_return(_if, parse_stmt_if()); _else->set_if(_if); } break;
	case TokenType::BLOCK_START: { parse_or_return(block, parse_stmt_block()); _else->set_block(block); } break;
	default: { err_parse(TokenType::KEYWORD_IF, context); return NULL; } //@Err set Expected 'if' or code block '{ ... }
	}

	node_span_end(_else);
	return _else;
}

Ast_Stmt_For* Parser::parse_stmt_for()
{
	const char* context = "for statement";
	Ast_Stmt_For* _for = this->arena.alloc<Ast_Stmt_For>();
	node_span_start(_for);
	
	require_token(TokenType::KEYWORD_FOR);
	
	if (peek() == TokenType::BLOCK_START)
	{
		parse_assign_or_return(_for->block, parse_stmt_block());
		node_span_end(_for);
		return _for;
	}
	if (peek() == TokenType::KEYWORD_MUT || (peek() == TokenType::IDENT && peek(1) == TokenType::COLON))
	{
		parse_assign_or_return(_for->var_decl, parse_stmt_var_decl());
	}

	parse_assign_or_return(_for->condition_expr, parse_expr()); //@using full expr with ; temp

	//@Separate var assign parsing properly to work across for stmt and regular stmt
	//@hardcoded var assign parsing same as in parse_stmt
	u32 start_2 = get_span_start();

	parse_or_return(something, parse_something(parse_module_access()));
	Ast_Stmt_Var_Assign* var_assign = this->arena.alloc<Ast_Stmt_Var_Assign>();
	var_assign->something = something;

	option<AssignOp> assign_op = try_consume_assign_op();
	if (!assign_op) { err_parse(TokenType::ASSIGN, "variable assignment statement"); return NULL; } //@Err set of assignment operators
	var_assign->op = assign_op.value();

	parse_assign_or_return(var_assign->expr, parse_expr());

	var_assign->span.start = start_2;
	var_assign->span.end = get_span_end();
	_for->var_assign = var_assign;
	//@end

	parse_assign_or_return(_for->block, parse_stmt_block());

	node_span_end(_for);
	return _for;
}

Ast_Stmt_Block* Parser::parse_stmt_block()
{
	const char* context = "code block";
	Ast_Stmt_Block* block = this->arena.alloc<Ast_Stmt_Block>();

	require_token(TokenType::BLOCK_START);
	while (!try_consume(TokenType::BLOCK_END))
	{
		parse_or_return(statement, parse_stmt());
		block->statements.emplace_back(statement);
	}
	return block;
}

Ast_Stmt_Block* Parser::parse_stmt_block_short()
{
	if (peek() == TokenType::BLOCK_START) return parse_stmt_block();

	Ast_Stmt_Block* block = this->arena.alloc<Ast_Stmt_Block>();
	parse_or_return(statement, parse_stmt());
	block->statements.emplace_back(statement);
	return block;
}

Ast_Stmt_Defer* Parser::parse_stmt_defer()
{
	const char* context = "defer statement";
	Ast_Stmt_Defer* defer = this->arena.alloc<Ast_Stmt_Defer>();
	node_span_start(defer);
	
	require_token(TokenType::KEYWORD_DEFER);
	parse_assign_or_return(defer->block, parse_stmt_block_short());

	node_span_end(defer);
	return defer;
}

Ast_Stmt_Break* Parser::parse_stmt_break()
{
	const char* context = "break statement";
	Ast_Stmt_Break* _break = this->arena.alloc<Ast_Stmt_Break>();
	node_span_start(_break);

	require_token(TokenType::KEYWORD_BREAK);
	require_token(TokenType::SEMICOLON);
	
	node_span_end(_break);
	return _break;
}

Ast_Stmt_Return* Parser::parse_stmt_return()
{
	const char* context = "return statement";
	Ast_Stmt_Return* _return = this->arena.alloc<Ast_Stmt_Return>();
	node_span_start(_return);
	
	require_token(TokenType::KEYWORD_RETURN);
	if (!try_consume(TokenType::SEMICOLON)) parse_assign_or_return(_return->expr, parse_expr());
	
	node_span_end(_return);
	return _return;
}

Ast_Stmt_Switch* Parser::parse_stmt_switch()
{
	const char* context = "switch statement";
	Ast_Stmt_Switch* _switch = this->arena.alloc<Ast_Stmt_Switch>();
	node_span_start(_switch);

	require_token(TokenType::KEYWORD_SWITCH);
	parse_assign_or_return(_switch->expr, parse_sub_expr());
	require_token(TokenType::BLOCK_START);
	while (!try_consume(TokenType::BLOCK_END))
	{
		option<Ast_Switch_Case> switch_case = parse_switch_case();
		if (!switch_case) return {};
		_switch->cases.emplace_back(switch_case.value());
	}

	node_span_end(_switch);
	return _switch;
}

option<Ast_Switch_Case> Parser::parse_switch_case()
{
	Ast_Switch_Case switch_case = {};

	parse_assign_or_return_opt(switch_case.case_expr, parse_sub_expr());
	if (!try_consume(TokenType::COLON)) parse_assign_or_return_opt(switch_case.block, parse_stmt_block_short());

	return switch_case;
}

Ast_Stmt_Continue* Parser::parse_stmt_continue()
{
	const char* context = "continue statement";
	Ast_Stmt_Continue* _continue = this->arena.alloc<Ast_Stmt_Continue>();
	node_span_start(_continue);

	require_token(TokenType::KEYWORD_CONTINUE);
	require_token(TokenType::SEMICOLON);
	
	node_span_end(_continue);
	return _continue;
}

Ast_Stmt_Var_Decl* Parser::parse_stmt_var_decl()
{
	const char* context = "variable declaration statement";
	Ast_Stmt_Var_Decl* var_decl = this->arena.alloc<Ast_Stmt_Var_Decl>();
	node_span_start(var_decl);

	if (try_consume(TokenType::KEYWORD_MUT)) var_decl->is_mutable = true;

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "variable declaration"); return NULL; }
	var_decl->ident = ident.value();

	if (!try_consume(TokenType::COLON)) { err_parse(TokenType::COLON, "variable declaration"); return NULL; }

	bool infer_type = try_consume(TokenType::ASSIGN).has_value();
	if (!infer_type)
	{
		option<Ast_Type> type = parse_type();
		if (!type) return NULL;
		var_decl->type = type.value();

		if (try_consume(TokenType::SEMICOLON)) 
		{
			node_span_end(var_decl);
			return var_decl;
		}
		if (!try_consume(TokenType::ASSIGN)) { err_parse(TokenType::ASSIGN, "var decl statement"); return NULL; } //@Err Expected '=' or ';' in a variable declaration
	}

	Ast_Expr* expr = parse_expr();
	if (!expr) return NULL;
	var_decl->expr = expr;

	node_span_end(var_decl);
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
		option<BinaryOp> binary_op = token_to_binary_op(peek());
		if (!binary_op) break;
		u32 prec = binary_op_prec(binary_op.value());
		if (prec < min_prec) break;
		consume();

		Ast_Expr* expr_rhs = parse_sub_expr(prec + 1);
		if (expr_rhs == NULL) return NULL;

		Ast_Expr* expr_lhs_copy = this->arena.alloc<Ast_Expr>();
		expr_lhs_copy->ptr_tag_copy(expr_lhs);

		Ast_Binary_Expr* bin_expr = this->arena.alloc<Ast_Binary_Expr>();
		bin_expr->op = binary_op.value();
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

	option<UnaryOp> unary_op = try_consume_unary_op();
	if (unary_op)
	{
		Ast_Expr* right_expr = parse_primary_expr();
		if (!right_expr) return NULL;

		Ast_Unary_Expr* unary_expr = this->arena.alloc<Ast_Unary_Expr>();
		unary_expr->op = unary_op.value();
		unary_expr->right = right_expr;

		Ast_Expr* expr = this->arena.alloc<Ast_Expr>();
		expr->set_unary(unary_expr);
		return expr;
	}

	Ast_Term* term = parse_term();
	if (!term) return NULL;
	
	Ast_Expr* expr = this->arena.alloc<Ast_Expr>();
	expr->set_term(term);
	return expr;
}

Ast_Consteval_Expr* Parser::parse_consteval_expr(Ast_Expr* expr)
{
	Ast_Consteval_Expr* consteval_expr = this->arena.alloc<Ast_Consteval_Expr>();
	consteval_expr->eval = Consteval::Not_Evaluated;
	consteval_expr->expr = expr;
	expr->flags |= AST_EXPR_FLAG_CONST_BIT;
	return consteval_expr;
}

Ast_Term* Parser::parse_term()
{
	Ast_Term* term = this->arena.alloc<Ast_Term>();
	
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
	case TokenType::LITERAL_INT:
	case TokenType::LITERAL_FLOAT:
	case TokenType::LITERAL_BOOL:
	case TokenType::LITERAL_STRING:
	{
		Ast_Literal* literal = parse_literal();
		if (!literal) return NULL;
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
	Ast_Enum* _enum = this->arena.alloc<Ast_Enum>();

	if (!try_consume(TokenType::DOT)) { err_parse(TokenType::DOT, "enum literal"); return NULL; }

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "enum literal"); return NULL; }
	_enum->unresolved.variant_ident = ident.value();

	return _enum;
}

Ast_Cast* Parser::parse_cast()
{
	Ast_Cast* cast = this->arena.alloc<Ast_Cast>();
	if (!try_consume(TokenType::KEYWORD_CAST)) { err_parse(TokenType::KEYWORD_CAST, "cast statement"); return NULL; }

	if (!try_consume(TokenType::PAREN_START)) { err_parse(TokenType::PAREN_START, "cast statement"); return NULL; }

	option<BasicType> basic_type = try_consume_basic_type();
	if (!basic_type) { err_parse(TokenType::KEYWORD_I8, "cast statement"); return NULL; } //@Error basic type set
	cast->basic_type = basic_type.value();

	if (!try_consume(TokenType::COMMA)) { err_parse(TokenType::COMMA, "cast statement"); return NULL; }

	Ast_Expr* expr = parse_sub_expr();
	if (!expr) return NULL;
	cast->expr = expr;

	if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "cast statement"); return NULL; }

	return cast;
}

Ast_Sizeof* Parser::parse_sizeof()
{
	Ast_Sizeof* _sizeof = this->arena.alloc<Ast_Sizeof>();
	consume();

	if (!try_consume(TokenType::PAREN_START)) { err_parse(TokenType::PAREN_START, "sizeof statement"); return NULL; }
	
	option<Ast_Type> type = parse_type();
	if (!type) return NULL;
	_sizeof->type = type.value();

	if (!try_consume(TokenType::PAREN_END)) { err_parse(TokenType::PAREN_END, "sizeof statement"); return NULL; }

	return _sizeof;
}

Ast_Literal* Parser::parse_literal()
{
	Ast_Literal* literal = this->arena.alloc<Ast_Literal>();
	
	Token token = peek_token();
	switch (token.type)
	{
	case TokenType::LITERAL_INT:    literal->set_u64(token.literal_u64); break;
	case TokenType::LITERAL_FLOAT:  literal->set_f64(token.literal_f64); break;
	case TokenType::LITERAL_BOOL:   literal->set_bool(token.literal_bool); break;
	case TokenType::LITERAL_STRING: literal->set_string(token.literal_string); break;
	default: { err_parse(TokenType::LITERAL_INT, "literal"); return NULL; } //@err token set
	}
	consume();
	
	literal->span = token.span;
	return literal;
}

Ast_Struct_Init* Parser::parse_struct_init(option<Ast_Module_Access*> module_access)
{
	Ast_Struct_Init* struct_init = this->arena.alloc<Ast_Struct_Init>();
	struct_init->unresolved.module_access = module_access;

	option<Ast_Ident> ident = try_consume_ident();
	if (ident) struct_init->unresolved.struct_ident = ident.value();
	if (!try_consume(TokenType::DOT)) { err_parse(TokenType::DOT, "struct initializer"); return NULL; }
	
	Ast_Expr_List* expr_list = parse_expr_list(TokenType::BLOCK_START, TokenType::BLOCK_END, "struct initializer");
	if (!expr_list) return NULL;
	struct_init->input = expr_list;

	return struct_init;
}

Ast_Array_Init* Parser::parse_array_init()
{
	Ast_Array_Init* array_init = this->arena.alloc<Ast_Array_Init>();

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
	Ast_Something* something = this->arena.alloc<Ast_Something>();
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
	Ast_Access* access = this->arena.alloc<Ast_Access>();

	option<Ast_Ident> ident = try_consume_ident();
	if (!ident) { err_parse(TokenType::IDENT, "access chain"); return NULL; }

	if (peek() == TokenType::PAREN_START)
	{
		Ast_Expr_List* expr_list = parse_expr_list(TokenType::PAREN_START, TokenType::PAREN_END, "procedure call");
		if (!expr_list) return NULL;
		access->set_call(ident.value(), expr_list);
	}
	else access->set_ident(ident.value());

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

	Ast_Access* access = this->arena.alloc<Ast_Access>();

	switch (peek())
	{
	case TokenType::DOT:
	{
		consume();

		option<Ast_Ident> ident = try_consume_ident();
		if (!ident) { err_parse(TokenType::IDENT, "access chain"); return false; }

		if (peek() == TokenType::PAREN_START)
		{
			Ast_Expr_List* expr_list = parse_expr_list(TokenType::PAREN_START, TokenType::PAREN_END, "procedure call");
			if (!expr_list) return NULL;
			access->set_call(ident.value(), expr_list);
		}
		else access->set_ident(ident.value());
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
	Ast_Expr_List* expr_list = this->arena.alloc<Ast_Expr_List>();
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

option<Ast_Ident> Parser::try_consume_ident()
{
	option<Token> token = try_consume(TokenType::IDENT);
	if (token) return token_to_ident(token.value());
	return {};
}

option<UnaryOp> Parser::try_consume_unary_op()
{
	option<UnaryOp> unary_op = token_to_unary_op(peek());
	if (unary_op) consume();
	return unary_op;
}

option<AssignOp> Parser::try_consume_assign_op()
{
	option<AssignOp> assign_op = token_to_assign_op(peek());
	if (assign_op) consume();
	return assign_op;
}

option<BasicType> Parser::try_consume_basic_type()
{
	option<BasicType> basic_type = token_to_basic_type(peek());
	if (basic_type) consume();
	return basic_type;
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
	err_report_parse(this->curr_module->source, expected, in, peek_token(offset));
}
