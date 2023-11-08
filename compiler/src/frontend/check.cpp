#include "check.h"

#include "error_handler.h"
#include "check_type.h"

//@Todo check cant import same file under multiple names
//@Todo check cant use same type or procedure under multiple names

bool check_program(Ast_Program* program)
{
	Check_Context cc = {};

	//1. check global symbols
	bool main_found = false;
	for (Ast* ast : program->modules)
	{
		check_context_init(&cc, ast, program);
		check_decl_uniqueness(&cc);
		if (ast->filepath == "main")
		{
			main_found = true;
			check_main_proc(&cc);
		}
	}
	if (!main_found) err_report(Error::MAIN_FILE_NOT_FOUND);

	//2. check decls
	for (Ast* ast : program->modules)
	{
		check_context_init(&cc, ast, program);
		check_decls(&cc);
	}
	//if (err.has_err || err_get_status()) return false;

	//3. check struct self storage
	check_context_init(&cc, NULL, program);
	check_perform_struct_sizing(&cc);
	//if (err.has_err || err_get_status()) return false;

	//4. check proc blocks
	for (Ast* ast : program->modules)
	{
		check_context_init(&cc, ast, program);
		check_ast(&cc);
	}

	return !err_get_status();
}

void check_decl_uniqueness(Check_Context* cc)
{
	Ast* ast = cc->ast;
	ast->import_table.init(64);
	ast->struct_table.init(64);
	ast->enum_table.init(64);
	ast->proc_table.init(64);
	ast->global_table.init(64);

	HashSet<Ast_Ident, u32, match_ident> symbol_table(256);
	Ast_Program* program = cc->program;
	
	for (Ast_Import_Decl* decl : ast->imports)
	{
		Ast_Ident ident = decl->alias;
		option<Ast_Ident> symbol = symbol_table.find_key(ident, hash_ident(ident));
		if (symbol)
		{ 
			err_report(Error::DECL_SYMBOL_ALREADY_DECLARED); 
			err_context(cc, ident.span);
			err_context(cc, symbol.value().span);
			continue;
		}
		symbol_table.add(ident, hash_ident(ident));
		ast->import_table.add(ident, decl, hash_ident(ident));
		
		char* path = decl->file_path.token.string_literal_value;
		option<Ast*> import_ast = cc->program->module_map.find(std::string(path), hash_fnv1a_32(string_view_from_string(std::string(path))));
		if (!import_ast)
		{
			err_report(Error::DECL_IMPORT_PATH_NOT_FOUND);
			err_context(cc, decl->file_path.token.span);
			continue;
		}
		decl->import_ast = import_ast.value();
	}

	for (Ast_Use_Decl* decl : ast->uses)
	{
		Ast_Ident ident = decl->alias;
		option<Ast_Ident> symbol = symbol_table.find_key(ident, hash_ident(ident));
		if (symbol)
		{
			err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
			err_context(cc, ident.span);
			err_context(cc, symbol.value().span);
			continue;
		}
		symbol_table.add(ident, hash_ident(ident));
	}

	for (Ast_Struct_Decl* decl : ast->structs)
	{
		Ast_Ident ident = decl->ident;
		option<Ast_Ident> symbol = symbol_table.find_key(ident, hash_ident(ident));
		if (symbol)
		{
			err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
			err_context(cc, ident.span);
			err_context(cc, symbol.value().span);
			continue;
		}
		symbol_table.add(ident, hash_ident(ident));
		ast->struct_table.add(ident, Ast_Struct_Info { (u32)program->structs.size(), decl }, hash_ident(ident));
		program->structs.emplace_back(Ast_Struct_IR_Info { cc->ast, decl });
	}

	for (Ast_Enum_Decl* decl : ast->enums)
	{
		Ast_Ident ident = decl->ident;
		option<Ast_Ident> symbol = symbol_table.find_key(ident, hash_ident(ident));
		if (symbol)
		{
			err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
			err_context(cc, ident.span);
			err_context(cc, symbol.value().span);
			continue;
		}
		symbol_table.add(ident, hash_ident(ident));
		ast->enum_table.add(ident, Ast_Enum_Info { (u32)program->enums.size(), decl }, hash_ident(ident));
		program->enums.emplace_back(Ast_Enum_IR_Info { decl });
	}

	for (Ast_Proc_Decl* decl : ast->procs)
	{
		Ast_Ident ident = decl->ident;
		option<Ast_Ident> symbol = symbol_table.find_key(ident, hash_ident(ident));
		if (symbol)
		{
			err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
			err_context(cc, ident.span);
			err_context(cc, symbol.value().span);
			continue;
		}
		symbol_table.add(ident, hash_ident(ident));
		ast->proc_table.add(ident, Ast_Proc_Info { (u32)program->procs.size(), decl }, hash_ident(ident));
		program->procs.emplace_back(Ast_Proc_IR_Info { decl });
	}

	for (Ast_Global_Decl* decl : ast->globals)
	{
		Ast_Ident ident = decl->ident;
		option<Ast_Ident> symbol = symbol_table.find_key(ident, hash_ident(ident));
		if (symbol)
		{
			err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
			err_context(cc, ident.span);
			err_context(cc, symbol.value().span);
			continue;
		}
		symbol_table.add(ident, hash_ident(ident));
		ast->global_table.add(ident, Ast_Global_Info { (u32)program->globals.size(), decl }, hash_ident(ident));
		program->globals.emplace_back(Ast_Global_IR_Info { decl });
	}
}

void check_decls(Check_Context* cc)
{
	Ast* ast = cc->ast;

	for (Ast_Use_Decl* use_decl : ast->uses)
	{
		Ast* import_ast = resolve_import(cc, { use_decl->import });
		if (import_ast == NULL) continue;

		Ast_Ident alias = use_decl->alias;
		Ast_Ident symbol = use_decl->symbol;
		option<Ast_Struct_Info> struct_info = import_ast->struct_table.find(symbol, hash_ident(symbol));
		if (struct_info) { ast->struct_table.add(alias, struct_info.value(), hash_ident(alias)); continue; }
		option<Ast_Enum_Info> enum_info = import_ast->enum_table.find(symbol, hash_ident(symbol));
		if (enum_info) { ast->enum_table.add(alias, enum_info.value(), hash_ident(alias)); continue; }
		option<Ast_Proc_Info> proc_info = import_ast->proc_table.find(symbol, hash_ident(symbol));
		if (proc_info) { ast->proc_table.add(alias, proc_info.value(), hash_ident(alias)); continue; }
		option<Ast_Global_Info> global_info = import_ast->global_table.find(symbol, hash_ident(symbol));
		if (global_info) { ast->global_table.add(alias, global_info.value(), hash_ident(alias)); continue; }

		err_report(Error::DECL_USE_SYMBOL_NOT_FOUND);
		err_context(cc, symbol.span);
	}
	
	HashSet<Ast_Ident, u32, match_ident> name_set(32);

	for (Ast_Struct_Decl* struct_decl : ast->structs)
	{
		if (!struct_decl->fields.empty()) name_set.zero_reset();
		
		for (u32 i = 0; i < (u32)struct_decl->fields.size();)
		{
			Ast_Struct_Field field = struct_decl->fields[i];
			option<Ast_Ident> name = name_set.find_key(field.ident, hash_ident(field.ident));
			if (name)
			{
				err_report(Error::DECL_STRUCT_DUPLICATE_FIELD);
				err_context(cc, name.value().span);
				struct_decl->fields.erase(struct_decl->fields.begin() + i);
			}
			else i += 1;
		}
	}

	for (Ast_Global_Decl* global_decl : ast->globals)
	{
		//@New pipeline
		global_decl->type = check_consteval_expr(cc, consteval_dependency_from_global(global_decl));
	}
	
	for (Ast_Struct_Decl* struct_decl : ast->structs)
	{
		check_consteval_expr(cc, consteval_dependency_from_struct_size(struct_decl));
	}

	//also remove from vector if duplicate?
	for (Ast_Enum_Decl* enum_decl : ast->enums)
	{
		if (!enum_decl->variants.empty()) name_set.zero_reset();
		else
		{
			err_report(Error::DECL_ENUM_ZERO_VARIANTS);
			err_context(cc, enum_decl->ident.span);
			continue;
		}

		BasicType type = enum_decl->basic_type;
		Ast_Type enum_type = type_from_basic(type);

		if (!basic_type_is_integer(type))
		{
			err_report(Error::DECL_ENUM_NON_INTEGER_TYPE);
			err_context(cc, enum_decl->ident.span);
			continue;
		}

		for (Ast_Enum_Variant& variant : enum_decl->variants)
		{
			option<Ast_Ident> name = name_set.find_key(variant.ident, hash_ident(variant.ident));
			if (name) 
			{
				err_report(Error::DECL_ENUM_DUPLICATE_VARIANT);
				err_context(cc, name.value().span);
			}
			else 
			{
				name_set.add(variant.ident, hash_ident(variant.ident));

				//@New pipeline
				check_consteval_expr(cc, consteval_dependency_from_enum_variant(&variant));
			}
		}
	}

	for (Ast_Struct_Decl* struct_decl : ast->structs)
	{
		for (Ast_Struct_Field& field : struct_decl->fields)
		{
			if (field.default_expr)
			{
				check_expr_type(cc, field.default_expr.value(), field.type, Expr_Constness::Const);
			}
		}
	}

	//also remove from vector if duplicate?
	for (Ast_Proc_Decl* proc_decl : ast->procs)
	{
		if (!proc_decl->input_params.empty()) name_set.zero_reset();

		for (Ast_Proc_Param& param : proc_decl->input_params)
		{
			resolve_type(cc, &param.type);
			
			option<Ast_Ident> name = name_set.find_key(param.ident, hash_ident(param.ident));
			if (name) 
			{
				err_report(Error::DECL_PROC_DUPLICATE_PARAM);
				err_context(cc, name.value().span);
			}
			else name_set.add(param.ident, hash_ident(param.ident));
		}

		if (proc_decl->return_type)
		{
			resolve_type(cc, &proc_decl->return_type.value());
		}
	}
}

void check_main_proc(Check_Context* cc)
{
	option<Ast_Proc_Info> proc_meta = find_proc(cc->ast, Ast_Ident { 0, 0, { (u8*)"main", 4} });
	if (!proc_meta) { err_report(Error::MAIN_PROC_NOT_FOUND); err_context(cc); return; }
	Ast_Proc_Decl* proc_decl = proc_meta.value().proc_decl;
	proc_decl->is_main = true;
	if (proc_decl->is_external) { err_report(Error::MAIN_PROC_EXTERNAL); /*@Error add context*/ }
	if (proc_decl->is_variadic) { err_report(Error::MAIN_PROC_VARIADIC); /*@Error add context*/ }
	if (proc_decl->input_params.size() != 0) { err_report(Error::MAIN_NOT_ZERO_PARAMS); /*@Error add context*/ }
	if (!proc_decl->return_type) { err_report(Error::MAIN_PROC_NO_RETURN_TYPE); /*@Error add context*/ return; }
	if (!type_match(proc_decl->return_type.value(), type_from_basic(BasicType::I32))) { err_report(Error::MAIN_PROC_WRONG_RETURN_TYPE); /*@Error add context*/ }
}
#include "debug_printer.h" //@Temp
void check_perform_struct_sizing(Check_Context* cc)
{
	std::vector<u32> visited_ids;
	std::vector<Ast_Ident> field_chain;

	for (u32 i = 0; i < cc->program->structs.size(); i += 1)
	{
		visited_ids.clear();
		field_chain.clear();
		Ast_Struct_Decl* in_struct = cc->program->structs[i].struct_decl;
		bool is_infinite = check_struct_self_storage(cc, in_struct, i, visited_ids, field_chain);
		if (is_infinite)
		{
			in_struct->size_eval = Consteval::Invalid;
			cc->ast = cc->program->structs[i].from_ast;
			err_report(Error::DECL_STRUCT_SELF_STORAGE);
			err_context(cc, in_struct->ident.span);
			printf("Field access path: ");
			debug_print_ident(field_chain[field_chain.size() - 1], false, false);
			for (int k = (int)field_chain.size() - 2; k >= 0; k -= 1)
			{
				printf(".");
				debug_print_ident(field_chain[k], false, false);
			}
			printf("\n");
		}
		else
		{
			//@Not doing struct sizing only self storage detection
			//check_struct_size(&cc->program->structs[i]);
		}
	}

	//@Todo sizing on valid structs
	//@Notice struct sizing must be done in earlier stages
	//to allow for correct const folding and range checking on sizeof expressions
}

//@Incomplete rework in context of check_perform_struct_sizing to only check for self storage and set const size_eval flag on struct decl
bool check_struct_self_storage(Check_Context* cc, Ast_Struct_Decl* in_struct, u32 struct_id, std::vector<u32>& visited_ids, std::vector<Ast_Ident>& field_chain)
{
	for (Ast_Struct_Field& field : in_struct->fields)
	{
		option<Ast_Struct_Type> struct_type = type_extract_struct_value_type(field.type);
		if (!struct_type) continue;
		if (struct_type.value().struct_id == struct_id) { field_chain.emplace_back(field.ident); return true; }
		
		bool already_visited = false;
		for (u32 id : visited_ids) if (struct_type.value().struct_id == id) { already_visited = true; break; }
		if (already_visited) continue;
		
		visited_ids.emplace_back(struct_type.value().struct_id);
		bool is_infinite = check_struct_self_storage(cc, struct_type.value().struct_decl, struct_id, visited_ids, field_chain);
		if (is_infinite) { field_chain.emplace_back(field.ident); return true; }
	}

	return false;
}

void check_ast(Check_Context* cc)
{
	for (Ast_Proc_Decl* proc_decl : cc->ast->procs)
	{
		check_proc_block(cc, proc_decl);
	}
}

void check_proc_block(Check_Context* cc, Ast_Proc_Decl* proc_decl)
{
	if (proc_decl->is_external) return;

	//@Notice this doesnt correctly handle if else on top level, which may allow all paths to return
	//const exprs arent considered
	Terminator terminator = check_cfg_block(cc, proc_decl->block, false, false);
	if (terminator != Terminator::Return && proc_decl->return_type) err_report(Error::CFG_NOT_ALL_PATHS_RETURN);

	check_context_block_reset(cc, proc_decl);
	check_context_block_add(cc);
	for (Ast_Proc_Param& param : proc_decl->input_params)
	{
		option<Ast_Global_Info> global_info = find_global(cc->ast, param.ident);
		if (global_info)
		{
			err_report(Error::VAR_DECL_ALREADY_IS_GLOBAL);
			err_context(cc, param.ident.span);
		}
		else
		{
			//@Notice this is checked in proc_decl but might be usefull for err recovery
			if (!check_context_block_contains_var(cc, param.ident))
				check_context_block_add_var(cc, param.ident, param.type);
		}
	}
	check_statement_block(cc, proc_decl->block, Checker_Block_Flags::Already_Added);
}

bool basic_type_is_integer(BasicType type)
{
	switch (type)
	{
	case BasicType::BOOL:
	case BasicType::F32:
	case BasicType::F64:
	case BasicType::STRING:
		return false;
	default: return true;
	}
}

Terminator check_cfg_block(Check_Context* cc, Ast_Block* block, bool is_loop, bool is_defer)
{
	Terminator terminator = Terminator::None;

	for (Ast_Statement* statement : block->statements)
	{
		if (terminator != Terminator::None)
		{
			err_report(Error::CFG_UNREACHABLE_STATEMENT);
			err_context(cc, statement->as_if->span); //@Hack
			break;
		}

		switch (statement->tag)
		{
		case Ast_Statement_Tag::If:
		{
			check_cfg_if(cc, statement->as_if, is_loop, is_defer);
		} break;
		case Ast_Statement_Tag::For: 
		{
			check_cfg_block(cc, statement->as_for->block, true, is_defer);
		} break;
		case Ast_Statement_Tag::Block: 
		{
			terminator = check_cfg_block(cc, statement->as_block, is_loop, is_defer);
		} break;
		case Ast_Statement_Tag::Defer:
		{
			if (is_defer) { err_report(Error::CFG_NESTED_DEFER); err_context(cc, statement->as_defer->span); }
			else check_cfg_block(cc, statement->as_defer->block, false, true);
		} break;
		case Ast_Statement_Tag::Break:
		{
			if (!is_loop)
			{
				if (is_defer) { err_report(Error::CFG_BREAK_INSIDE_DEFER); err_context(cc, statement->as_break->span); }
				else { err_report(Error::CFG_BREAK_OUTSIDE_LOOP); err_context(cc, statement->as_break->span); }
			}
			else terminator = Terminator::Break;
		} break;
		case Ast_Statement_Tag::Return:
		{
			if (is_defer) { err_report(Error::CFG_RETURN_INSIDE_DEFER); err_context(cc, statement->as_return->span); }
			else terminator = Terminator::Return;
		} break;
		case Ast_Statement_Tag::Switch:
		{
			check_cfg_switch(cc, statement->as_switch, is_loop, is_defer);
		} break;
		case Ast_Statement_Tag::Continue:
		{
			if (!is_loop)
			{
				if (is_defer) { err_report(Error::CFG_CONTINUE_INSIDE_DEFER); err_context(cc, statement->as_continue->span); }
				else { err_report(Error::CFG_CONTINUE_OUTSIDE_LOOP); err_context(cc, statement->as_continue->span); }
			}
			else terminator = Terminator::Continue;
		} break;
		case Ast_Statement_Tag::Var_Decl: break;
		case Ast_Statement_Tag::Var_Assign: break;
		case Ast_Statement_Tag::Proc_Call: break;
		}
	}

	return terminator;
}

void check_cfg_if(Check_Context* cc, Ast_If* _if, bool is_loop, bool is_defer)
{
	check_cfg_block(cc, _if->block, is_loop, is_defer);
	
	if (_if->_else)
	{
		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else_Tag::If)
			check_cfg_if(cc, _else->as_if, is_loop, is_defer);
		else check_cfg_block(cc, _else->as_block, is_loop, is_defer);
	}
}

void check_cfg_switch(Check_Context* cc, Ast_Switch* _switch, bool is_loop, bool is_defer)
{
	for (Ast_Switch_Case& _case : _switch->cases)
	{
		if (_case.block) check_cfg_block(cc, _case.block.value(), is_loop, is_defer);
	}
}

static void check_statement_block(Check_Context* cc, Ast_Block* block, Checker_Block_Flags flags)
{
	if (flags != Checker_Block_Flags::Already_Added) check_context_block_add(cc);

	for (Ast_Statement* statement: block->statements)
	{
		switch (statement->tag)
		{
		case Ast_Statement_Tag::If: check_statement_if(cc, statement->as_if); break;
		case Ast_Statement_Tag::For: check_statement_for(cc, statement->as_for); break;
		case Ast_Statement_Tag::Block: check_statement_block(cc, statement->as_block, Checker_Block_Flags::None); break;
		case Ast_Statement_Tag::Defer: check_statement_block(cc, statement->as_defer->block, Checker_Block_Flags::None); break;
		case Ast_Statement_Tag::Break: break;
		case Ast_Statement_Tag::Return: check_statement_return(cc, statement->as_return); break;
		case Ast_Statement_Tag::Switch: check_statement_switch(cc, statement->as_switch); break;
		case Ast_Statement_Tag::Continue: break;
		case Ast_Statement_Tag::Proc_Call: check_proc_call(cc, statement->as_proc_call, Checker_Proc_Call_Flags::In_Statement); break;
		case Ast_Statement_Tag::Var_Decl: check_statement_var_decl(cc, statement->as_var_decl); break;
		case Ast_Statement_Tag::Var_Assign: check_statement_var_assign(cc, statement->as_var_assign); break;
		}
	}

	check_context_block_pop_back(cc);
}

void check_statement_if(Check_Context* cc, Ast_If* _if)
{
	check_expr_type(cc, _if->condition_expr, type_from_basic(BasicType::BOOL), Expr_Constness::Normal);
	check_statement_block(cc, _if->block, Checker_Block_Flags::None);

	if (_if->_else)
	{
		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else_Tag::If) check_statement_if(cc, _else->as_if);
		else check_statement_block(cc, _else->as_block, Checker_Block_Flags::None);
	}
}

void check_statement_for(Check_Context* cc, Ast_For* _for)
{
	check_context_block_add(cc);
	if (_for->var_decl) check_statement_var_decl(cc, _for->var_decl.value());
	if (_for->var_assign) check_statement_var_assign(cc, _for->var_assign.value());
	if (_for->condition_expr) check_expr_type(cc, _for->condition_expr.value(), type_from_basic(BasicType::BOOL), Expr_Constness::Normal);
	check_statement_block(cc, _for->block, Checker_Block_Flags::Already_Added);
}

void check_statement_return(Check_Context* cc, Ast_Return* _return)
{
	Ast_Proc_Decl* curr_proc = cc->curr_proc;

	if (_return->expr)
	{
		if (curr_proc->return_type)
		{
			check_expr_type(cc, _return->expr.value(), curr_proc->return_type, Expr_Constness::Normal);
		}
		else
		{
			err_report(Error::RETURN_EXPECTED_NO_EXPR);
			err_context(cc, _return->span);
		}
	}
	else
	{
		if (curr_proc->return_type)
		{
			err_report(Error::RETURN_EXPECTED_EXPR);
			err_context(cc, curr_proc->return_type.value().span);
			err_context(cc, _return->span);
		}
	}
}

void check_statement_switch(Check_Context* cc, Ast_Switch* _switch)
{
	//@Very unfinished. Share const expr unique pool logic with EnumVariants
	
	//@Check if switch is exaustive with enums / integers, require
	//add default or discard like syntax _ for default case
	//@Check matching switched on => case expr type
	//@Todo check case constant value overlap
	//@Todo check that cases fall into block, and theres no cases that dont do anything
	
	for (Ast_Switch_Case& _case : _switch->cases)
	{
		if (_case.block)
		{
			check_statement_block(cc, _case.block.value(), Checker_Block_Flags::None);
		}
	}

	option<Ast_Type> type = check_expr_type(cc, _switch->expr, {}, Expr_Constness::Normal);
	if (!type) return;

	Type_Kind kind = type_kind(type.value());
	if (kind != Type_Kind::Integer && kind != Type_Kind::Enum)
	{
		err_report(Error::SWITCH_INCORRECT_EXPR_TYPE);
		//@Context add expr type
		err_context(cc, _switch->expr->span);
	}

	if (_switch->cases.empty())
	{
		err_report(Error::SWITCH_ZERO_CASES);
		err_context(cc, _switch->span);
		return;
	}

	for (Ast_Switch_Case& _case : _switch->cases)
	{
		check_expr_type(cc, _case.case_expr, type.value(), Expr_Constness::Const);
	}
}

void check_statement_var_decl(Check_Context* cc, Ast_Var_Decl* var_decl)
{
	Ast_Ident ident = var_decl->ident;

	option<Ast_Global_Info> global_info = find_global(cc->ast, ident);
	if (global_info)
	{
		err_report(Error::VAR_DECL_ALREADY_IS_GLOBAL);
		err_context(cc, var_decl->span);
		//@Todo global decl context
		return;
	}

	if (check_context_block_contains_var(cc, ident))
	{
		err_report(Error::VAR_DECL_ALREADY_IN_SCOPE);
		err_context(cc, var_decl->span);
		//@Todo context of prev declaration
		return;
	}

	if (var_decl->type)
	{
		resolve_type(cc, &var_decl->type.value());
		if (type_is_poison(var_decl->type.value())) return;

		if (var_decl->expr)
		{
			option<Ast_Type> expr_type = check_expr_type(cc, var_decl->expr.value(), var_decl->type.value(), Expr_Constness::Normal);
		}
		
		check_context_block_add_var(cc, ident, var_decl->type.value());
	}
	else
	{
		// @Errors this might produce "var not found" error in later checks, might be solved by flagging
		// not adding var to the stack, when inferred type is not valid
		option<Ast_Type> expr_type = check_expr_type(cc, var_decl->expr.value(), {}, Expr_Constness::Normal);
		if (expr_type)
		{
			var_decl->type = expr_type.value();
			check_context_block_add_var(cc, ident, expr_type.value());
		}
	}
}

void check_statement_var_assign(Check_Context* cc, Ast_Var_Assign* var_assign)
{
	option<Ast_Type> var_type = check_var(cc, var_assign->var);
	if (!var_type) return;

	if (var_assign->op != AssignOp::NONE) //@Temp error, implement var asssign ops
	{
		err_report(Error::TEMP_VAR_ASSIGN_OP);
		err_context(cc, var_assign->var->local.ident.span);
		return;
	}

	check_expr_type(cc, var_assign->expr, var_type.value(), Expr_Constness::Normal);
}
