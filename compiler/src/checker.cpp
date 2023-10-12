#include "checker.h"

#include "debug_printer.h"

bool check_declarations(Ast* ast, Ast_Program* program, Module_Map& modules)
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

	for (Ast_Enum_Decl* decl : ast->enums)
	{
		Ast_Ident ident = decl->type;
		auto key = symbol_table.find_key(ident, hash_ident(ident));
		if (!key) { symbol_table.add(ident, hash_ident(ident)); ast->enum_table.add(ident, decl, hash_ident(ident)); }
		else { error_pair("Symbol already declared", "Enum", ident, "Symbol", key.value()); passed = false; }
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

	//@Low priority
	//@Rule todo: cant import same thing under multiple names
	//@Rule todo: cant import same type or procedure under multiple names
	
	return passed;
}

bool check_ast(Ast* ast, Ast_Program* program)
{
	bool passed = true;
	
	// Find and add use symbols to current scope
	for (Ast_Use_Decl* decl : ast->uses)
	{
		Ast* import_ast = try_import(ast, { decl->import });
		if (import_ast == NULL)
		{
			passed = false;
			continue;
		}

		Ast_Ident alias = decl->alias;
		Ast_Ident symbol = decl->symbol;
		auto struct_decl = import_ast->struct_table.find(symbol, hash_ident(symbol));
		if (struct_decl) { ast->struct_table.add(alias, struct_decl.value(), hash_ident(alias)); continue; }
		auto enum_decl = import_ast->enum_table.find(symbol, hash_ident(symbol));
		if (enum_decl) { ast->enum_table.add(alias, enum_decl.value(), hash_ident(alias)); continue; }
		auto proc_decl = import_ast->proc_table.find(symbol, hash_ident(symbol));
		if (proc_decl) { ast->proc_table.add(alias, proc_decl.value(), hash_ident(alias)); continue; }

		error("Use symbol isnt found in imported namespace", symbol); //@Improve error
		passed = false;
	}

	Block_Stack bc = {};
	//@Notice not setting passed flag in checks
	for (Ast_Proc_Decl* proc_decl : ast->procs)
	{
		if (proc_decl->is_external) continue;

		//@Notice this doesnt correctly handle if else on top level, which may allow all paths to return
		Terminator terminator = check_block_cfg(proc_decl->block, false, false, true);
		if (proc_decl->return_type.has_value() && terminator != Terminator::Return)
		error("Not all control flow paths return value", proc_decl->ident);
		
		//@Notice need to add input variables to block stack
		block_stack_reset(&bc);
		block_stack_add_block(&bc);
		for (Ast_Ident_Type_Pair& param : proc_decl->input_params)
		{
			if (block_stack_contains_var(&bc, param.ident))
			{
				error("Input parameter with same name is already exists", param.ident);
			}
			else block_stack_add_var(&bc, param.ident);
		}
		check_block(ast, &bc, proc_decl->block, false);
	}

	return true;
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

Terminator check_block_cfg(Ast_Block* block, bool is_loop, bool is_defer, bool is_entry)
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
			case Ast_Statement::Tag::Return: break;
			case Ast_Statement::Tag::Continue: break;
			case Ast_Statement::Tag::Proc_Call: check_proc_call(ast, bc, statement->as_proc_call); break;
			case Ast_Statement::Tag::Var_Decl: check_var_decl(ast, bc, statement->as_var_decl); break;
			case Ast_Statement::Tag::Var_Assign: check_var_assign(ast, bc, statement->as_var_assign); break;
		}
	}

	block_stack_remove_block(bc);
}

void check_if(Ast* ast, Block_Stack* bc, Ast_If* _if)
{
	//@Check expr, must be bool
	auto type = check_expr(ast, bc, _if->condition_expr);
	if (!type.has_value() || (!type.value().is_basic || type.value().basic_type != BASIC_TYPE_BOOL))
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

	//@Check expr, must be bool
	if (_for->condition_expr)
	{
		auto type = check_expr(ast, bc, _for->condition_expr.value());
		if (!type.has_value() || (!type.value().is_basic || type.value().basic_type != BASIC_TYPE_BOOL))
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
	if (block_stack_contains_var(bc, ident))
	{
		error("Variable already in scope in variable declaration", ident);
		return;
	}
	block_stack_add_var(bc, ident);

	//@Check type
	//@Check expr
}

void check_var_assign(Ast* ast, Block_Stack* bc, Ast_Var_Assign* var_assign)
{
	Ast_Ident ident = var_assign->var->ident;
	if (!block_stack_contains_var(bc, ident))
	{
		error("Variable not in scope in variable assignment", ident);
		return;
	}
	
	//@Check var access
	//@Check type
	//@Check expr
	//@Check assign op
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
		case Ast_Term::Tag::Enum: return check_enum(ast, bc, term->as_enum);
		case Ast_Term::Tag::Literal: return check_literal(ast, bc, term->as_literal);
		case Ast_Term::Tag::Proc_Call: return check_proc_call(ast, bc, term->as_proc_call);
		default: return {};
	}
}

std::optional<Type_Info> check_var(Ast* ast, Block_Stack* bc, Ast_Var* var)
{
	return {};
}

std::optional<Type_Info> check_enum(Ast* ast, Block_Stack* bc, Ast_Enum* _enum)
{
	return {};
}

std::optional<Type_Info> check_literal(Ast* ast, Block_Stack* bc, Ast_Literal literal)
{
	//@Todo
	//handle string literals
	//handle integer limits and int type which is returned

	switch (literal.token.type)
	{
		case TOKEN_BOOL_LITERAL: return Type_Info { true, BASIC_TYPE_BOOL, NULL };
		case TOKEN_FLOAT_LITERAL: return Type_Info { true, BASIC_TYPE_F64, NULL };
		case TOKEN_INTEGER_LITERAL: return Type_Info { true, BASIC_TYPE_I32, NULL };
		default:
		{
			printf("Unknown or unsupported literal value:\n");
			debug_print_token(literal.token, true, true);
			printf("\n");
			return {};
		}
	}
}

std::optional<Type_Info> check_proc_call(Ast* ast, Block_Stack* bc, Ast_Proc_Call* proc_call)
{
	Ast* ast_target = try_import(ast, proc_call->import);
	if (ast_target == NULL) return {};

	Ast_Ident ident = proc_call->ident;
	Ast_Proc_Decl* proc_decl = NULL;

	auto proc_meta = ast_target->proc_table.find(ident, hash_ident(ident));
	if (!proc_meta)
	{
		error("Calling undeclared procedure", ident);
		return {};
	}
	else
	{
		proc_call->proc_id = proc_meta.value().proc_id;
		proc_decl = proc_meta.value().proc_decl;
	}

	//@Check input exprs
	//@Check statement cant discard return type
	//@Check access
	//return the return type

	return {};
}

std::optional<Type_Info> check_unary_expr(Ast* ast, Block_Stack* bc, Ast_Unary_Expr* unary_expr)
{
	auto rhs_result = check_expr(ast, bc, unary_expr->right);
	if (!rhs_result) return {};

	UnaryOp op = unary_expr->op;
	Type_Info rhs = rhs_result.value();

	//@Check op semantics

	return {};
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
	
	//@Check op semantics

	return {};
}

bool match_type_info(Type_Info type_a, Type_Info type_b)
{
	bool basic_a = type_a.is_basic;
	bool basic_b = type_b.is_basic;
	if (basic_a != basic_b) return false;
	if (basic_a && type_a.basic_type != type_b.basic_type) return false;
	return match_type(type_a.type, type_b.type);
}

bool match_type(Ast_Type* type_a, Ast_Type* type_b)
{
	if (type_a->tag != type_b->tag) return false;

	switch (type_a->tag)
	{
		case Ast_Type::Tag::Basic:
		{
			return type_a->as_basic == type_b->as_basic;
		} break;
		case Ast_Type::Tag::Pointer:
		{
			return match_type(type_a->as_pointer, type_b->as_pointer);
		} break;
		case Ast_Type::Tag::Array:
		{
			Ast_Array_Type* array_a = type_a->as_array;
			Ast_Array_Type* array_b = type_b->as_array;
			if (array_a->is_dynamic != array_b->is_dynamic) return false;
			if (array_a->fixed_size != array_b->fixed_size) return false;
			return match_type(array_a->element_type, array_b->element_type);
		} break;
		case Ast_Type::Tag::Custom:
		{
			Ast_Custom_Type* custom_a = type_a->as_custom;
			Ast_Custom_Type* custom_b = type_b->as_custom;
			bool import_a = custom_a->import.has_value();
			bool import_b = custom_b->import.has_value();
			if (import_a != import_b) return false;
			if (import_a && !match_ident(custom_a->import.value(), custom_b->import.value())) return false; 
			return match_ident(custom_a->type, custom_b->type);
		} break;
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

void block_stack_add_var(Block_Stack* bc, Ast_Ident ident)
{
	bc->var_count_stack[bc->block_count - 1] += 1;
	bc->var_stack.emplace_back(ident);
}

bool block_stack_contains_var(Block_Stack* bc, Ast_Ident ident)
{
	for (Ast_Ident& var_ident : bc->var_stack)
	{
		if (match_ident(var_ident, ident))
			return true;
	}
	return false;
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
