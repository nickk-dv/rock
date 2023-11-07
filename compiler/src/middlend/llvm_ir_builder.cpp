#include "llvm_ir_builder.h"

#include "llvm-c/Core.h"

LLVMModuleRef build_module(Ast_Program* program)
{
	IR_Builder_Context bc = {};
	builder_context_init(&bc, program);

	for (Ast_Enum_IR_Info& enum_info : program->enums)
	{
		BasicType basic_type = enum_info.enum_decl->basic_type;
		Type type = type_from_basic_type(basic_type);
		enum_info.enum_type = type;

		for (Ast_Enum_Variant& variant : enum_info.enum_decl->variants)
		{
			variant.constant = build_expr(&bc, variant.consteval_expr->expr);
		}
	}

	std::vector<Type> type_array(32);
	
	for (Ast_Struct_IR_Info& struct_info : program->structs)
	{
		struct_info.struct_type = LLVMStructCreateNamed(LLVMGetGlobalContext(), "struct");
	}

	for (Ast_Struct_IR_Info& struct_info : program->structs)
	{
		type_array.clear();
		Ast_Struct_Decl* struct_decl = struct_info.struct_decl;
		for (Ast_Struct_Field& field : struct_decl->fields) 
		type_array.emplace_back(type_from_ast_type(&bc, field.type));
		
		LLVMStructSetBody(struct_info.struct_type, type_array.data(), (u32)type_array.size(), 0);
	}

	for (Ast_Proc_IR_Info& proc_info : program->procs)
	{
		type_array.clear();
		Ast_Proc_Decl* proc_decl = proc_info.proc_decl;
		for (Ast_Proc_Param& param : proc_decl->input_params) 
		type_array.emplace_back(type_from_ast_type(&bc, param.type));

		Type ret_type = proc_decl->return_type ? type_from_ast_type(&bc, proc_decl->return_type.value()) : LLVMVoidType();
		char* name = (proc_decl->is_external || proc_decl->is_main) ? ident_to_cstr(proc_decl->ident) : "proc";
		proc_info.proc_type = LLVMFunctionType(ret_type, type_array.data(), (u32)type_array.size(), proc_info.proc_decl->is_variadic);
		proc_info.proc_value = LLVMAddFunction(bc.module, name, proc_info.proc_type);
	}

	for (Ast_Global_IR_Info& global_info : program->globals)
	{
		build_global_var(&bc, &global_info);
	}

	for (Ast_Struct_IR_Info& struct_info : program->structs)
	{
		struct_info.default_value = build_default_struct(&bc, &struct_info);
	}
	
	for (Ast_Proc_IR_Info& proc_info : program->procs)
	{
		Ast_Proc_Decl* proc_decl = proc_info.proc_decl;
		if (proc_decl->is_external) continue;

		builder_context_block_reset(&bc, proc_info.proc_value);
		builder_context_block_add(&bc);
		Basic_Block entry_block = builder_context_add_bb(&bc, "entry");
		builder_context_set_bb(&bc, entry_block);
		u32 count = 0;
		for (Ast_Proc_Param& param : proc_decl->input_params)
		{
			Type type = type_from_ast_type(&bc, param.type);
			Value param_value = LLVMGetParam(proc_info.proc_value, count);
			Value copy_ptr = LLVMBuildAlloca(bc.builder, type, "copy_ptr");
			LLVMBuildStore(bc.builder, param_value, copy_ptr);
			builder_context_block_add_var(&bc, IR_Var_Info { param.ident.str, copy_ptr, param.type });
			count += 1;
		}
		build_block(&bc, proc_decl->block, IR_Block_Flags::Already_Added);
		
		Basic_Block proc_exit_bb = builder_context_get_bb(&bc);
		Value terminator = LLVMGetBasicBlockTerminator(proc_exit_bb);
		if (terminator == NULL) LLVMBuildRetVoid(bc.builder);
	}

	builder_context_deinit(&bc);
	return bc.module;
}

IR_Terminator build_block(IR_Builder_Context* bc, Ast_Block* block, IR_Block_Flags flags)
{
	if (flags != IR_Block_Flags::Already_Added) builder_context_block_add(bc);

	for (Ast_Statement* statement : block->statements)
	{
		switch (statement->tag)
		{
		case Ast_Statement_Tag::If: build_if(bc, statement->as_if, builder_context_add_bb(bc, "cont")); break;
		case Ast_Statement_Tag::For: build_for(bc, statement->as_for); break;
		case Ast_Statement_Tag::Block:
		{
			IR_Terminator terminator = build_block(bc, statement->as_block, IR_Block_Flags::None);
			if (terminator != IR_Terminator::None)
			{
				build_defer(bc, terminator);
				builder_context_block_pop_back(bc);
				return terminator;
			}
		} break;
		case Ast_Statement_Tag::Defer: builder_context_block_add_defer(bc, statement->as_defer); break;
		case Ast_Statement_Tag::Break:
		{
			build_defer(bc, IR_Terminator::Break);
			IR_Loop_Info loop = builder_context_block_get_loop(bc);
			LLVMBuildBr(bc->builder, loop.break_block);
			
			builder_context_block_pop_back(bc);
			return IR_Terminator::Break;
		} break;
		case Ast_Statement_Tag::Return:
		{
			build_defer(bc, IR_Terminator::Return);
			if (statement->as_return->expr)
				LLVMBuildRet(bc->builder, build_expr(bc, statement->as_return->expr.value()));
			else LLVMBuildRetVoid(bc->builder);
			
			builder_context_block_pop_back(bc);
			return IR_Terminator::Return;
		} break;
		case Ast_Statement_Tag::Switch: build_switch(bc, statement->as_switch); break;
		case Ast_Statement_Tag::Continue:
		{
			build_defer(bc, IR_Terminator::Continue);
			IR_Loop_Info loop = builder_context_block_get_loop(bc);
			if (loop.var_assign) build_var_assign(bc, loop.var_assign.value());
			LLVMBuildBr(bc->builder, loop.continue_block);

			builder_context_block_pop_back(bc);
			return IR_Terminator::Continue;
		} break;
		case Ast_Statement_Tag::Var_Decl: build_var_decl(bc, statement->as_var_decl); break;
		case Ast_Statement_Tag::Var_Assign: build_var_assign(bc, statement->as_var_assign); break;
		case Ast_Statement_Tag::Proc_Call: build_proc_call(bc, statement->as_proc_call, IR_Proc_Call_Flags::In_Statement); break;
		}
	}

	build_defer(bc, IR_Terminator::None);
	builder_context_block_pop_back(bc);
	return IR_Terminator::None;
}

void build_defer(IR_Builder_Context* bc, IR_Terminator terminator)
{
	IR_Block_Info block_info = bc->blocks[bc->blocks.size() - 1];
	u64 total_defer_count = bc->defer_stack.size();
	u32 block_defer_count = block_info.defer_count;

	i32 start_defer_id = (i32)(total_defer_count - 1);
	i32 end_defer_id = (i32)(total_defer_count - block_defer_count);
	if (terminator == IR_Terminator::Return) end_defer_id = 0;

	for (i32 i = start_defer_id; i >= end_defer_id; i -= 1)
	build_block(bc, bc->defer_stack[i]->block, IR_Block_Flags::None);
}

void build_if(IR_Builder_Context* bc, Ast_If* _if, Basic_Block cont_block)
{
	Value cond_value = build_expr(bc, _if->condition_expr);

	if (_if->_else)
	{
		Basic_Block then_block = builder_context_add_bb(bc, "then");
		Basic_Block else_block = builder_context_add_bb(bc, "else");
		LLVMBuildCondBr(bc->builder, cond_value, then_block, else_block);
		builder_context_set_bb(bc, then_block);

		IR_Terminator terminator = build_block(bc, _if->block, IR_Block_Flags::None);
		if (terminator == IR_Terminator::None) LLVMBuildBr(bc->builder, cont_block);
		builder_context_set_bb(bc, else_block);

		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else_Tag::If)
		{
			build_if(bc, _else->as_if, cont_block);
		}
		else
		{
			IR_Terminator else_terminator = build_block(bc, _else->as_block, IR_Block_Flags::None);
			if (else_terminator == IR_Terminator::None) LLVMBuildBr(bc->builder, cont_block);
			builder_context_set_bb(bc, cont_block);
		}
	}
	else
	{
		Basic_Block then_block = builder_context_add_bb(bc, "then");
		LLVMBuildCondBr(bc->builder, cond_value, then_block, cont_block);
		builder_context_set_bb(bc, then_block);

		IR_Terminator terminator = build_block(bc, _if->block, IR_Block_Flags::None);
		if (terminator == IR_Terminator::None) LLVMBuildBr(bc->builder, cont_block);
		builder_context_set_bb(bc, cont_block);
	}
}

void build_for(IR_Builder_Context* bc, Ast_For* _for)
{
	if (_for->var_decl) build_var_decl(bc, _for->var_decl.value());

	Basic_Block cond_block = builder_context_add_bb(bc, "loop_cond");
	LLVMBuildBr(bc->builder, cond_block);
	builder_context_set_bb(bc, cond_block);

	Basic_Block body_block = builder_context_add_bb(bc, "loop_body");
	Basic_Block exit_block = builder_context_add_bb(bc, "loop_exit");
	if (_for->condition_expr)
	{
		Value cond_value = build_expr(bc, _for->condition_expr.value());
		LLVMBuildCondBr(bc->builder, cond_value, body_block, exit_block);
	}
	else LLVMBuildBr(bc->builder, body_block);
	builder_context_set_bb(bc, body_block);

	builder_context_block_add(bc);
	builder_context_block_add_loop(bc, IR_Loop_Info { exit_block, cond_block, _for->var_assign });
	IR_Terminator terminator = build_block(bc, _for->block, IR_Block_Flags::Already_Added);
	if (terminator == IR_Terminator::None)
	{
		if (_for->var_assign) build_var_assign(bc, _for->var_assign.value());
		LLVMBuildBr(bc->builder, cond_block);
	}
	builder_context_set_bb(bc, exit_block);
}

void build_switch(IR_Builder_Context* bc, Ast_Switch* _switch)
{
	Value on_value = build_expr(bc, _switch->expr);
	Basic_Block exit_block = builder_context_add_bb(bc, "switch_exit");
	Value switch_value = LLVMBuildSwitch(bc->builder, on_value, exit_block, (u32)_switch->cases.size());

	for (Ast_Switch_Case& _case : _switch->cases)
	{
		_case.basic_block = builder_context_add_bb(bc, "case_block");
	}

	for (u64 i = 0; i < _switch->cases.size(); i += 1)
	{
		Ast_Switch_Case _case = _switch->cases[i];
		Value case_value = build_expr(bc, _case.case_expr);
		Basic_Block case_block = _case.basic_block;
		LLVMAddCase(switch_value, case_value, case_block);
		builder_context_set_bb(bc, case_block);
		
		if (_case.block)
		{
			IR_Terminator terminator = build_block(bc, _case.block.value(), IR_Block_Flags::None);
			if (terminator == IR_Terminator::None) LLVMBuildBr(bc->builder, exit_block);
		}
		else
		{
			for (u64 k = i; k < _switch->cases.size(); k += 1)
			{
				Ast_Switch_Case next_case = _switch->cases[k];
				if (next_case.block)
				{
					LLVMBuildBr(bc->builder, next_case.basic_block);
					break;
				}
			}
		}
	}

	builder_context_set_bb(bc, exit_block);
}

void build_var_decl(IR_Builder_Context* bc, Ast_Var_Decl* var_decl)
{
	Type type = type_from_ast_type(bc, var_decl->type.value());
	Value var_ptr = LLVMBuildAlloca(bc->builder, type, ident_to_cstr(var_decl->ident));
	
	if (var_decl->expr)
	{
		Value value = build_expr(bc, var_decl->expr.value());
		build_implicit_cast(bc, &value, LLVMTypeOf(value), type);
		LLVMBuildStore(bc->builder, value, var_ptr);
	}
	else
	{
		Value default_value = build_default_value(bc, var_decl->type.value());
		LLVMBuildStore(bc->builder, default_value, var_ptr);
	}

	builder_context_block_add_var(bc, IR_Var_Info { var_decl->ident.str, var_ptr, var_decl->type.value() });
}

void build_var_assign(IR_Builder_Context* bc, Ast_Var_Assign* var_assign)
{
	IR_Access_Info access_info = build_var(bc, var_assign->var);
	Value value = build_expr(bc, var_assign->expr);
	build_implicit_cast(bc, &value, LLVMTypeOf(value), access_info.type);
	LLVMBuildStore(bc->builder, value, access_info.ptr);
}

void build_global_var(IR_Builder_Context* bc, Ast_Global_IR_Info* global_info)
{
	if (global_info->global_ptr != NULL) return;

	Ast_Global_Decl* global_decl = global_info->global_decl;
	Value const_value = build_expr(bc, global_decl->consteval_expr->expr);
	Value global = LLVMAddGlobal(bc->module, LLVMTypeOf(const_value), "g");
	LLVMSetInitializer(global, const_value);
	LLVMSetGlobalConstant(global, 1);
	global_info->global_ptr = global;
}

Value build_default_struct(IR_Builder_Context* bc, Ast_Struct_IR_Info* struct_info)
{
	std::vector<Value> values;
	Ast_Struct_Decl* struct_decl = struct_info->struct_decl;

	for (Ast_Struct_Field& field : struct_decl->fields)
	{
		if (field.default_expr)
		{
			Value value = build_expr(bc, field.default_expr.value());
			build_implicit_cast(bc, &value, LLVMTypeOf(value), type_from_ast_type(bc, field.type));
			values.emplace_back(value);
		}
		else
		{
			Value value = build_default_value(bc, field.type);
			values.emplace_back(value);
		}
	}

	struct_info->default_value = LLVMConstNamedStruct(struct_info->struct_type, values.data(), (u32)values.size());
	return struct_info->default_value;
}

Value build_default_value(IR_Builder_Context* bc, Ast_Type ast_type)
{
	Type type = type_from_ast_type(bc, ast_type);
	if (ast_type.pointer_level > 0) return LLVMConstNull(type);

	switch (ast_type.tag)
	{
	case Ast_Type_Tag::Basic:
	{
		return LLVMConstNull(type);
	}
	case Ast_Type_Tag::Array:
	{
		Ast_Array_Type* array_type = ast_type.as_array;
		u32 size = LLVMGetArrayLength(type);
		Value default_value = build_default_value(bc, array_type->element_type);
		
		std::vector<Value> values;
		values.reserve(size);
		for (u32 i = 0; i < size; i += 1)
		values.emplace_back(default_value);
		
		Type element_type = type_from_ast_type(bc, array_type->element_type);
		return LLVMConstArray(element_type, values.data(), size);
	}
	case Ast_Type_Tag::Struct:
	{
		Value default_struct = bc->program->structs[ast_type.as_struct.struct_id].default_value;
		if (default_struct == NULL) return build_default_struct(bc, &bc->program->structs[ast_type.as_struct.struct_id]);
		return default_struct;
	}
	case Ast_Type_Tag::Enum:
	{
		return ast_type.as_enum.enum_decl->variants[0].constant;
	}
	default: { err_internal("build_default_value: invalid Ast_Type_Tag"); return NULL; }
	}
}

//@Notice proc result access might be changed to work with address / deref similarly to var 
Value build_proc_call(IR_Builder_Context* bc, Ast_Proc_Call* proc_call, IR_Proc_Call_Flags flags)
{
	Ast_Proc_IR_Info proc_info = bc->program->procs[proc_call->resolved.proc_id];

	std::vector<Value> input_values = {}; //@Perf memory overhead
	input_values.reserve(proc_call->input_exprs.size());
	for (Ast_Expr* expr : proc_call->input_exprs)
	input_values.emplace_back(build_expr(bc, expr));

	if (flags == IR_Proc_Call_Flags::In_Statement)
	{
		return LLVMBuildCall2(bc->builder, proc_info.proc_type, proc_info.proc_value, 
		input_values.data(), (u32)input_values.size(), "");
	}

	Value return_value = LLVMBuildCall2(bc->builder, proc_info.proc_type, proc_info.proc_value, 
	input_values.data(), (u32)input_values.size(), "call_val");
	if (!proc_call->access) return return_value;

	Ast_Type return_ast_type = proc_info.proc_decl->return_type.value();
	Type return_type = type_from_ast_type(bc, return_ast_type);
	Value temp_ptr = LLVMBuildAlloca(bc->builder, return_type, "call_temp");
	LLVMBuildStore(bc->builder, return_value, temp_ptr);

	IR_Access_Info access_info = build_access(bc, proc_call->access.value(), temp_ptr, return_ast_type);
	return LLVMBuildLoad2(bc->builder, access_info.type, access_info.ptr, "call_access_val");
}

Value build_expr(IR_Builder_Context* bc, Ast_Expr* expr, bool unary_address)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term: return build_term(bc, expr->as_term, unary_address);
	case Ast_Expr_Tag::Unary_Expr: return build_unary_expr(bc, expr->as_unary_expr);
	case Ast_Expr_Tag::Binary_Expr: return build_binary_expr(bc, expr->as_binary_expr);
	case Ast_Expr_Tag::Folded_Expr: return build_folded_expr(bc, expr->as_folded_expr);
	default: { err_internal("build_expr: invalid Ast_Expr_Tag"); return NULL; }
	}
}

Value build_term(IR_Builder_Context* bc, Ast_Term* term, bool unary_address)
{
	switch (term->tag)
	{
	case Ast_Term_Tag::Var: 
	{
		IR_Access_Info access_info = build_var(bc, term->as_var);
		if (unary_address) return access_info.ptr;
		return LLVMBuildLoad2(bc->builder, access_info.type, access_info.ptr, "load_val");
	}
	case Ast_Term_Tag::Enum:
	{
		Ast_Enum* _enum = term->as_enum;
		return bc->program->enums[_enum->resolved.type.enum_id].enum_decl->variants[_enum->resolved.variant_id].constant;
	}
	case Ast_Term_Tag::Sizeof:
	{
		//@Notice returning 0 in void case to avoid crashing
		Ast_Sizeof* _sizeof = term->as_sizeof;
		Type type = type_from_ast_type(bc, _sizeof->type);
		if (LLVMTypeIsSized(type)) return LLVMSizeOf(type);
		return LLVMConstInt(LLVMInt64Type(), 0, 0);
	}
	case Ast_Term_Tag::Literal:
	{
		Token token = term->as_literal->token;
		if (token.type == TokenType::STRING_LITERAL)
		{
			//@Notice crash when no functions were declared (llvm bug)
			return LLVMBuildGlobalStringPtr(bc->builder, token.string_literal_value, "str");
		}
		else
		{
			printf("IR Builder: Expected Ast_Term literal to only be a string literal. Other things are Const_Expr now\n");
			exit(EXIT_FAILURE); //@Hack, maybe use compiler internal error instead
		}
	}
	case Ast_Term_Tag::Proc_Call: return build_proc_call(bc, term->as_proc_call, IR_Proc_Call_Flags::In_Expr);
	case Ast_Term_Tag::Struct_Init:
	{
		Ast_Struct_Init* struct_init = term->as_struct_init;
		Type type = bc->program->structs[struct_init->resolved.type.value().struct_id].struct_type;
		
		std::vector<Value> values; //@Perf frequent allocation
		bool is_const = true;
		for (Ast_Expr* expr : struct_init->input_exprs)
		{
			if (!expr->is_const) is_const = false;
			values.emplace_back(build_expr(bc, expr));
		}
		if (is_const) return LLVMConstNamedStruct(type, values.data(), (u32)values.size());

		Value temp_ptr = LLVMBuildAlloca(bc->builder, type, "temp_struct");
		
		u32 count = 0;
		for (Ast_Expr* expr : struct_init->input_exprs)
		{
			Value value = build_expr(bc, expr);
			Type field_type = LLVMStructGetTypeAtIndex(type, count);
			build_implicit_cast(bc, &value, LLVMTypeOf(value), field_type);
			Value field_ptr = LLVMBuildStructGEP2(bc->builder, type, temp_ptr, count, "fieldptr");
			LLVMBuildStore(bc->builder, value, field_ptr);
			count += 1;
		}
		Value temp_value = LLVMBuildLoad2(bc->builder, type, temp_ptr, "temp_struct_val");
		return temp_value;
	}
	case Ast_Term_Tag::Array_Init:
	{
		Ast_Array_Init* array_init = term->as_array_init;
		Type type = type_from_ast_type(bc, array_init->type.value());
		Type element_type = type_from_ast_type(bc, array_init->type.value().as_array->element_type);

		std::vector<Value> values; //@Perf frequent allocation
		bool is_const = true;
		for (Ast_Expr* expr : array_init->input_exprs)
		{
			if (!expr->is_const) is_const = false;
			values.emplace_back(build_expr(bc, expr));
		}
		if (is_const) return LLVMConstArray(element_type, values.data(), (u32)values.size());
		
		Value temp_ptr = LLVMBuildAlloca(bc->builder, type, "temp_array");
		
		u32 count = 0;
		for (Ast_Expr* expr : array_init->input_exprs)
		{
			Value value = build_expr(bc, expr);
			build_implicit_cast(bc, &value, LLVMTypeOf(value), element_type);
			Value index = LLVMConstInt(type_from_basic_type(BasicType::U32), count, 0);
			Value array_ptr = LLVMBuildGEP2(bc->builder, element_type, temp_ptr, &index, 1, "arrayptr");
			LLVMBuildStore(bc->builder, value, array_ptr);
			count += 1;
		}
		Value temp_value = LLVMBuildLoad2(bc->builder, type, temp_ptr, "temp_array_val");
		return temp_value;
	}
	default: { err_internal("build_term: invalid Ast_Term_Tag"); return NULL; }
	}
}

IR_Access_Info build_var(IR_Builder_Context* bc, Ast_Var* var)
{
	if (var->tag == Ast_Var_Tag::Global)
	{
		Ast_Global_IR_Info* global_info = &bc->program->globals[var->global.global_id];
		build_global_var(bc, global_info);
		return build_access(bc, var->access, global_info->global_ptr, global_info->global_decl->type.value());
	}
	else
	{
		IR_Var_Info var_info = builder_context_block_find_var(bc, var->local.ident);
		return build_access(bc, var->access, var_info.ptr, var_info.ast_type);
	}
}

IR_Access_Info build_access(IR_Builder_Context* bc, option<Ast_Access*> access_option, Value ptr, Ast_Type ast_type)
{
	if (!access_option) return IR_Access_Info{ ptr, type_from_ast_type(bc, ast_type) };

	Ast_Access* access = access_option.value();
	while (access != NULL)
	{
		if (access->tag == Ast_Access_Tag::Array)
		{
			if (ast_type.pointer_level > 0) //@Temp allowing pointer to array array access before slices
			{
				ptr = LLVMBuildLoad2(bc->builder, LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0), ptr, "ptr_load");
				ast_type.pointer_level -= 1;
			}

			Ast_Array_Access* array_access = access->as_array;

			Value index = build_expr(bc, array_access->index_expr);
			Type element_type = type_from_ast_type(bc, ast_type.as_array->element_type);
			ptr = LLVMBuildGEP2(bc->builder, element_type, ptr, &index, 1, "array_gep");
			ast_type = ast_type.as_array->element_type;

			access = array_access->next.has_value() ? array_access->next.value() : NULL;
		}
		else
		{
			if (ast_type.pointer_level > 0)
			{
				ptr = LLVMBuildLoad2(bc->builder, LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0), ptr, "ptr_load");
				ast_type.pointer_level -= 1;
			}

			Ast_Var_Access* var_access = access->as_var;

			Ast_Struct_IR_Info struct_info = bc->program->structs[ast_type.as_struct.struct_id];
			ptr = LLVMBuildStructGEP2(bc->builder, struct_info.struct_type, ptr, var_access->field_id, "struct_ptr");
			ast_type = struct_info.struct_decl->fields[var_access->field_id].type;

			access = var_access->next.has_value() ? var_access->next.value() : NULL;
		}
	}

	return IR_Access_Info{ ptr, type_from_ast_type(bc, ast_type) };
}

Value build_unary_expr(IR_Builder_Context* bc, Ast_Unary_Expr* unary_expr)
{
	UnaryOp op = unary_expr->op;
	bool unary_address = op == UnaryOp::ADDRESS_OF;
	Value rhs = build_expr(bc, unary_expr->right, unary_address);

	switch (op)
	{
	case UnaryOp::MINUS:
	{
		Type rhs_type = LLVMTypeOf(rhs);
		if (LLVMGetTypeKind(rhs_type) == LLVMFloatTypeKind) return LLVMBuildFNeg(bc->builder, rhs, "utmp");
		else return LLVMBuildNeg(bc->builder, rhs, "utmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	}
	case UnaryOp::LOGIC_NOT: return LLVMBuildNot(bc->builder, rhs, "utmp");
	case UnaryOp::BITWISE_NOT: return LLVMBuildNot(bc->builder, rhs, "utmp");
	case UnaryOp::ADDRESS_OF: return rhs;
	case UnaryOp::DEREFERENCE: return NULL; //@Notice dereference isnt handled yet
	default: { err_internal("build_unary_expr: invalid UnaryOp"); return NULL; }
	}
}

Value build_binary_expr(IR_Builder_Context* bc, Ast_Binary_Expr* binary_expr)
{
	BinaryOp op = binary_expr->op;
	Value lhs = build_expr(bc, binary_expr->left);
	Value rhs = build_expr(bc, binary_expr->right);
	Type lhs_type = LLVMTypeOf(lhs);
	Type rhs_type = LLVMTypeOf(rhs);
	bool float_kind = LLVMGetTypeKind(lhs_type) == LLVMFloatTypeKind || LLVMGetTypeKind(lhs_type) == LLVMDoubleTypeKind;
	build_implicit_binary_cast(bc, &lhs, &rhs, lhs_type, rhs_type);

	switch (op)
	{
	// LogicOps [&& ||]
	case BinaryOp::LOGIC_AND: return LLVMBuildAnd(bc->builder, lhs, rhs, "btmp");
	case BinaryOp::LOGIC_OR: return LLVMBuildOr(bc->builder, lhs, rhs, "btmp");
	// CmpOps [< > <= >= == !=] //@RealPredicates using ordered (no nans) variants
	case BinaryOp::LESS:           if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOLT, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSLT, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BinaryOp::GREATER:        if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOGT, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSGT, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BinaryOp::LESS_EQUALS:    if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOLE, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSLE, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BinaryOp::GREATER_EQUALS: if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOGE, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSGE, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BinaryOp::IS_EQUALS:      if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOEQ, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntEQ, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BinaryOp::NOT_EQUALS:     if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealONE, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntNE, lhs, rhs, "btmp"); //@Determine S / U predicates
	// MathOps [+ - * / %]
	case BinaryOp::PLUS:  if (float_kind) return LLVMBuildFAdd(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildAdd(bc->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BinaryOp::MINUS: if (float_kind) return LLVMBuildFSub(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildSub(bc->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BinaryOp::TIMES: if (float_kind) return LLVMBuildFMul(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildMul(bc->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BinaryOp::DIV:   if (float_kind) return LLVMBuildFDiv(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildSDiv(bc->builder, lhs, rhs, "btmp"); //@ SU variants: LLVMBuildSDiv, LLVMBuildExactSDiv, LLVMBuildUDiv, LLVMBuildExactUDiv
	case BinaryOp::MOD: return LLVMBuildSRem(bc->builder, lhs, rhs, "btmp"); //@ SU rem variants: LLVMBuildSRem, LLVMBuildURem (using SRem always now)
	// BitwiseOps [& | ^ << >>]
	case BinaryOp::BITWISE_AND: return LLVMBuildAnd(bc->builder, lhs, rhs, "btmp"); // @Design only allow those for uints ideally
	case BinaryOp::BITWISE_OR: return LLVMBuildOr(bc->builder, lhs, rhs, "btmp");
	case BinaryOp::BITWISE_XOR: return LLVMBuildXor(bc->builder, lhs, rhs, "btmp");
	case BinaryOp::BITSHIFT_LEFT: return LLVMBuildShl(bc->builder, lhs, rhs, "btmp");
	case BinaryOp::BITSHIFT_RIGHT: return LLVMBuildLShr(bc->builder, lhs, rhs, "btmp"); //@LLVMBuildAShr used for maintaining the sign?
	default: { err_internal("build_binary_expr: invalid BinaryOp"); return NULL; }
	}
}

Value build_folded_expr(IR_Builder_Context* bc, Ast_Folded_Expr folded_expr)
{
	Type type = type_from_basic_type(folded_expr.basic_type);
	switch (folded_expr.basic_type)
	{
	case BasicType::BOOL: return LLVMConstInt(type, (int)folded_expr.as_bool, 0);
	case BasicType::F32:
	case BasicType::F64: return LLVMConstReal(type, folded_expr.as_f64);
	case BasicType::I8:
	case BasicType::I16:
	case BasicType::I32:
	case BasicType::I64: return LLVMConstInt(type, folded_expr.as_i64, 1);
	default: return LLVMConstInt(type, folded_expr.as_u64, 1);
	}
}

void build_implicit_cast(IR_Builder_Context* bc, Value* value, Type type, Type target_type)
{
	if (type == target_type) return;
	LLVMTypeKind kind = LLVMGetTypeKind(type);
	bool is_float = kind == LLVMFloatTypeKind || kind == LLVMDoubleTypeKind;
	bool is_int = !is_float;

	if (is_float)
	{
		*value = LLVMBuildFPCast(bc->builder, *value, target_type, "fval");
		return;
	}

	if (is_int)
	{
		//
	}
}

void build_implicit_binary_cast(IR_Builder_Context* bc, Value* value_lhs, Value* value_rhs, Type type_lhs, Type type_rhs)
{
	if (type_lhs == type_rhs) return;
	LLVMTypeKind kind_lhs = LLVMGetTypeKind(type_lhs);
	bool is_float = kind_lhs == LLVMFloatTypeKind || kind_lhs == LLVMDoubleTypeKind;
	bool is_int = !is_float;

	if (is_float)
	{
		if (kind_lhs == LLVMFloatTypeKind)
			*value_lhs = LLVMBuildFPCast(bc->builder, *value_lhs, type_rhs, "fval");
		else *value_rhs = LLVMBuildFPCast(bc->builder, *value_rhs, type_lhs, "fval");
		return;
	}

	if (is_int)
	{
		//
	}
}

char* ident_to_cstr(Ast_Ident& ident)
{
	ident.str.data[ident.str.count] = '\0';
	return (char*)ident.str.data;
}

Type type_from_basic_type(BasicType basic_type)
{
	switch (basic_type)
	{
	case BasicType::I8: return LLVMInt8Type();
	case BasicType::U8: return LLVMInt8Type();
	case BasicType::I16: return LLVMInt16Type();
	case BasicType::U16: return LLVMInt16Type();
	case BasicType::I32: return LLVMInt32Type();
	case BasicType::U32: return LLVMInt32Type();
	case BasicType::I64: return LLVMInt64Type();
	case BasicType::U64: return LLVMInt64Type();
	case BasicType::F32: return LLVMFloatType();
	case BasicType::F64: return LLVMDoubleType();
	case BasicType::BOOL: return LLVMInt1Type();
	default: return LLVMVoidType(); //@Notice string is void type
	}
}

Type type_from_ast_type(IR_Builder_Context* bc, Ast_Type type)
{
	if (type.pointer_level > 0) return LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0);

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic: return type_from_basic_type(type.as_basic);
	case Ast_Type_Tag::Array:
	{
		Ast_Array_Type* array = type.as_array;
		Type element_type = type_from_ast_type(bc, array->element_type);
		u32 size = (u32)array->size_expr->as_folded_expr.as_u64; //@Notice what if its positive i64
		return LLVMArrayType(element_type, size);
	}
	case Ast_Type_Tag::Struct: return bc->program->structs[type.as_struct.struct_id].struct_type;
	case Ast_Type_Tag::Enum: return bc->program->enums[type.as_enum.enum_id].enum_type;
	default: return LLVMVoidType();
	}
}
