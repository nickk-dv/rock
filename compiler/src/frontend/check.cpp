#include "check.h"

#include "error_handler.h"
//#include "check_type.h"

bool check_program(Ast_Program* program)
{
	Check_Context cc = {};
	//program->external_proc_table.init(256);

	check_module_tree(&program->root);

	for (Ast* ast : program->modules)
	{
		check_context_init(&cc, ast, program);
		check_decls_symbols(&cc);
	}
	
	check_main_entry_point(program);

	for (Ast* ast : program->modules)
	{
		check_context_init(&cc, ast, program);
		//@disabled check_decls_consteval(&cc);
	}

	for (Ast* ast : program->modules)
	{
		check_context_init(&cc, ast, program);
		//@disabled check_decls_finalize(&cc);
	}

	for (Ast* ast : program->modules)
	{
		check_context_init(&cc, ast, program);
		//@disabled check_proc_blocks(&cc);
	}

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
void check_module_tree(Ast_Module_Tree* node)
{	
	return; //@remove ruins the data reword the allocation of the tree, and store pointers to the nodes from arena allocator
	bool conflict_found;
	do
	{
		conflict_found = false;
		u32 conflict_index = 0;

		for (u32 i = 0; i < node->submodules.size(); i += 1)
		{
			for (u32 k = 0; k < node->submodules.size(); k += 1)
			{
				if (i == k) continue;
				Ast_Module_Tree* module_a = &node->submodules[i];
				Ast_Module_Tree* module_b = &node->submodules[k];
				if (match_ident(module_a->ident, module_b->ident))
				{
					conflict_found = true;
					if (module_a->leaf_ast && !module_b->leaf_ast) conflict_index = i;
					else conflict_index = k;
					break;
				}
			}
			if (conflict_found) break;
		}

		if (conflict_found)
		{
			err_internal("check_module_tree: conflict modules, only folder or file module can exist in sigle module");
			//err_context(node->submodules[conflict_index].leaf_ast.value()->filepath.c_str());
			node->submodules.erase(node->submodules.begin() + conflict_index);
		}
	}
	while (conflict_found);

	for (u32 i = 0; i < node->submodules.size(); i += 1)
	{
		check_module_tree(&node->submodules[i]);
	}
}

Ast* check_module_access(Check_Context* cc, option<Ast_Module_Access*> module_access)
{
	if (!module_access) return cc->ast;
	return check_module_list(cc, module_access.value()->modules);
}

Ast* check_module_list(Check_Context* cc, std::vector<Ast_Ident>& modules)
{
	if (modules.empty()) return cc->ast;

	Ast_Module_Tree* node = &cc->program->root;
	Ast_Ident node_ident = {};

	for (Ast_Ident& ident : modules)
	{
		bool found = false;
		for (Ast_Module_Tree& module : node->submodules)
		{
			if (match_ident(ident, module.ident))
			{
				found = true;
				node = &module;
				node_ident = ident;
				break;
			}
		}
		if (!found)
		{
			err_internal("check_module_list: module not found");
			err_context(cc, ident.span);
			return NULL;
		}
	}

	//@restructure
	//@this check is valid for module access 
	//@but in import decl the last identifier might be a module, and this check might need to be delayed
	if (!node->leaf_ast)
	{
		err_internal("check_module_list: module path must end with module with associated file");
		err_context(cc, node_ident.span);
		return NULL;
	}

	return node->leaf_ast.value();
}

void check_decl_import(Check_Context* cc, Ast_Decl_Import* import_decl)
{
	Ast* ast = check_module_list(cc, import_decl->modules);
	if (!ast) return;
	if (!import_decl->target) return;

	Ast_Import_Target* target = import_decl->target.value();
	switch (target->tag())
	{
	case Ast_Import_Target::Tag::Wildcard:
	{
		//@todo import wildcard
		//@maybe check for conflicts
	} break;
	case Ast_Import_Target::Tag::Symbol_List:
	{
		//@todo check that symbols exist + import
		//@maybe check for conflicts
	} break;
	case Ast_Import_Target::Tag::Symbol_Or_Module:
	{
		//@todo resolve symbol or module
		//@check module & symbol
		//@maybe check for conflicts
	} break;
	default: err_internal("check_decl_import: invalid Ast_Import_Target_Tag"); break;
	}
}
