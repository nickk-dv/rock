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

	
	//@Notice not setting passed flag in checks
	for (Ast_Proc_Decl* proc_decl : ast->procs)
	{
		if (proc_decl->is_external) continue;

		//@Notice this doesnt correctly handle if else on top level, which may allow all paths to return
		Terminator terminator = check_block_cfg(proc_decl->block, false, false, true);
		if (proc_decl->return_type.has_value() && terminator != Terminator::Return)
		error("Not all control flow paths return value", proc_decl->ident);
		
		//@Notice need to add input variables to block stack
		Block_Stack bc = {};
		//block_stack_reset(&bc);
		printf("check block call \n");
		check_block(ast, &bc, proc_decl->block);
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
				}
				else terminator = Terminator::Break;
			} break;
			case Ast_Statement::Tag::Return:
			{
				if (is_defer)
				{
					printf("Defer block cant contain 'return' statements:\n");
					debug_print_token(statement->as_defer->token, true, true);
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
				}
				else terminator = Terminator::Continue;
			} break;
			case Ast_Statement::Tag::Proc_Call: break;
			case Ast_Statement::Tag::Var_Decl: break;
			case Ast_Statement::Tag::Var_Assign: break;
			default: break;
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
		if (_else->tag == Ast_Else::Tag::If) check_if_cfg(_else->as_if, is_loop, is_defer);
		else check_block_cfg(_else->as_block, is_loop, is_defer);
	}
}

static void check_block(Ast* ast, Block_Stack* bc, Ast_Block* block)
{
	block_stack_add_block(bc);

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
			default: break;
		}
	}

	block_stack_remove_block(bc);
}

void check_if(Ast* ast, Block_Stack* bc, Ast_If* _if)
{
	check_block(ast, bc, _if->block);

	if (_if->_else)
	{
		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else::Tag::If) check_if(ast, bc, _else->as_if);
		else check_block(ast, bc, _else->as_block);
	}
}

void check_for(Ast* ast, Block_Stack* bc, Ast_For* _for)
{
	//@Check handle var decl
	//@Check handle var assign
	//@Notice var should be declared inside the for block scope to not leak in outside scope
	check_block(ast, bc, _for->block);
}

static void check_proc_call(Ast* ast, Block_Stack* bc, Ast_Proc_Call* proc_call)
{
	Ast* ast_target = try_import(ast, proc_call->import);
	if (ast_target == NULL) return;
	
	Ast_Ident ident = proc_call->ident;
	Ast_Proc_Decl* proc_decl = NULL;

	auto proc_meta = ast_target->proc_table.find(ident, hash_ident(ident));
	if (!proc_meta)
	{
		error("Calling undeclared procedure", ident);
		return;
	}
	else
	{
		proc_call->proc_id = proc_meta.value().proc_id;
		proc_decl = proc_meta.value().proc_decl;
	}

	//@Check input exprs
	//@Check statement cant discard return type
	//return the return type
}

void check_var_decl(Ast* ast, Block_Stack* bc, Ast_Var_Decl* var_decl)
{
	Ast_Ident var_ident = var_decl->ident;
	if (block_stack_contains_var(bc, var_ident))
	{
		error("Variable already in scope in variable declaration", var_ident);
		return;
	}
	block_stack_add_var(bc, var_ident);

	//@Check type
	//@Check expr
}

void check_var_assign(Ast* ast, Block_Stack* bc, Ast_Var_Assign* var_assign)
{
	Ast_Ident var_ident = var_assign->var->ident;
	if (!block_stack_contains_var(bc, var_ident))
	{
		error("Variable not in scope in variable assignment", var_ident);
		return;
	}
	
	//@Check var access
	//@Check type
	//@Check expr
	//@Check assign op
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

/*
bool Checker::check_ast(Ast* ast)
{
	proc_table.init(64);
	typer.init_primitive_types();

	if (!check_types_and_proc_definitions(ast)) return false;
	
	bool declarations_valid = true;
	for (Ast_Struct_Decl* decl : ast->structs) if (!check_struct_decl(decl)) declarations_valid = false;
	for (Ast_Enum_Decl* decl : ast->enums) if (!check_enum_decl(decl)) declarations_valid = false;
	for (Ast_Proc_Decl* decl : ast->procs) if (!check_proc_decl(decl)) declarations_valid = false;
	if (!declarations_valid) return false;

	bool procedure_blocks_valid = true;
	for (Ast_Proc_Decl* decl : ast->procs)
	if (!check_proc_block(decl)) procedure_blocks_valid = false;
	if (!procedure_blocks_valid) return false;

	return true;
}

bool Checker::check_types_and_proc_definitions(Ast* ast)
{
	for (Ast_Struct_Decl* decl : ast->structs)
	{
		if (typer.is_type_in_scope(&decl->type)) { printf("Struct type redifinition.\n"); return false; }
		typer.add_struct_type(decl);
	}
	for (Ast_Enum_Decl* decl : ast->enums)
	{
		if (typer.is_type_in_scope(&decl->type)) { printf("Enum type redifinition.\n"); return false; }
		typer.add_enum_type(decl);
	}
	for (Ast_Proc_Decl* decl : ast->procs)
	{
		if (is_proc_in_scope(&decl->ident)) { printf("Procedure redifinition"); return false; }
		proc_table.add(decl->ident.token.string_value, decl, hash_fnv1a_32(decl->ident.token.string_value));
	}
	return true;
}

bool Checker::is_proc_in_scope(Ast_Ident* proc_ident)
{
	return proc_table.contains(proc_ident->token.string_value, hash_fnv1a_32(proc_ident->token.string_value));
}

Ast_Proc_Decl* Checker::get_proc_decl(Ast_Ident* proc_ident)
{
	return proc_table.find(proc_ident->token.string_value, hash_fnv1a_32(proc_ident->token.string_value)).value();
}

bool Checker::check_struct_decl(Ast_Struct_Decl* struct_decl) //@Incomplete allow for multple errors
{
	if (struct_decl->fields.empty()) { printf("Struct must have at least 1 field.\n"); return false; }

	HashSet<StringView, u32, match_string_view> names(16); //@Perf later try to re-use it by adding reset option
	for (auto& field : struct_decl->fields)
	{
		if (names.contains(field.ident.token.string_value, hash_fnv1a_32(field.ident.token.string_value))) { printf("Field name redifinition.\n"); return false; }
		if (!typer.is_type_in_scope(&field.type)) { printf("Field type is not in scope.\n"); return false; }
		names.add(field.ident.token.string_value, hash_fnv1a_32(field.ident.token.string_value));
	}
	return true;
}

bool Checker::check_enum_decl(Ast_Enum_Decl* enum_decl) //@Incomplete allow for multple errors
{
	if (enum_decl->variants.empty()) { printf("Enum must have at least 1 variant.\n"); return false; }

	HashSet<StringView, u32, match_string_view> names(16); //@Perf later try to re-use it by adding reset option
	for (const auto& field : enum_decl->variants)
	{
		if (names.contains(field.ident.token.string_value, hash_fnv1a_32(field.ident.token.string_value))) { printf("Variant name redifinition.\n"); return false; }
		names.add(field.ident.token.string_value, hash_fnv1a_32(field.ident.token.string_value));
	}
	return true;
}

bool Checker::check_proc_decl(Ast_Proc_Decl* proc_decl)
{
	HashSet<StringView, u32, match_string_view> names(16); //@Perf later try to re-use it by adding reset option
	for (auto& param : proc_decl->input_params)
	{
		if (names.contains(param.ident.token.string_value, hash_fnv1a_32(param.ident.token.string_value))) { printf("Procedure parameter name redifinition.\n"); return false; }
		if (!typer.is_type_in_scope(&param.type)) { printf("Procedure parameter type is not in scope.\n"); return false; }
		names.add(param.ident.token.string_value, hash_fnv1a_32(param.ident.token.string_value));
	}
	if (proc_decl->return_type.has_value() && !typer.is_type_in_scope(&proc_decl->return_type.value())) 
	{ printf("Procedure return type is not in scope.\n"); return false; }
	return true;
}

bool Checker::check_proc_block(Ast_Proc_Decl* proc_decl)
{
	Block_Checker bc = {};
	bc.block_enter(proc_decl->block, false);
	for (const auto& param : proc_decl->input_params)
	bc.var_add(param);

	bool result = check_block(proc_decl->block, &bc, true, false);
	return result;
}

void Block_Checker::block_enter(Ast_Block* block, bool is_inside_loop)
{
	if (block_stack.size() > 0 && is_inside_a_loop()) is_inside_loop = true;
	block_stack.emplace_back( Block_Info { block, 0, is_inside_loop });
}

void Block_Checker::block_exit()
{
	Block_Info block = block_stack[block_stack.size() - 1]; //@Perf copying this its small for now
	for (u32 i = 0; i < block.var_count; i++)
		var_stack.pop_back();
	block_stack.pop_back();
}

void Block_Checker::var_add(const Ast_Ident_Type_Pair& ident_type)
{
	var_stack.emplace_back(ident_type);
	block_stack[block_stack.size() - 1].var_count += 1;
}

void Block_Checker::var_add(const Ast_Ident& ident, const Ast_Ident& type)
{
	var_stack.emplace_back(Ast_Ident_Type_Pair{ ident, type });
	block_stack[block_stack.size() - 1].var_count += 1;
}

bool Block_Checker::is_var_declared(const Ast_Ident& ident) //@Perf linear search for now
{
	for (const auto& var : var_stack)
	if (var.ident.token.string_value == ident.token.string_value) return true;
	return false;
}

bool Block_Checker::is_inside_a_loop()
{
	return block_stack[block_stack.size() - 1].is_inside_loop;
}

Ast_Ident Block_Checker::var_get_type(const Ast_Ident& ident)
{
	for (const auto& var : var_stack)
	if (var.ident.token.string_value == ident.token.string_value) return var.type;
	printf("FATAL Block_Checker::var_get_type expected to find the var but didnt");
	return {};
}

bool Checker::check_block(Ast_Block* block, Block_Checker* bc, bool is_entry, bool is_inside_loop)
{
	if (!is_entry) 
	bc->block_enter(block, is_inside_loop);

	for (Ast_Statement* stmt : block->statements)
	{
		switch (stmt->tag)
		{
			case Ast_Statement::Tag::If: { if (!check_if(stmt->as_if, bc)) return false; } break;
			case Ast_Statement::Tag::For: { if (!check_for(stmt->as_for, bc)) return false; } break;
			case Ast_Statement::Tag::Break: { if (!check_break(stmt->as_break, bc)) return false; } break;
			case Ast_Statement::Tag::Return: { if (!check_return(stmt->as_return, bc)) return false; } break;
			case Ast_Statement::Tag::Continue: { if (!check_continue(stmt->as_continue, bc)) return false; } break;
			case Ast_Statement::Tag::Proc_Call: 
			{
				bool is_valid = false;
				check_proc_call(stmt->as_proc_call, bc, is_valid);
				if (!is_valid) return false;
			} break;
			case Ast_Statement::Tag::Var_Decl: { if (!check_var_decl(stmt->as_var_decl, bc)) return false; } break;
			case Ast_Statement::Tag::Var_Assign: { if (!check_var_assign(stmt->as_var_assign, bc)) return false; } break;
			default: break;
		}
	}

	bc->block_exit();
	return true;
}

bool Checker::check_if(Ast_If* _if, Block_Checker* bc)
{
	condition_expr is bool
	condition_expr valid
	block valid
	else is valid or doesnt exist

	std::optional<Type_Info> expr_type = check_expr(_if->condition_expr, bc);
	if (!expr_type) return false;
	if (!expr_type.value().is_bool())
	{
		printf("Expected conditional expression to evaluate to 'bool' type.\n");
		debug_print_token(_if->token, true, true);
		printf("Expr type: "); typer.debug_print_type_info(&expr_type.value());
		return false;
	}

	if (!check_block(_if->block, bc, false, false)) return false;
	if (_if->_else.has_value() && !check_else(_if->_else.value(), bc)) return false;
	
	return true;
}

bool Checker::check_else(Ast_Else* _else, Block_Checker* bc)
{
	switch (_else->tag)
	{
		case Ast_Else::Tag::If: { if (!check_if(_else->as_if, bc)) return false; } break;
		case Ast_Else::Tag::Block: { if (!check_block(_else->as_block, bc, false, false)) return false; } break;
		default: break;
	}
	return true;
}

bool Checker::check_for(Ast_For* _for, Block_Checker* bc)
{
	not sure about syntax & semantics yet

	if (!check_block(_for->block, bc, false, true)) return false;
	return true;
}

bool Checker::check_break(Ast_Break* _break, Block_Checker* bc)
{
	if (!bc->is_inside_a_loop())
	{
		printf("Invalid 'break' outside a for loop.\n");
		debug_print_token(_break->token, true, true);
		return false;
	}
	return true;
}

bool Checker::check_return(Ast_Return* _return, Block_Checker* bc)
{
	expr matches parent proc return type or lack of type

	//@Idea put Proc_Declaration* on Ast_Return, which will set during first checker pass

	if (_return->expr.has_value())
	{
		std::optional<Type_Info> expr_type = check_expr(_return->expr.value(), bc);
		if (!expr_type) return false;

		//@Check proc decl return type matches
	}
	else
	{
		//@Check proc decl also has no return type
	}

	return true;
}

bool Checker::check_continue(Ast_Continue* _continue, Block_Checker* bc)
{
	if (!bc->is_inside_a_loop())
	{
		printf("Invalid 'continue' outside a for loop.\n");
		debug_print_token(_continue->token, true, true);
		return false;
	}
	return true;
}

std::optional<Type_Info> Checker::check_proc_call(Ast_Proc_Call* proc_call, Block_Checker* bc, bool& is_valid)
{
	proc name in scope
	param count and decl param count match
	all param types in scope
	all input expr are valid
	all input expr match param types
	return type in scope
	
	if (!is_proc_in_scope(&proc_call->ident))
	{
		printf("Calling unknown procedure.\n");
		printf("Procedure: ");
		debug_print_token(proc_call->ident.token, true, true); 
		is_valid = false;
		return {};
	}

	Ast_Proc_Decl* proc_decl = get_proc_decl(&proc_call->ident);

	u64 decl_param_count = proc_decl->input_params.size();
	u64 call_param_count = proc_call->input_exprs.size();
	if (decl_param_count != call_param_count)
	{
		printf("Calling procedure with incorrect number of params. Expected: %llu Calling with: %llu.\n", decl_param_count, call_param_count);
		printf("Procedure: ");
		debug_print_token(proc_call->ident.token, true, true);
		is_valid = false; 
		return {};
	}
	
	u64 counter = 0;
	for (auto& param : proc_decl->input_params)
	{
		if (!typer.is_type_in_scope(&param.type)) //@Redundant function decls should already have verified types, no need to check this
		{
			printf("Calling procedure with param type out of scope.\n");
			printf("Procedure: ");
			debug_print_token(proc_call->ident.token, true, true);
			printf("Type out of scope: ");
			debug_print_token(param.type.token, true, true);
			is_valid = false;
			return {};
		}

		std::optional<Type_Info> param_expr_type = check_expr(proc_call->input_exprs[counter], bc);
		if (!param_expr_type) return {};
		counter += 1;

		Type_Info param_type = typer.get_type_info(&param.type);
		if (!typer.is_type_equals_type(&param_expr_type.value(), &param_type))
		{
			printf("Type mismatch between param type and input expression.\n"); //@Print out type infos to indicate which ones
			printf("Procedure: ");
			debug_print_token(proc_call->ident.token, true, true);
			printf("Expected input type: ");
			typer.debug_print_type_info(&param_type);
			printf("Expression type: ");
			typer.debug_print_type_info(&param_expr_type.value());
			is_valid = false;
			return {};
		}
	}

	if (!proc_decl->return_type.has_value())
	{
		is_valid = true;
		return {};
	}

	if (!typer.is_type_in_scope(&proc_decl->return_type.value()))
	{
		printf("Calling procedure with return type out of scope.\n");
		printf("Procedure: ");
		debug_print_token(proc_call->ident.token, true, true);
		printf("Return type: ");
		debug_print_token(proc_decl->return_type.value().token, true, true);
		is_valid = false;
		return {};
	}

	is_valid = true;
	return typer.get_type_info(&proc_decl->return_type.value());
}

bool Checker::check_var_decl(Ast_Var_Decl* var_decl, Block_Checker* bc)
{
	ident must not be in scope
	[has expr & has type]
	expr valid
	expr evaluates to same type
	[has expr]
	expr valid
	infer var type
	[result]
	existing or inferred type must be in scope
	add ident & type to var_stack

	if (bc->is_var_declared(var_decl->ident)) 
	{ 
		printf("Can not shadow already declared variable.\n");
		debug_print_token(var_decl->ident.token, true, true);
		return false; 
	}

	Ast_Ident type_ident = {};
	
	if (var_decl->type.has_value()) //has type, might have expr
	{
		type_ident = var_decl->type.value();

		if (!typer.is_type_in_scope(&type_ident))
		{
			printf("Type is not in scope.\n");
			debug_print_token(type_ident.token, true, true);
			return false;
		}

		if (var_decl->expr.has_value()) //check that expr matches type
		{
			std::optional<Type_Info> expr_type = check_expr(var_decl->expr.value(), bc);
			if (!expr_type) return false;

			Type_Info var_type = typer.get_type_info(&type_ident);
			if (!typer.is_type_equals_type(&expr_type.value(), &var_type))
			{
				printf("Var declaration type mismatch.\n");
				typer.debug_print_type_info(&var_type);
				typer.debug_print_type_info(&expr_type.value());
				return false;
			}
		}
	}
	else //no type, infer from expr
	{
		std::optional<Type_Info> expr_type = check_expr(var_decl->expr.value(), bc);
		if (!expr_type) return false;

		//@Problem no inference possible due to needing Ast_Ident to push var on var_stack
		//cannot go from expr Type_Info to Ast_Ident
		//will test current setup with all types declared

		printf("Type inference is NOT YET SUPPORTED.\n"); //@Incomplete @Check
		debug_print_token(var_decl->ident.token, true, true);
		return false;
	}

	bc->var_add(var_decl->ident, type_ident);
	
	return true;
}

bool Checker::check_var_assign(Ast_Var_Assign* var_assign, Block_Checker* bc)
{
	access chain must be valid
	expr valid
	AssignOp = : expr evaluates to same type
	AssignOp other: supported by lhs-rhs

	auto chain_type = check_ident_chain(var_assign->ident_chain, bc);
	if (!chain_type) return false;
	std::optional<Type_Info> expr_type = check_expr(var_assign->expr, bc);
	if (!expr_type) return false;

	if (var_assign->op == ASSIGN_OP_NONE)
	{
		if (!typer.is_type_equals_type(&chain_type.value(), &expr_type.value()))
		{
			printf("Var assignment type mismatch.\n");
			debug_print_token(var_assign->ident_chain->ident.token, true, true); //@Hack just for location, redundant first ident print
			debug_print_ident_chain(var_assign->ident_chain); //@Only printing first token of the chain
			printf("Var  Type: "); typer.debug_print_type_info(&chain_type.value());
			printf("Expr Type: "); typer.debug_print_type_info(&expr_type.value());
			return false;
		}
	}
	else
	{
		//@Incomplete assign op evaluation (similar to binary ops)
		printf("AssignOps are not supported yet.\n");
		return false;
	}

	return true;
}

std::optional<Type_Info> Checker::check_ident_chain(Ast_Ident_Chain* ident_chain, Block_Checker* bc)
{
	first ident is declared variable
	further idents exist within the type
	return the last type

	if (!bc->is_var_declared(ident_chain->ident))
	{
		printf("Trying to access undeclared variable.\n");
		debug_print_token(ident_chain->ident.token, true, true);
		return {};
	}

	Ast_Ident type = bc->var_get_type(ident_chain->ident);
	Type_Info type_info = typer.get_type_info(&type);

	while (true)
	{
		ident_chain = ident_chain->next;
		if (ident_chain == NULL) break;

		if (type_info.tag == TYPE_TAG_STRUCT) //@Perf switch?
		{
			bool found_field = false;

			for (const auto& field : type_info.as_struct_decl->fields)
			{
				if (field.ident.token.string_value == ident_chain->ident.token.string_value)
				{
					found_field = true;
					type = field.type;
					type_info = typer.get_type_info(&type);
					break;
				}
			}

			if (!found_field)
			{
				printf("Trying to access struct field which doesnt exist.\n");
				debug_print_token(ident_chain->ident.token, true, true);
				return {};
			}
		}
		else if (type_info.tag == TYPE_TAG_ENUM)
		{
			printf("Accessing fields of an Enum is NOT YET SUPPORTED.\n"); //@Incomplete
			debug_print_token(ident_chain->ident.token, true, true);
			return {};
		}
		else if (type_info.tag == TYPE_TAG_PRIMITIVE) //@Assuming that primitive types dont have any accesible fields within it
		{
			printf("Trying to access a field of a primitive type.\n");
			debug_print_token(ident_chain->ident.token, true, true);
			return {};
		}
	}

	return type_info;
}

std::optional<Type_Info> Checker::check_expr(Ast_Expr* expr, Block_Checker* bc)
{
	Type_Info expr_type = {};

	switch (expr->tag)
	{
		case Ast_Expr::Tag::Term:
		{
			Ast_Term* term = expr->as_term;
			switch (term->tag)
			{
				case Ast_Term::Tag::Literal:
				{
					Ast_Literal lit = term->as_literal;

					if (lit.token.type == TOKEN_NUMBER)
					{
						//@Incomplete only supporting integer token literals, defaulting to i32
						expr_type = typer.get_primitive_type_info(TYPE_I32);
					}
					else if (lit.token.type == TOKEN_STRING)
					{
						//@Incomplete not supporting string literals yet
						expr_type = typer.get_primitive_type_info(TYPE_STRING);
					}
					else if (lit.token.type == TOKEN_BOOL_LITERAL)
					{
						expr_type = typer.get_primitive_type_info(TYPE_BOOL);
					}
				} break;
				case Ast_Term::Tag::Ident_Chain:
				{
					auto chain_type = check_ident_chain(term->as_ident_chain, bc);
					if (!chain_type) return {};

					expr_type = chain_type.value();

				} break;
				case Ast_Term::Tag::Proc_Call:
				{
					bool is_valid = false;
					auto return_type = check_proc_call(term->as_proc_call, bc, is_valid);
					if (!is_valid) return {};
					
					if (!return_type)
					{
						printf("Called procedure as part of the expression must have a return type.\n");
						return {};
					}

					expr_type = return_type.value();

				} break;
			}
		} break;
		case Ast_Expr::Tag::Unary_Expr:
		{
			Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
			auto rhs_type = check_expr(unary_expr->right, bc);
			if (!rhs_type) return {};
			Type_Info type = rhs_type.value();
			UnaryOp op = unary_expr->op;

			switch (op)
			{
				case UNARY_OP_MINUS:
				{
					if (type.is_user_defined()) { printf("Can not apply unary op '-' to user defined type.\n"); return {}; }
					if (type.is_uint()) { printf("Can not apply unary op '-' to unsigned int type.\n"); return {}; }
				} break;
				case UNARY_OP_LOGIC_NOT:
				{
					if (type.is_user_defined()) { printf("Can not apply unary op '!' to user defined type.\n"); return {}; }
					if (!type.is_bool()) { printf("Unary op '!' can only be applied to [bool] expressions.\n"); return {}; }
				} break;
				case UNARY_OP_BITWISE_NOT:
				{
					if (type.is_user_defined()) { printf("Can not apply unary op '~' to user defined type.\n"); return {}; }
					if (!type.is_uint()) { printf("Unary op '!' can only be applied to [unsigned int] expressions.\n"); return {}; }
				} break;
			}

			expr_type = type;

		} break;
		case Ast_Expr::Tag::Binary_Expr:
		{
			Ast_Binary_Expr* bin_expr = expr->as_binary_expr;
			auto lhs_type = check_expr(bin_expr->left, bc);
			if (!lhs_type) return {};
			auto rhs_type = check_expr(bin_expr->left, bc);
			if (!rhs_type) return {};
			BinaryOp op = bin_expr->op;

			Type_Info type_l = lhs_type.value();
			Type_Info type_r = rhs_type.value();
			
			if (type_l.is_user_defined() || type_r.is_user_defined()) { printf("Can not apply binary op to 1 or 2 user defined types.\n"); return {}; }
			if (!typer.is_type_equals_type(&type_l, &type_r)) { printf("Binary expr type missmatch.\n"); return {}; }

			//@Incomplete binary op specific restrictions
			//return bool if comparison op

			expr_type = type_l;

		} break;
	}

	return expr_type;
}
*/
