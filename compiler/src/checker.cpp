#include "checker.h"

#include "debug_printer.h"

//@Design currently checking is split into 3 stages
// 1. import paths & decl uniqueness checks
// 2. decl signature validity checks
// 3. proc block cfg & type and other semantics checks 

//@Perf general issues with re-hashing Ast_Ident every time
bool check_decl_uniqueness(Ast* ast, Ast_Program* program, Module_Map& modules)
{
	bool passed = true;
	
	HashSet<Ast_Ident, u32, match_ident> symbol_table(256);
	ast->import_table.init(64);
	ast->struct_table.init(64);
	ast->enum_table.init(64);
	ast->proc_table.init(64);

	for (Ast_Import_Decl* decl : ast->imports)
	{
		Ast_Ident ident = decl->alias;
		auto key = symbol_table.find_key(ident, hash_ident(ident));
		if (!key) { symbol_table.add(ident, hash_ident(ident)); ast->import_table.add(ident, decl, hash_ident(ident)); }
		else { error_pair("Symbol already declared", "Import", ident, "Symbol", key.value()); passed = false; }
	}

	for (Ast_Import_Decl* decl : ast->imports)
	{
		if (modules.find(decl->file_path.token.string_literal_value) != modules.end())
		{
			Ast* import_ast = modules.at(decl->file_path.token.string_literal_value);
			decl->import_ast = import_ast;
		}
		else
		{
			error("Import path not found", decl->alias);
			passed = false;
		}
	}

	for (Ast_Use_Decl* decl : ast->uses)
	{
		Ast_Ident ident = decl->alias;
		auto key = symbol_table.find_key(ident, hash_ident(ident));
		if (!key) symbol_table.add(ident, hash_ident(ident));
		else { error_pair("Symbol already declared", "Use", ident, "Symbol", key.value()); passed = false; }
	}

	u64 struct_count = 0;
	for (Ast_Struct_Decl* decl : ast->structs)
	{
		Ast_Ident ident = decl->type;
		auto key = symbol_table.find_key(ident, hash_ident(ident));
		if (!key) 
		{
			symbol_table.add(ident, hash_ident(ident)); 
			ast->struct_table.add(ident, Ast_Struct_Decl_Meta { ast->struct_id_start + struct_count, decl }, hash_ident(ident));
			
			Ast_Struct_Meta struct_meta = {};
			struct_meta.struct_decl = decl;
			program->structs.emplace_back(struct_meta);
		}
		else { error_pair("Symbol already declared", "Struct", ident, "Symbol", key.value()); passed = false; }
		struct_count += 1;
	}

	u64 enum_count = 0;
	for (Ast_Enum_Decl* decl : ast->enums)
	{
		Ast_Ident ident = decl->type;
		auto key = symbol_table.find_key(ident, hash_ident(ident));
		if (!key) 
		{ 
			symbol_table.add(ident, hash_ident(ident));
			ast->enum_table.add(ident, Ast_Enum_Decl_Meta { ast->enum_id_start + enum_count, decl }, hash_ident(ident));

			Ast_Enum_Meta enum_meta = {};
			enum_meta.enum_decl = decl;
			program->enums.emplace_back(enum_meta);
		}
		else { error_pair("Symbol already declared", "Enum", ident, "Symbol", key.value()); passed = false; }
		enum_count += 1;
	}

	u64 proc_count = 0;
	for (Ast_Proc_Decl* decl : ast->procs)
	{
		Ast_Ident ident = decl->ident;
		auto key = symbol_table.find_key(ident, hash_ident(ident));
		if (!key) 
		{ 
			symbol_table.add(ident, hash_ident(ident));
			ast->proc_table.add(ident, Ast_Proc_Decl_Meta { ast->proc_id_start + proc_count, decl }, hash_ident(ident));
			
			Ast_Proc_Meta proc_meta = {};
			proc_meta.proc_decl = decl;
			program->procedures.emplace_back(proc_meta);
		}
		else { error_pair("Symbol already declared", "Procedure", ident, "Symbol", key.value()); passed = false; }
		proc_count += 1;
	}

	//@Low priority
	//@Rule todo: cant import same thing under multiple names
	//@Rule todo: cant import same type or procedure under multiple names
	
	return passed;
}

bool check_decls(Ast* ast)
{
	bool passed = true;

	// Find and add use symbols to current scope
	for (Ast_Use_Decl* use_decl : ast->uses)
	{
		Ast* import_ast = try_import(ast, { use_decl->import });
		if (import_ast == NULL)
		{
			passed = false;
			continue;
		}

		Ast_Ident alias = use_decl->alias;
		Ast_Ident symbol = use_decl->symbol;
		auto struct_decl = import_ast->struct_table.find(symbol, hash_ident(symbol));
		if (struct_decl) { ast->struct_table.add(alias, struct_decl.value(), hash_ident(alias)); continue; }
		auto enum_decl = import_ast->enum_table.find(symbol, hash_ident(symbol));
		if (enum_decl) { ast->enum_table.add(alias, enum_decl.value(), hash_ident(alias)); continue; }
		auto proc_decl = import_ast->proc_table.find(symbol, hash_ident(symbol));
		if (proc_decl) { ast->proc_table.add(alias, proc_decl.value(), hash_ident(alias)); continue; }

		error("Use symbol isnt found in imported namespace", symbol);
		passed = false;
	}

	for (Ast_Struct_Decl* struct_decl : ast->structs)
	{
		if (!check_struct_decl(ast, struct_decl)) passed = false;
	}

	for (Ast_Enum_Decl* enum_decl : ast->enums)
	{
		if (!check_enum_decl(enum_decl)) passed = false;
	}

	for (Ast_Proc_Decl* proc_decl : ast->procs)
	{
		if (!check_proc_decl(ast, proc_decl)) passed = false;
	}

	return passed;
}

bool check_main_proc(Ast* ast)
{
	Ast_Ident ident = {};
	char name_arr[4] = { 'm', 'a', 'i', 'n' };
	ident.str.data = (u8*)name_arr;
	ident.str.count = 4;

	auto proc_meta = find_proc(ast, ident);
	if (!proc_meta)
	{
		printf("Main procedure not found. Make sure src/main file has 'main :: () :: i32 { ... }' declared\n\n");
		return false;
	}

	Ast_Proc_Decl_Meta meta = proc_meta.value();
	meta.proc_decl->is_main = true;
	bool passed = true;
	
	if (meta.proc_decl->is_external)
	{
		error("Main procedure cannot be specified as external '@'. You must define a main procedure block", meta.proc_decl->ident);
		passed = false;
	}

	if (meta.proc_decl->input_params.size() != 0)
	{
		error("Main procedure cannot have any input parameters. Command line arguments can be accessed using core library", meta.proc_decl->ident);
		passed = false;
	}

	if (!meta.proc_decl->return_type)
	{
		error("Main procedure must have i32 return type", meta.proc_decl->ident);
		passed = false;
	}
	else
	{
		Ast_Type return_type = meta.proc_decl->return_type.value();
		if (return_type.tag != Ast_Type::Tag::Basic || return_type.as_basic != BASIC_TYPE_I32)
		{
			printf("Main procedure must have i32 return type\n");
			debug_print_ident(meta.proc_decl->ident);
			printf("\n");
			passed = false;
		}
	}

	return passed;
}

bool check_ast(Ast* ast)
{
	bool passed = true;
	
	Block_Stack bc = {};
	//@Notice not setting passed flag in checks
	for (Ast_Proc_Decl* proc_decl : ast->procs)
	{
		if (proc_decl->is_external) continue;

		//@Notice this doesnt correctly handle if else on top level, which may allow all paths to return
		Terminator terminator = check_block_cfg(proc_decl->block, false, false);
		if (proc_decl->return_type.has_value() && terminator != Terminator::Return)
		error("Not all control flow paths return value", proc_decl->ident);
		
		//@Notice need to add input variables to block stack
		block_stack_reset(&bc);
		block_stack_add_block(&bc);
		for (Ast_Ident_Type_Pair& param : proc_decl->input_params)
		{
			//@Notice input params are checked at declaration check stage
			Type_Info type_info = Type_Info { true, param.type };
			
			if (block_stack_contains_var(&bc, param.ident))
			{
				error("Input parameter with same name is already exists", param.ident);
			}
			else 
			{
				block_stack_add_var(&bc, param.ident, type_info);
			}
		}
		check_block(ast, &bc, proc_decl->block, false);
	}

	return passed;
}

bool check_struct_decl(Ast* ast, Ast_Struct_Decl* struct_decl)
{
	bool passed = true;
	HashSet<Ast_Ident, u32, match_ident> name_set(16); //@Perf possibly move out from function and re-use hash_set with reset()

	for (Ast_Ident_Type_Pair& field : struct_decl->fields)
	{
		auto name = name_set.find_key(field.ident, hash_ident(field.ident));
		if (!name) name_set.add(field.ident, hash_ident(field.ident));
		else { error("Duplicate struct field identifier", field.ident); passed = false; }

		if (!check_type(ast, &field.type)) passed = false;
	}

	return passed;
}

bool check_enum_decl(Ast_Enum_Decl* enum_decl)
{
	bool passed = true;
	HashSet<Ast_Ident, u32, match_ident> name_set(16); //@Perf read check_struct_decl comment

	BasicType basic_type = BASIC_TYPE_I32;
	if (enum_decl->basic_type) basic_type = enum_decl->basic_type.value();
	bool is_unsigned = basic_type_is_unsigned(basic_type);

	//@Note this might be usefull, look into supporting string enums later
	if (basic_type == BASIC_TYPE_STRING)
	{
		error("Cannot use string basic type in enum declration", enum_decl->type);
		passed = false;
	}

	for (Ast_Ident_Literal_Pair& variant : enum_decl->variants)
	{
		auto name = name_set.find_key(variant.ident, hash_ident(variant.ident));
		if (!name) name_set.add(variant.ident, hash_ident(variant.ident));
		else { error("Duplicate enum variant identifier", variant.ident); passed = false; }

		if (is_unsigned && variant.is_negative)
		{
			error("Cannot use negative constant for enum of unsigned integer type", variant.ident);
			passed = false;
		}
	}

	return passed;
}

bool check_proc_decl(Ast* ast, Ast_Proc_Decl* proc_decl)
{
	bool passed = true;
	HashSet<Ast_Ident, u32, match_ident> name_set(16); //@Perf read check_struct_decl comment

	for (Ast_Ident_Type_Pair& param : proc_decl->input_params)
	{
		auto name = name_set.find_key(param.ident, hash_ident(param.ident));
		if (!name) name_set.add(param.ident, hash_ident(param.ident));
		else { error("Duplicate procedure input parameter identifier", param.ident); passed = false; }
		
		if (!check_type(ast, &param.type)) passed = false;
	}

	if (proc_decl->return_type)
	{
		if (!check_type(ast, &proc_decl->return_type.value())) passed = false;
	}

	return passed;
}

Ast* try_import(Ast* ast, std::optional<Ast_Ident> import)
{
	if (!import) return ast;

	Ast_Ident import_ident = import.value();
	auto import_decl = ast->import_table.find(import_ident, hash_ident(import_ident));
	if (!import_decl)
	{
		error("Import module not found", import_ident);
		return NULL;
	}

	return import_decl.value()->import_ast;
}

std::optional<Ast_Struct_Decl_Meta> find_struct(Ast* target_ast, Ast_Ident ident)
{
	return target_ast->struct_table.find(ident, hash_ident(ident));
}

std::optional<Ast_Enum_Decl_Meta> find_enum(Ast* target_ast, Ast_Ident ident)
{
	return target_ast->enum_table.find(ident, hash_ident(ident));
}

std::optional<Ast_Proc_Decl_Meta> find_proc(Ast* target_ast, Ast_Ident ident)
{
	return target_ast->proc_table.find(ident, hash_ident(ident));
}

std::optional<u32> find_enum_variant(Ast_Enum_Decl* enum_decl, Ast_Ident ident)
{
	u32 count = 0;
	for (Ast_Ident_Literal_Pair& variant : enum_decl->variants)
	{
		if (match_ident(variant.ident, ident)) return count;
		count += 1;
	}
	return {};
}

std::optional<u32> find_struct_field(Ast_Struct_Decl* struct_decl, Ast_Ident ident)
{
	u32 count = 0;
	for (Ast_Ident_Type_Pair& field : struct_decl->fields)
	{
		if (match_ident(field.ident, ident)) return count;
		count += 1;
	}
	return {};
}

Terminator check_block_cfg(Ast_Block* block, bool is_loop, bool is_defer)
{
	Terminator terminator = Terminator::None;

	for (Ast_Statement* statement : block->statements)
	{
		if (terminator != Terminator::None)
		{
			printf("Unreachable statement:\n");
			debug_print_statement(statement, 0);
			printf("\n");
			statement->unreachable = true;
			break;
		}

		switch (statement->tag)
		{
			case Ast_Statement::Tag::If:
			{
				check_if_cfg(statement->as_if, is_loop, is_defer);
			} break;
			case Ast_Statement::Tag::For: 
			{
				check_block_cfg(statement->as_for->block, true, is_defer);
			} break;
			case Ast_Statement::Tag::Block: 
			{
				terminator = check_block_cfg(statement->as_block, is_loop, is_defer);
			} break;
			case Ast_Statement::Tag::Defer:
			{
				if (is_defer)
				{
					printf("Nested defer blocks are not allowed:\n");
					debug_print_token(statement->as_defer->token, true, true);
					printf("\n");
				}
				else check_block_cfg(statement->as_defer->block, false, true);
			} break;
			case Ast_Statement::Tag::Break:
			{
				if (!is_loop)
				{
					if (is_defer) 
						printf("Break statement inside defer block is not allowed:\n");
					else printf("Break statement outside a loop:\n");
					debug_print_token(statement->as_break->token, true, true);
					printf("\n");
				}
				else terminator = Terminator::Break;
			} break;
			case Ast_Statement::Tag::Return:
			{
				if (is_defer)
				{
					printf("Defer block cant contain 'return' statements:\n");
					debug_print_token(statement->as_defer->token, true, true);
					printf("\n");
				}
				else terminator = Terminator::Return;
			} break;
			case Ast_Statement::Tag::Continue:
			{
				if (!is_loop)
				{
					if (is_defer)
						printf("Continue statement inside defer block is not allowed:\n");
					else printf("Continue statement outside a loop:\n");
					debug_print_token(statement->as_continue->token, true, true);
					printf("\n");
				}
				else terminator = Terminator::Continue;
			} break;
			case Ast_Statement::Tag::Proc_Call: break;
			case Ast_Statement::Tag::Var_Decl: break;
			case Ast_Statement::Tag::Var_Assign: break;
		}
	}

	return terminator;
}

void check_if_cfg(Ast_If* _if, bool is_loop, bool is_defer)
{
	check_block_cfg(_if->block, is_loop, is_defer);
	
	if (_if->_else)
	{
		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else::Tag::If)
			check_if_cfg(_else->as_if, is_loop, is_defer);
		else check_block_cfg(_else->as_block, is_loop, is_defer);
	}
}

//@Todo store proc context in Block_Stack and 
//type check return type with proc decl return type, create check_return()
static void check_block(Ast* ast, Block_Stack* bc, Ast_Block* block, bool add_block)
{
	if (add_block) block_stack_add_block(bc);

	for (Ast_Statement* statement: block->statements)
	{
		switch (statement->tag)
		{
			case Ast_Statement::Tag::If: check_if(ast, bc, statement->as_if); break;
			case Ast_Statement::Tag::For: check_for(ast, bc, statement->as_for); break;
			case Ast_Statement::Tag::Block: check_block(ast, bc, statement->as_block); break;
			case Ast_Statement::Tag::Defer: check_block(ast, bc, statement->as_defer->block); break;
			case Ast_Statement::Tag::Break: break;
			case Ast_Statement::Tag::Return: if (statement->as_return) check_expr(ast, bc, statement->as_return->expr.value()); break;
			case Ast_Statement::Tag::Continue: break;
			case Ast_Statement::Tag::Proc_Call: check_proc_call(ast, bc, statement->as_proc_call, true); break;
			case Ast_Statement::Tag::Var_Decl: check_var_decl(ast, bc, statement->as_var_decl); break;
			case Ast_Statement::Tag::Var_Assign: check_var_assign(ast, bc, statement->as_var_assign); break;
		}
	}

	block_stack_remove_block(bc);
}

void check_if(Ast* ast, Block_Stack* bc, Ast_If* _if)
{
	auto type = check_expr(ast, bc, _if->condition_expr);
	if (!type.has_value() || type_info_kind(type.value()) != Type_Kind::Bool)
	{
		printf("Expected conditional expression to be of type 'bool', got not bool or type error:\n");
		debug_print_token(_if->token, true, true);
		printf("\n");
	}

	check_block(ast, bc, _if->block);

	if (_if->_else)
	{
		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else::Tag::If)
			check_if(ast, bc, _else->as_if);
		else check_block(ast, bc, _else->as_block);
	}
}

void check_for(Ast* ast, Block_Stack* bc, Ast_For* _for)
{
	block_stack_add_block(bc);
	if (_for->var_decl) check_var_decl(ast, bc, _for->var_decl.value());
	if (_for->var_assign) check_var_assign(ast, bc, _for->var_assign.value());

	if (_for->condition_expr)
	{
		auto type = check_expr(ast, bc, _for->condition_expr.value());
		if (!type.has_value() || type_info_kind(type.value()) != Type_Kind::Bool)
		{
			printf("Expected conditional expression to be of type 'bool', got not bool or type error:\n");
			debug_print_token(_for->token, true, true);
			printf("\n");
		}
	}

	check_block(ast, bc, _for->block, false);
}

void check_var_decl(Ast* ast, Block_Stack* bc, Ast_Var_Decl* var_decl)
{
	Ast_Ident ident = var_decl->ident;
	bool is_valid_decl = true;

	if (block_stack_contains_var(bc, ident))
	{
		error("Variable already in scope in variable declaration", ident);
		is_valid_decl = false;
	}

	if (var_decl->type)
	{
		auto type = check_type(ast, &var_decl->type.value());
		
		if (var_decl->expr)
		{
			auto expr_type = check_expr(ast, bc, var_decl->expr.value());
			if (type && expr_type && !match_type_info(type.value(), expr_type.value()))
			{
				printf("Type mismatch in variable declaration:\n"); 
				debug_print_ident(var_decl->ident);
				printf("Expected: "); debug_print_type(type.value().type); printf("\n");
				printf("Got:      "); debug_print_type(expr_type.value().type); printf("\n\n");
			}
		}
		
		// adding var to the stack, even when expr type is not valid
		if (is_valid_decl && type)
		{
			Type_Info type_info = type.value();
			block_stack_add_var(bc, ident, type_info);
		}
	}
	else
	{
		auto expr_type = check_expr(ast, bc, var_decl->expr.value());
		if (!expr_type) return;

		var_decl->type = expr_type.value().type;

		// @Errors this might produce "var not found" error in later checks, might be solved by flagging
		// not adding var to the stack, when inferred type is not valid
		if (is_valid_decl)
		{
			Type_Info type_info = Type_Info { true, expr_type.value().type };
			block_stack_add_var(bc, ident, type_info);
		}
	}
}

void check_var_assign(Ast* ast, Block_Stack* bc, Ast_Var_Assign* var_assign)
{
	auto var_type = check_var(ast, bc, var_assign->var);
	auto expr_type = check_expr(ast, bc, var_assign->expr);
	
	if (var_assign->op != ASSIGN_OP_NONE)
	{
		printf("Check var assign: only '=' assign op is supported\n");
		debug_print_var_assign(var_assign, 0);
		printf("\n");
		return;
	}

	if (var_type && expr_type && !match_type_info(var_type.value(), expr_type.value()))
	{
		printf("Type mismatch in variable assignment:\n");
		debug_print_ident(var_assign->var->ident);
		printf("Expected: "); debug_print_type(var_type.value().type); printf("\n");
		printf("Got:      "); debug_print_type(expr_type.value().type); printf("\n\n");
	}
}

std::optional<Type_Info> check_type(Ast* ast, Ast_Type* type)
{
	switch (type->tag)
	{
		case Ast_Type::Tag::Basic:
		{
			return Type_Info { false, *type };
		}
		case Ast_Type::Tag::Array:
		{
			auto element_type = check_type(ast, &type->as_array->element_type);
			if (!element_type) return {};
			return Type_Info { false, *type };
		}
		case Ast_Type::Tag::Custom:
		{
			Ast* target_ast = try_import(ast, type->as_custom->import);
			if (target_ast == NULL) return {};

			auto struct_meta = find_struct(target_ast, type->as_custom->type);
			if (struct_meta)
			{
				type->tag = Ast_Type::Tag::Struct;
				type->as_struct.struct_id = struct_meta.value().struct_id;
				type->as_struct.struct_decl = struct_meta.value().struct_decl;
				return Type_Info { false, *type };
			}

			auto enum_meta = find_enum(target_ast, type->as_custom->type);
			if (enum_meta)
			{
				type->tag = Ast_Type::Tag::Enum;
				type->as_enum.enum_id = enum_meta.value().enum_id;
				type->as_enum.enum_decl = enum_meta.value().enum_decl;
				return Type_Info { false, *type };
			}

			printf("Failed to find the custom type: ");
			debug_print_custom_type(type->as_custom);
			printf("\n");
			debug_print_ident(type->as_custom->type);
			printf("\n");
			return {};
		}
		default:
		{
			printf("check_type: Unexpected Ast_Type::Tag, this should never happen!");
			debug_print_type(*type);
			printf("\n");
			return {};
		}
	}
}

//@Notice not accounting for is_var_owned propagation yet using Ast_Type directly as input
std::optional<Type_Info> check_access(Ast* ast, Block_Stack* bc, Ast_Access* access, Ast_Type type)
{
	switch (access->tag)
	{
	case Ast_Access::Tag::Var:
	{
		Ast_Var_Access* var_access = access->as_var;

		Type_Kind kind = type_kind(type);
		if (kind == Type_Kind::Pointer && type.pointer_level == 1 && type.tag == Ast_Type::Tag::Struct) kind = Type_Kind::Struct;
		if (kind != Type_Kind::Struct)
		{
			printf("Field access might only be used on variables of struct or pointer to a struct type:\n");
			debug_print_ident(var_access->ident);
			printf("\n");
			return {};
		}

		Ast_Struct_Decl* struct_decl = type.as_struct.struct_decl;
		auto field_id = find_struct_field(struct_decl, var_access->ident);
		if (!field_id) 
		{ 
			error("Failed to find struct field during access", var_access->ident); 
			return {}; 
		}
		var_access->field_id = field_id.value();

		Ast_Type result_type = struct_decl->fields[var_access->field_id].type;
		if (var_access->next) return check_access(ast, bc, var_access->next.value(), result_type);
		return Type_Info { false, result_type };
	}
	case Ast_Access::Tag::Array:
	{
		Ast_Array_Access* array_access = access->as_array;
		
		Type_Kind kind = type_kind(type);
		if (kind != Type_Kind::Array)
		{
			printf("Array access might only be used on variables of array type:\n");
			debug_print_access(access);
			printf("\n\n");
			return {};
		}

		auto expr_type = check_expr(ast, bc, array_access->index_expr);
		if (expr_type)
		{
			Type_Kind expr_kind = type_info_kind(expr_type.value());
			if (expr_kind != Type_Kind::Integer)
			{
				printf("Array access expression must be of integer type:\n");
				debug_print_expr(array_access->index_expr, 0);
				printf("\n\n");
			}
		}

		Ast_Type result_type = type.as_array->element_type;
		if (array_access->next) return check_access(ast, bc, array_access->next.value(), result_type);
		return Type_Info { false, result_type };
	}
	}
}

std::optional<Type_Info> check_expr(Ast* ast, Block_Stack* bc, Ast_Expr* expr)
{
	switch (expr->tag)
	{
		case Ast_Expr::Tag::Term: return check_term(ast, bc, expr->as_term);
		case Ast_Expr::Tag::Unary_Expr: return check_unary_expr(ast, bc, expr->as_unary_expr);
		case Ast_Expr::Tag::Binary_Expr: return check_binary_expr(ast, bc, expr->as_binary_expr);
		default: return {};
	}
}

std::optional<Type_Info> check_term(Ast* ast, Block_Stack* bc, Ast_Term* term)
{
	switch (term->tag)
	{
		case Ast_Term::Tag::Var: return check_var(ast, bc, term->as_var);
		case Ast_Term::Tag::Enum: return check_enum(ast, term->as_enum);
		case Ast_Term::Tag::Literal: return check_literal(&term->as_literal);
		case Ast_Term::Tag::Proc_Call: return check_proc_call(ast, bc, term->as_proc_call, false);
		default: return {};
	}
}

std::optional<Type_Info> check_var(Ast* ast, Block_Stack* bc, Ast_Var* var)
{
	auto type = block_stack_find_var_type(bc, var->ident);
	if (!type)
	{
		error("Check var: var is not found or has not valid type\n", var->ident);
		return {};
	}
	
	if (var->access)
	{
		return check_access(ast, bc, var->access.value(), type.value().type);
	}
	
	return type;
}

std::optional<Type_Info> check_enum(Ast* ast, Ast_Enum* _enum)
{
	Ast* target_ast = try_import(ast, _enum->import);
	if (target_ast == NULL) return {};
	
	// return none type if enum wasnt found
	auto enum_meta = find_enum(target_ast, _enum->type);
	if (!enum_meta) { error("Accessing undeclared enum", _enum->type); return {}; }
	Ast_Enum_Decl* enum_decl = enum_meta.value().enum_decl;
	_enum->enum_id = enum_meta.value().enum_id;

	// even when variant is invalid, return enum type
	auto variant_id = find_enum_variant(enum_decl, _enum->variant);
	if (!variant_id) { error("Accessing undeclared enum variant", _enum->variant); }
	else _enum->variant_id = variant_id.value();

	Ast_Type type = {};
	type.tag = Ast_Type::Tag::Enum;
	type.as_enum.enum_id = _enum->enum_id;
	type.as_enum.enum_decl = enum_meta.value().enum_decl;
	return Type_Info { false, type };
}

std::optional<Type_Info> check_literal(Ast_Literal* literal)
{
	//@Todo
	//handle string literals
	//handle integer limits and int type which is returned

	switch (literal->token.type)
	{
		case TOKEN_BOOL_LITERAL: return type_info_from_basic(BASIC_TYPE_BOOL);
		case TOKEN_FLOAT_LITERAL: return type_info_from_basic(BASIC_TYPE_F64);
		case TOKEN_INTEGER_LITERAL: return type_info_from_basic(BASIC_TYPE_I32);
		default:
		{
			printf("Check literal: unknown or unsupported literal:\n");
			debug_print_token(literal->token, true, true);
			printf("\n");
			return {};
		}
	}
}

std::optional<Type_Info> check_proc_call(Ast* ast, Block_Stack* bc, Ast_Proc_Call* proc_call, bool is_statement)
{
	Ast* target_ast = try_import(ast, proc_call->import);
	if (target_ast == NULL) return {};
	
	// find procedure
	Ast_Ident ident = proc_call->ident;
	auto proc_meta = find_proc(target_ast, ident);
	if (!proc_meta) { error("Calling undeclared procedure", ident); return {}; }
	Ast_Proc_Decl* proc_decl = proc_meta.value().proc_decl;
	proc_call->proc_id = proc_meta.value().proc_id;

	// check param count
	u32 param_count = (u32)proc_decl->input_params.size();
	u32 input_count = (u32)proc_call->input_exprs.size();
	if (param_count != input_count)
	{
		printf("Check proc call: unexpected number of input arguments:\n");
		debug_print_ident(ident, true, true);
		printf("Expected: %lu Input count: %lu \n\n", param_count, input_count);
	}

	// check input exprs and types 
	for (u32 i = 0; i < input_count; i += 1)
	{
		auto expr_type = check_expr(ast, bc, proc_call->input_exprs[i]);
		if (expr_type && i < param_count)
		{
			Ast_Type param_type = proc_decl->input_params[i].type;
			if (!match_type(param_type, expr_type.value().type))
			{
				printf("Type mismatch in procedure call input argument with id: %lu\n", i);
				debug_print_ident(proc_call->ident);
				printf("Expected: "); debug_print_type(param_type); printf("\n");
				printf("Got:      "); debug_print_type(expr_type.value().type); printf("\n\n");
			}
		}
	}

	if (is_statement)
	{
		if (proc_call->access)
		{
			printf("Procedure call statement cannot have access chains:\n");
			debug_print_proc_call(proc_call, 0);
			printf("\n");
		}

		if (proc_decl->return_type)
		{
			printf("Procedure call statement cannot discard the return type value:\n");
			debug_print_proc_call(proc_call, 0);
			printf("\n");
		}

		return {};
	}
	else
	{
		if (!proc_decl->return_type)
		{
			printf("Procedure call inside an expression must have a return type:\n");
			debug_print_proc_call(proc_call, 0);
			printf("\n");
			return {};
		}

		Ast_Type return_type = proc_decl->return_type.value();
		
		if (proc_call->access)
		{
			return check_access(ast, bc, proc_call->access.value(), return_type);
		}
		
		return Type_Info { false, return_type };
	}
}

std::optional<Type_Info> check_unary_expr(Ast* ast, Block_Stack* bc, Ast_Unary_Expr* unary_expr)
{
	auto rhs_result = check_expr(ast, bc, unary_expr->right);
	if (!rhs_result) return {};

	UnaryOp op = unary_expr->op;
	Type_Info rhs = rhs_result.value();
	Type_Kind rhs_kind = type_info_kind(rhs);

	switch (op)
	{
	case UNARY_OP_MINUS:
	{
		if (rhs_kind == Type_Kind::Float || rhs_kind == Type_Kind::Integer) return rhs;
		printf("Cannot apply unary op '-' to non float / integer type\n\n");
		return {};
	} break;
	case UNARY_OP_LOGIC_NOT:
	{
		if (rhs_kind == Type_Kind::Bool) return rhs;
		printf("Cannot apply unary op '!' to non bool type\n\n");
		return {};
	} break;
	case UNARY_OP_ADDRESS_OF:
	{
		if (!rhs.is_var_owned)
		{
			printf("Cannot take address of temporary value, use '&' with variables\n");
			debug_print_unary_expr(unary_expr, 0);
			printf("\n");
			return {};
		}

		rhs.type.pointer_level += 1;
		Type_Info ptr_type = Type_Info { false, rhs.type };
		return ptr_type;
	} break;
	case UNARY_OP_BITWISE_NOT:
	{
		if (rhs_kind == Type_Kind::Integer) return rhs;
		printf("Cannot apply unary op '~' to non integer type\n\n");
		return {};
	} break;
	default: return {};
	}
}

std::optional<Type_Info> check_binary_expr(Ast* ast, Block_Stack* bc, Ast_Binary_Expr* binary_expr)
{
	auto lhs_result = check_expr(ast, bc, binary_expr->left);
	auto rhs_result = check_expr(ast, bc, binary_expr->right);
	if (!lhs_result) return {};
	if (!rhs_result) return {};

	BinaryOp op = binary_expr->op;
	Type_Info lhs = lhs_result.value();
	Type_Info rhs = rhs_result.value();
	Type_Kind lhs_kind = type_info_kind(lhs);
	Type_Kind rhs_kind = type_info_kind(rhs);

	if (lhs_kind != rhs_kind)
	{
		printf("Type mismatch in binary expression\n");
		debug_print_binary_expr(binary_expr, 0);
		printf("\n");
		return {};
	}

	if (lhs_kind != Type_Kind::Bool && lhs_kind != Type_Kind::Float && lhs_kind != Type_Kind::Integer)
	{
		printf("Exprected bool float or integer type in binary expression\n");
		debug_print_binary_expr(binary_expr, 0);
		printf("\n");
		return {};
	}

	switch (op)
	{
	// LogicOps [&& ||]
	case BINARY_OP_LOGIC_AND:
	case BINARY_OP_LOGIC_OR:
	{
		if (lhs_kind == Type_Kind::Bool) return type_info_from_basic(BASIC_TYPE_BOOL);
		printf("Exprected bool operands in binary expression\n");
		debug_print_binary_expr(binary_expr, 0);
		printf("\n");
		return {};
	} break;
	//@Semantics == != should work for enums
	// CmpOps [< > <= >= == !=]
	case BINARY_OP_LESS:
	case BINARY_OP_GREATER:
	case BINARY_OP_LESS_EQUALS:
	case BINARY_OP_GREATER_EQUALS:
	case BINARY_OP_IS_EQUALS:
	case BINARY_OP_NOT_EQUALS:
	{
		if (lhs_kind == Type_Kind::Float || lhs_kind == Type_Kind::Integer) return type_info_from_basic(BASIC_TYPE_BOOL);
		printf("Exprected float or int in comparison binary expression\n");
		debug_print_binary_expr(binary_expr, 0);
		printf("\n");
		return {};
	} break;
	// MathOps [+ - * / ]
	case BINARY_OP_PLUS:
	case BINARY_OP_MINUS:
	case BINARY_OP_TIMES:
	case BINARY_OP_DIV:
	{
		if (lhs_kind == Type_Kind::Float || lhs_kind == Type_Kind::Integer) return lhs;
		printf("Exprected float or int in math binary expression\n");
		debug_print_binary_expr(binary_expr, 0);
		printf("\n");
		return {};
	} break;
	case BINARY_OP_MOD:
	{
		if (lhs_kind == Type_Kind::Integer) return lhs;
		printf("Exprected int in '%%' binary expression\n");
		debug_print_binary_expr(binary_expr, 0);
		printf("\n");
		return {};
	} break;
	// BitwiseOps [& | ^ << >>]
	case BINARY_OP_BITWISE_AND:
	case BINARY_OP_BITWISE_OR:
	case BINARY_OP_BITWISE_XOR:
	case BINARY_OP_BITSHIFT_LEFT:
	case BINARY_OP_BITSHIFT_RIGHT:
	{
		if (lhs_kind == Type_Kind::Integer) return lhs;
		printf("Exprected int in bitwise binary expression\n");
		debug_print_binary_expr(binary_expr, 0);
		printf("\n");
		return {};
	} break;
	default: return {};
	}
}

Type_Kind type_kind(Ast_Type type)
{
	if (type.pointer_level > 0) return Type_Kind::Pointer;

	switch (type.tag)
	{
	case Ast_Type::Tag::Basic:
	{
		switch (type.as_basic)
		{
		case BASIC_TYPE_I8:
		case BASIC_TYPE_U8:
		case BASIC_TYPE_I16:
		case BASIC_TYPE_U16:
		case BASIC_TYPE_I32:
		case BASIC_TYPE_U32:
		case BASIC_TYPE_I64:
		case BASIC_TYPE_U64:
			return Type_Kind::Integer;
		case BASIC_TYPE_F32:
		case BASIC_TYPE_F64:
			return Type_Kind::Float;
		case BASIC_TYPE_BOOL:
			return Type_Kind::Bool;
		case BASIC_TYPE_STRING:
			return Type_Kind::String;
		}
	}
	case Ast_Type::Tag::Array: return Type_Kind::Array;
	case Ast_Type::Tag::Struct: return Type_Kind::Struct;
	case Ast_Type::Tag::Enum: return Type_Kind::Enum;
	case Ast_Type::Tag::Custom:
	{
		printf("type_info_kind: Ast_Type::Tag::Custom is not allowed type must be checked and disambiguated beforehand.\n");
		return Type_Kind::Struct;
	}
	}
}

Type_Kind type_info_kind(Type_Info type_info)
{
	return type_kind(type_info.type);
}

Type_Info type_info_from_basic(BasicType basic_type)
{
	Ast_Type type = {};
	type.tag = Ast_Type::Tag::Basic;
	type.as_basic = basic_type;
	return Type_Info { false, type };
}

bool match_type_info(Type_Info type_info_a, Type_Info type_info_b)
{
	return match_type(type_info_a.type, type_info_b.type);
}

bool match_type(Ast_Type type_a, Ast_Type type_b)
{
	if (type_a.pointer_level != type_b.pointer_level) return false;
	if (type_a.tag != type_b.tag) return false;

	switch (type_a.tag)
	{
		case Ast_Type::Tag::Basic: return type_a.as_basic == type_b.as_basic;
		case Ast_Type::Tag::Struct: return type_a.as_struct.struct_id == type_b.as_struct.struct_id;
		case Ast_Type::Tag::Enum: return type_a.as_enum.enum_id == type_b.as_enum.enum_id;
		case Ast_Type::Tag::Array:
		{
			Ast_Array_Type* array_a = type_a.as_array;
			Ast_Array_Type* array_b = type_b.as_array;
			if (array_a->is_dynamic != array_b->is_dynamic) return false;
			if (array_a->fixed_size != array_b->fixed_size) return false;
			return match_type(array_a->element_type, array_b->element_type);
		}
		default:
		{
			printf("match_type: Unexpected Ast_Type::Tag. Disambiguate Tag::Custom by using check_type first:\n");
			debug_print_type(type_a); printf("\n");
			debug_print_type(type_b); printf ("\n");
			return false;
		}
	}
}

void block_stack_reset(Block_Stack* bc)
{
	bc->block_count = 0;
	bc->var_count_stack.clear();
	bc->var_stack.clear();
}

void block_stack_add_block(Block_Stack* bc)
{
	bc->block_count += 1;
	bc->var_count_stack.emplace_back(0);
}

void block_stack_remove_block(Block_Stack* bc)
{
	u32 var_count = bc->var_count_stack[bc->block_count - 1];
	for (u32 i = 0; i < var_count; i += 1)
	{
		bc->var_stack.pop_back();
	}
	bc->var_count_stack.pop_back();
	bc->block_count -= 1;
}

void block_stack_add_var(Block_Stack* bc, Ast_Ident ident, Type_Info type_info)
{
	bc->var_count_stack[bc->block_count - 1] += 1;
	bc->var_stack.emplace_back(Var_Info { ident, type_info });
}

bool block_stack_contains_var(Block_Stack* bc, Ast_Ident ident)
{
	for (Var_Info& var : bc->var_stack)
	{
		if (match_ident(var.ident, ident))
			return true;
	}
	return false;
}

std::optional<Type_Info> block_stack_find_var_type(Block_Stack* bc, Ast_Ident ident)
{
	for (Var_Info& var : bc->var_stack)
	{
		if (match_ident(var.ident, ident))
			return var.type_info;
	}
	return {};
}

void error_pair(const char* message, const char* labelA, Ast_Ident identA, const char* labelB, Ast_Ident identB)
{
	printf("%s:\n", message);
	printf("%s: ", labelA);
	debug_print_ident(identA, true, true);
	printf("%s: ", labelB);
	debug_print_ident(identB, true, true);
	printf("\n");
}

void error(const char* message, Ast_Ident ident)
{
	printf("%s:\n", message);
	debug_print_ident(ident, true, true);
	printf("\n");
}
