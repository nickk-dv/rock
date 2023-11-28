export module check;

import general;
import ast;
import err_handler;
import check_context;
//#include "check_type.h" @later

export bool check_program(Ast_Program* program);

void check_main_entry_point(Ast_Program* program);
void check_decls_symbols(Check_Context* cc);
void check_decls_consteval(Check_Context* cc);
void check_decls_finalize(Check_Context* cc);
void check_proc_blocks(Check_Context* cc);

Terminator check_cfg_block(Check_Context* cc, Ast_Stmt_Block* block, bool is_loop, bool is_defer);
void check_cfg_if(Check_Context* cc, Ast_Stmt_If* _if, bool is_loop, bool is_defer);
void check_cfg_switch(Check_Context* cc, Ast_Stmt_Switch* _switch, bool is_loop, bool is_defer);

void check_stmt_block(Check_Context* cc, Ast_Stmt_Block* block, Checker_Block_Flags flags);
void check_stmt_if(Check_Context* cc, Ast_Stmt_If* _if);
void check_stmt_for(Check_Context* cc, Ast_Stmt_For* _for);
void check_stmt_return(Check_Context* cc, Ast_Stmt_Return* _return);
void check_stmt_switch(Check_Context* cc, Ast_Stmt_Switch* _switch);
void check_stmt_var_decl(Check_Context* cc, Ast_Stmt_Var_Decl* var_decl);
void check_stmt_var_assign(Check_Context* cc, Ast_Stmt_Var_Assign* var_assign);

module : private;

struct Module_Scope
{
	Ast_Program* program;
	Ast* ast;
	HashMap<Ast_Ident, Ast_Decl*> decl_map;
};

void module_scope_try_add_symbol(Module_Scope* module_scope, Ast_Decl* decl)
{
	Ast_Ident ident;
	switch (decl->tag())
	{
	case Ast_Decl::Tag::Proc: ident = decl->as_proc->ident; break;
	case Ast_Decl::Tag::Enum: ident = decl->as_enum->ident; break;
	case Ast_Decl::Tag::Struct: ident = decl->as_struct->ident; break;
	case Ast_Decl::Tag::Global: ident = decl->as_global->ident; break;
	default: return;
	}

	option<Ast_Decl*> duplicate = module_scope->decl_map.find(ident);
	if (duplicate)
	{
		err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
	}
	else module_scope->decl_map.add(ident, decl);
}

void module_scope_populate_decl_map(Module_Scope* module_scope)
{
	for (Ast_Decl* decl : module_scope->ast->decls)
	{
		module_scope_try_add_symbol(module_scope, decl);
	}
}

option<Ast_Module*> module_list_traverse_from_root(const std::vector<Ast_Ident>& modules)
{
	for (const Ast_Ident& module : modules)
	{

	}
	
	return {};
}

void module_scope_process_imports(Module_Scope* module_scope)
{
	for (Ast_Decl* decl : module_scope->ast->decls)
	{
		if (decl->tag() != Ast_Decl::Tag::Import) continue;

		Ast_Decl_Import* import_decl = decl->as_import;
		
		//check modules chain to be valid

		if (!import_decl->target) continue;
		Ast_Import_Target* import_target = import_decl->target.value();

		switch (import_target->tag())
		{
		case Ast_Import_Target::Tag::Wildcard:
		{
			//must be a file module
			//add all symbols, with duplicate checks
		} break;
		case Ast_Import_Target::Tag::Symbol_List:
		{
			//must be a file module
			//add all symbols, with duplicate checks
		} break;
		case Ast_Import_Target::Tag::Symbol_Or_Module:
		{
			//determine is it module or symbol
			//symbol: add, with duplicate checks
		} break;
		}
	}
}

void module_scope_process_impls(Module_Scope* module_scope)
{
	for (Ast_Decl* decl : module_scope->ast->decls)
	{
		if (decl->tag() != Ast_Decl::Tag::Impl) continue;

		//@todo
	}
}

bool check_program(Ast_Program* program)
{
	Arena arena = {};
	arena.init(1024 * 1024); //@idk size

	std::vector<Module_Scope*> module_scopes = {};
	for (Ast* ast : program->modules)
	{
		Module_Scope* module_scope = arena.alloc<Module_Scope>();
		module_scope->ast = ast;
		module_scope->decl_map.init(256);
		module_scopes.emplace_back(module_scope);
	}

	for (Module_Scope* module_scope : module_scopes) module_scope_populate_decl_map(module_scope);
	for (Module_Scope* module_scope : module_scopes) module_scope_process_imports(module_scope);
	for (Module_Scope* module_scope : module_scopes) module_scope_process_impls(module_scope);
	
	//Check_Context cc = {};
	//program->external_proc_table.init(256);

	//check_module_tree(&program->root);
	//
	//for (Ast* ast : program->modules)
	//{
	//
	//	//check_context_init(&cc, ast, program);
	//	//check_decls_symbols(&cc);
	//}
	//
	//check_main_entry_point(program);
	//
	//for (Ast* ast : program->modules)
	//{
	//	check_context_init(&cc, ast, program);
	//	//@disabled check_decls_consteval(&cc);
	//}
	//
	//for (Ast* ast : program->modules)
	//{
	//	check_context_init(&cc, ast, program);
	//	//@disabled check_decls_finalize(&cc);
	//}
	//
	//for (Ast* ast : program->modules)
	//{
	//	check_context_init(&cc, ast, program);
	//	//@disabled check_proc_blocks(&cc);
	//}
	//
	//return !err_get_status();

	return !err_get_status();
}

void check_main_entry_point(Ast_Program* program)
{
	Ast* main_ast = NULL;

	for (Ast* ast : program->modules)
	{
		//if (ast->filepath == "main") main_ast = ast;
	}

	if (main_ast == NULL) 
	{
		err_report(Error::MAIN_FILE_NOT_FOUND);
		return;
	}

	Check_Context cc = {};
	check_context_init(&cc, main_ast, program);

	/* @disabled
	option<Ast_Info_Proc> proc_meta = find_proc(cc.ast, Ast_Ident{ 0, 0, { (u8*)"main", 4} });
	if (!proc_meta) { err_report(Error::MAIN_PROC_NOT_FOUND); err_context(&cc); return; }
	Ast_Decl_Proc* proc_decl = proc_meta.value().proc_decl;
	proc_decl->is_main = true;
	if (proc_decl->is_external) { err_report(Error::MAIN_PROC_EXTERNAL); err_context(&cc, proc_decl->ident.span); }
	if (proc_decl->is_variadic) { err_report(Error::MAIN_PROC_VARIADIC); err_context(&cc, proc_decl->ident.span); }
	if (proc_decl->input_params.size() != 0) { err_report(Error::MAIN_NOT_ZERO_PARAMS); err_context(&cc, proc_decl->ident.span); }
	if (!proc_decl->return_type) { err_report(Error::MAIN_PROC_NO_RETURN_TYPE); err_context(&cc, proc_decl->ident.span); return; }
	if (!type_match(proc_decl->return_type.value(), type_from_basic(BasicType::I32))) { err_report(Error::MAIN_PROC_WRONG_RETURN_TYPE); err_context(&cc, proc_decl->ident.span); }
	*/
}

void check_decls_symbols(Check_Context* cc)
{/*
	Ast* ast = cc->ast;
	ast->import_table.init(64);
	ast->struct_table.init(64);
	ast->enum_table.init(64);
	ast->proc_table.init(64);
	ast->global_table.init(64);

	HashSet<Ast_Ident, u32, match_ident> symbol_table(256);
	Ast_Program* program = cc->program;

	for (Ast_Decl_Import* decl : ast->imports)
	{
		check_decl_import(cc, decl);
	}

	for (Ast_Decl_Struct* decl : ast->structs)
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
		ast->struct_table.add(ident, Ast_Info_Struct { decl, (u32)program->structs.size() }, hash_ident(ident));
		program->structs.emplace_back(Ast_Info_IR_Struct { decl });
	}

	for (Ast_Decl_Enum* decl : ast->enums)
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
		ast->enum_table.add(ident, Ast_Info_Enum { decl, (u32)program->enums.size() }, hash_ident(ident));
		program->enums.emplace_back(Ast_Info_IR_Enum { decl });
	}

	for (Ast_Decl_Proc* decl : ast->procs)
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
		ast->proc_table.add(ident, Ast_Info_Proc { decl, (u32)program->procs.size() }, hash_ident(ident));
		program->procs.emplace_back(Ast_Info_IR_Proc { decl });

		if (!decl->is_external) continue;
		option<Ast_Decl_Proc*> external = program->external_proc_table.find(ident, hash_ident(ident));
		if (external)
		{
			err_report(Error::DECL_PROC_DUPLICATE_EXTERNAL);
			err_context(cc, ident.span);
			err_context(cc, external.value()->ident.span); //@Need to reference ast of the declaration, same for many other error contexts
			continue;
		}
		program->external_proc_table.add(ident, decl, hash_ident(ident));
	}

	for (Ast_Decl_Global* decl : ast->globals)
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
		ast->global_table.add(ident, Ast_Info_Global { decl, (u32)program->globals.size() }, hash_ident(ident));
		program->globals.emplace_back(Ast_Info_IR_Global { decl });
	}
*/ }
/* @disabled
void check_decls_consteval(Check_Context* cc)
{
	Ast* ast = cc->ast;
	HashSet<Ast_Ident, u32, match_ident> name_set(64);

	for (Ast_Decl_Struct* struct_decl : ast->structs)
	{
		if (!struct_decl->fields.empty()) name_set.zero_reset();
		
		for (u32 i = 0; i < (u32)struct_decl->fields.size();)
		{
			Ast_Struct_Field field = struct_decl->fields[i];
			option<Ast_Ident> name = name_set.find_key(field.ident, hash_ident(field.ident));
			if (name)
			{
				err_report(Error::DECL_STRUCT_DUPLICATE_FIELD);
				err_context(cc, field.ident.span);
				err_context(cc, name.value().span);
				struct_decl->fields.erase(struct_decl->fields.begin() + i);
			}
			else
			{
				name_set.add(field.ident, hash_ident(field.ident));
				i += 1;
			}
		}
	}

	for (Ast_Decl_Enum* enum_decl : ast->enums)
	{
		if (!enum_decl->variants.empty()) name_set.zero_reset();

		for (u32 i = 0; i < (u32)enum_decl->variants.size();)
		{
			Ast_Enum_Variant variant = enum_decl->variants[i];
			option<Ast_Ident> name = name_set.find_key(variant.ident, hash_ident(variant.ident));
			if (name)
			{
				err_report(Error::DECL_ENUM_DUPLICATE_VARIANT);
				err_context(cc, variant.ident.span);
				err_context(cc, name.value().span);
				enum_decl->variants.erase(enum_decl->variants.begin() + i);
			}
			else
			{
				name_set.add(variant.ident, hash_ident(variant.ident));
				i += 1;
			}
		}
	}

	for (Ast_Decl_Proc* proc_decl : ast->procs)
	{
		if (!proc_decl->input_params.empty()) name_set.zero_reset();

		for (u32 i = 0; i < (u32)proc_decl->input_params.size();)
		{
			Ast_Proc_Param param = proc_decl->input_params[i];
			option<Ast_Ident> name = name_set.find_key(param.ident, hash_ident(param.ident));
			if (name)
			{
				err_report(Error::DECL_PROC_DUPLICATE_PARAM);
				err_context(cc, param.ident.span);
				err_context(cc, name.value().span);
				proc_decl->input_params.erase(proc_decl->input_params.begin() + i);
			}
			else
			{
				name_set.add(param.ident, hash_ident(param.ident));
				i += 1;
			}
		}
	}

	for (Ast_Decl_Struct* struct_decl : ast->structs)
	{
		check_consteval_expr(cc, consteval_dependency_from_struct_size(struct_decl, struct_decl->ident.span));
	}

	for (Ast_Decl_Global* global_decl : ast->globals)
	{
		check_consteval_expr(cc, consteval_dependency_from_global(global_decl, global_decl->ident.span));
	}

	for (Ast_Decl_Enum* enum_decl : ast->enums)
	{
		if (enum_decl->variants.empty())
		{
			err_report(Error::DECL_ENUM_ZERO_VARIANTS);
			err_context(cc, enum_decl->ident.span);
			continue;
		}

		BasicType type = enum_decl->basic_type;
		Type_Kind kind = type_kind(type_from_basic(type));
		
		if (kind != Type_Kind::Int && kind != Type_Kind::Uint) //@Detect this on parse stage to simplify error states
		{
			err_report(Error::DECL_ENUM_NON_INTEGER_TYPE);
			err_context(cc, enum_decl->ident.span);
			continue;
		}

		for (Ast_Enum_Variant& variant : enum_decl->variants)
		{
			check_consteval_expr(cc, consteval_dependency_from_enum_variant(&variant, enum_decl->basic_type, variant.ident.span));
		}
	}
}

void check_decls_finalize(Check_Context* cc)
{
	Ast* ast = cc->ast;

	for (Ast_Decl_Struct* struct_decl : ast->structs)
	{
		for (Ast_Struct_Field& field : struct_decl->fields)
		{
			if (field.default_expr) 
			{
				check_expr_type(cc, field.default_expr.value(), field.type, Expr_Constness::Const);
			}
		}
	}

	for (Ast_Decl_Proc* proc_decl : ast->procs)
	{
		for (Ast_Proc_Param& param : proc_decl->input_params)
		{
			resolve_type(cc, &param.type, true);
		}

		if (proc_decl->return_type)
		{
			resolve_type(cc, &proc_decl->return_type.value(), true);
		}
	}
}

void check_proc_blocks(Check_Context* cc)
{
	for (Ast_Decl_Proc* proc_decl : cc->ast->procs)
	{
		if (proc_decl->is_external) continue;

		//@Notice this doesnt correctly handle if else on top level, which may allow all paths to return
		//const exprs arent considered
		Terminator terminator = check_cfg_block(cc, proc_decl->block, false, false);
		if (terminator != Terminator::Return && proc_decl->return_type)
		{
			err_report(Error::CFG_NOT_ALL_PATHS_RETURN);
			err_context(cc, proc_decl->ident.span);
		}

		check_context_block_reset(cc, proc_decl);
		check_context_block_add(cc);
		for (Ast_Proc_Param& param : proc_decl->input_params)
		{
			option<Ast_Info_Global> global_info = find_global(cc->ast, param.ident);
			if (global_info)
			{
				err_report(Error::VAR_ALREADY_IS_GLOBAL);
				err_context(cc, param.ident.span);
			}
			else check_context_block_add_var(cc, param.ident, param.type);
		}

		check_stmt_block(cc, proc_decl->block, Checker_Block_Flags::Already_Added);
	}
}

Terminator check_cfg_block(Check_Context* cc, Ast_Stmt_Block* block, bool is_loop, bool is_defer)
{
	Terminator terminator = Terminator::None;

	for (Ast_Stmt* statement : block->statements)
	{
		if (terminator != Terminator::None)
		{
			err_report(Error::CFG_UNREACHABLE_STATEMENT);
			err_context(cc, statement->as_if->span); //@Hack
			break;
		}

		switch (statement->tag)
		{
		case Ast_Stmt_Tag::If:
		{
			check_cfg_if(cc, statement->as_if, is_loop, is_defer);
		} break;
		case Ast_Stmt_Tag::For: 
		{
			check_cfg_block(cc, statement->as_for->block, true, is_defer);
		} break;
		case Ast_Stmt_Tag::Block: 
		{
			terminator = check_cfg_block(cc, statement->as_block, is_loop, is_defer);
		} break;
		case Ast_Stmt_Tag::Defer:
		{
			if (is_defer) { err_report(Error::CFG_NESTED_DEFER); err_context(cc, statement->as_defer->span); }
			else check_cfg_block(cc, statement->as_defer->block, false, true);
		} break;
		case Ast_Stmt_Tag::Break:
		{
			if (!is_loop)
			{
				if (is_defer) { err_report(Error::CFG_BREAK_INSIDE_DEFER); err_context(cc, statement->as_break->span); }
				else { err_report(Error::CFG_BREAK_OUTSIDE_LOOP); err_context(cc, statement->as_break->span); }
			}
			else terminator = Terminator::Break;
		} break;
		case Ast_Stmt_Tag::Return:
		{
			if (is_defer) { err_report(Error::CFG_RETURN_INSIDE_DEFER); err_context(cc, statement->as_return->span); }
			else terminator = Terminator::Return;
		} break;
		case Ast_Stmt_Tag::Switch:
		{
			check_cfg_switch(cc, statement->as_switch, is_loop, is_defer);
		} break;
		case Ast_Stmt_Tag::Continue:
		{
			if (!is_loop)
			{
				if (is_defer) { err_report(Error::CFG_CONTINUE_INSIDE_DEFER); err_context(cc, statement->as_continue->span); }
				else { err_report(Error::CFG_CONTINUE_OUTSIDE_LOOP); err_context(cc, statement->as_continue->span); }
			}
			else terminator = Terminator::Continue;
		} break;
		case Ast_Stmt_Tag::Var_Decl: break;
		case Ast_Stmt_Tag::Var_Assign: break;
		case Ast_Stmt_Tag::Proc_Call: break;
		}
	}

	return terminator;
}

void check_cfg_if(Check_Context* cc, Ast_Stmt_If* _if, bool is_loop, bool is_defer)
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

void check_cfg_switch(Check_Context* cc, Ast_Stmt_Switch* _switch, bool is_loop, bool is_defer)
{
	for (Ast_Switch_Case& _case : _switch->cases)
	{
		if (_case.block) check_cfg_block(cc, _case.block.value(), is_loop, is_defer);
	}
}

static void check_stmt_block(Check_Context* cc, Ast_Stmt_Block* block, Checker_Block_Flags flags)
{
	if (flags != Checker_Block_Flags::Already_Added) check_context_block_add(cc);

	for (Ast_Stmt* statement: block->statements)
	{
		switch (statement->tag)
		{
		case Ast_Stmt_Tag::If: check_stmt_if(cc, statement->as_if); break;
		case Ast_Stmt_Tag::For: check_stmt_for(cc, statement->as_for); break;
		case Ast_Stmt_Tag::Block: check_stmt_block(cc, statement->as_block, Checker_Block_Flags::None); break;
		case Ast_Stmt_Tag::Defer: check_stmt_block(cc, statement->as_defer->block, Checker_Block_Flags::None); break;
		case Ast_Stmt_Tag::Break: break;
		case Ast_Stmt_Tag::Return: check_stmt_return(cc, statement->as_return); break;
		case Ast_Stmt_Tag::Switch: check_stmt_switch(cc, statement->as_switch); break;
		case Ast_Stmt_Tag::Continue: break;
		case Ast_Stmt_Tag::Proc_Call: check_proc_call(cc, statement->as_proc_call, Checker_Proc_Call_Flags::In_Statement); break;
		case Ast_Stmt_Tag::Var_Decl: check_stmt_var_decl(cc, statement->as_var_decl); break;
		case Ast_Stmt_Tag::Var_Assign: check_stmt_var_assign(cc, statement->as_var_assign); break;
		}
	}

	check_context_block_pop_back(cc);
}

void check_stmt_if(Check_Context* cc, Ast_Stmt_If* _if)
{
	check_expr_type(cc, _if->condition_expr, type_from_basic(BasicType::BOOL), Expr_Constness::Normal);
	check_stmt_block(cc, _if->block, Checker_Block_Flags::None);

	if (_if->_else)
	{
		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else_Tag::If) check_stmt_if(cc, _else->as_if);
		else check_stmt_block(cc, _else->as_block, Checker_Block_Flags::None);
	}
}

void check_stmt_for(Check_Context* cc, Ast_Stmt_For* _for)
{
	check_context_block_add(cc);
	if (_for->var_decl) check_stmt_var_decl(cc, _for->var_decl.value());
	if (_for->var_assign) check_stmt_var_assign(cc, _for->var_assign.value());
	if (_for->condition_expr) check_expr_type(cc, _for->condition_expr.value(), type_from_basic(BasicType::BOOL), Expr_Constness::Normal);
	check_stmt_block(cc, _for->block, Checker_Block_Flags::Already_Added);
}

void check_stmt_return(Check_Context* cc, Ast_Stmt_Return* _return)
{
	Ast_Decl_Proc* curr_proc = cc->curr_proc;

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

void check_stmt_switch(Check_Context* cc, Ast_Stmt_Switch* _switch)
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
			check_stmt_block(cc, _case.block.value(), Checker_Block_Flags::None);
		}
	}

	option<Ast_Type> type = check_expr_type(cc, _switch->expr, {}, Expr_Constness::Normal);
	if (!type) return;

	Type_Kind kind = type_kind(type.value());
	if (kind != Type_Kind::Int && kind != Type_Kind::Uint && kind != Type_Kind::Enum)
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

void check_stmt_var_decl(Check_Context* cc, Ast_Stmt_Var_Decl* var_decl)
{
	Ast_Ident ident = var_decl->ident;

	option<Ast_Info_Global> global_info = find_global(cc->ast, ident);
	if (global_info)
	{
		err_report(Error::VAR_ALREADY_IS_GLOBAL);
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
		resolve_type(cc, &var_decl->type.value(), true);
		if (type_is_poison(var_decl->type.value())) 
		{
			check_context_block_add_var(cc, ident, type_from_poison());
		}
		else
		{
			check_context_block_add_var(cc, ident, var_decl->type.value());
			if (var_decl->expr) check_expr_type(cc, var_decl->expr.value(), var_decl->type.value(), Expr_Constness::Normal);
		}
	}
	else
	{
		option<Ast_Type> expr_type = check_expr_type(cc, var_decl->expr.value(), {}, Expr_Constness::Normal);
		if (expr_type) 
		{ 
			check_context_block_add_var(cc, ident, expr_type.value()); 
			var_decl->type = expr_type.value(); 
		}
		else check_context_block_add_var(cc, ident, type_from_poison());
	}
}

void check_stmt_var_assign(Check_Context* cc, Ast_Stmt_Var_Assign* var_assign)
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
*/
