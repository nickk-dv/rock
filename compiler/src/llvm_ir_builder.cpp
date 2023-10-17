#include "llvm_ir_builder.h"

#include "llvm-c/Core.h"

Module build_module(Ast_Program* program)
{
	IR_Builder_Context bc = {};
	context_init(&bc, program);

	for (Ast_Enum_Meta& enum_meta : program->enums)
	{
		BasicType basic_type = BASIC_TYPE_I32; //@Notice maybe store i32 at checking stage
		if (enum_meta.enum_decl->basic_type) basic_type = enum_meta.enum_decl->basic_type.value();
		Type type = type_from_basic_type(basic_type);
		enum_meta.enum_type = type;

		for (Ast_Ident_Literal_Pair& variant : enum_meta.enum_decl->variants)
		{
			int sign = variant.is_negative ? -1 : 1;
			if (basic_type <= BASIC_TYPE_U64) variant.constant = LLVMConstInt(type, sign * variant.literal.token.integer_value, basic_type % 2 == 0); //@Check if sign extend is correct or needed
			else if (basic_type <= BASIC_TYPE_F64) variant.constant = LLVMConstReal(type, sign * variant.literal.token.float64_value);
			else variant.constant = LLVMConstInt(type, (int)variant.literal.token.bool_value, 0);
		}
	}

	std::vector<Type> type_array(32);
	
	for (Ast_Struct_Meta& struct_meta : program->structs)
	{
		struct_meta.struct_type = LLVMStructCreateNamed(LLVMGetGlobalContext(), "struct");
	}

	for (Ast_Struct_Meta& struct_meta : program->structs)
	{
		type_array.clear();
		Ast_Struct_Decl* struct_decl = struct_meta.struct_decl;
		for (Ast_Ident_Type_Pair& field : struct_decl->fields) 
		type_array.emplace_back(type_from_ast_type(&bc, field.type));
		
		LLVMStructSetBody(struct_meta.struct_type, type_array.data(), (u32)type_array.size(), 0);

		LLVMAddGlobal(bc.module, struct_meta.struct_type, "global_test");
	}

	for (Ast_Proc_Meta& proc_meta : program->procedures)
	{
		type_array.clear();
		Ast_Proc_Decl* proc_decl = proc_meta.proc_decl;
		for (Ast_Ident_Type_Pair& param : proc_decl->input_params) 
		type_array.emplace_back(type_from_ast_type(&bc, param.type));

		Type ret_type = proc_decl->return_type ? type_from_ast_type(&bc, proc_decl->return_type.value()) : LLVMVoidType();
		char* name = (proc_decl->is_external || proc_decl->is_main) ? ident_to_cstr(proc_decl->ident) : "proc";
		proc_meta.proc_type = LLVMFunctionType(ret_type, type_array.data(), (u32)type_array.size(), 0);
		proc_meta.proc_value = LLVMAddFunction(bc.module, name, proc_meta.proc_type);
	}

	for (Ast_Proc_Meta& proc_meta : program->procedures)
	{
		Ast_Proc_Decl* proc_decl = proc_meta.proc_decl;
		if (proc_decl->is_external) continue;

		context_block_reset(&bc, proc_meta.proc_value);
		context_block_add(&bc);
		Basic_Block entry_block = context_add_bb(&bc, "entry");
		context_set_bb(&bc, entry_block);
		u32 count = 0;
		for (Ast_Ident_Type_Pair& param : proc_decl->input_params)
		{
			Type type = type_from_ast_type(&bc, param.type);
			Value param_value = LLVMGetParam(proc_meta.proc_value, count);
			Value copy_ptr = LLVMBuildAlloca(bc.builder, type, "copy_ptr");
			LLVMBuildStore(bc.builder, param_value, copy_ptr);
			context_block_add_var(&bc, IR_Var_Info { param.ident.str, copy_ptr, param.type });
			count += 1;
		}
		build_block(&bc, proc_decl->block, Block_Flags::Already_Added);
		
		Basic_Block proc_exit_bb = context_get_bb(&bc);
		Value terminator = LLVMGetBasicBlockTerminator(proc_exit_bb);
		if (terminator == NULL) LLVMBuildRetVoid(bc.builder);
	}

	context_deinit(&bc);
	return bc.module;
}

Block_Terminator build_block(IR_Builder_Context* bc, Ast_Block* block, Block_Flags flags)
{
	if (flags != Block_Flags::Already_Added) context_block_add(bc);

	for (Ast_Statement* statement : block->statements)
	{
		switch (statement->tag)
		{
		case Ast_Statement::Tag::If: build_if(bc, statement->as_if, context_add_bb(bc, "cont")); break;
		case Ast_Statement::Tag::For: build_for(bc, statement->as_for); break;
		case Ast_Statement::Tag::Block:
		{
			Block_Terminator terminator = build_block(bc, statement->as_block, Block_Flags::None);
			if (terminator != Block_Terminator::None)
			{
				build_defer(bc, terminator);
				context_block_pop_back(bc);
				return terminator;
			}
		} break;
		case Ast_Statement::Tag::Defer: context_block_add_defer(bc, statement->as_defer); break;
		case Ast_Statement::Tag::Break:
		{
			build_defer(bc, Block_Terminator::Break);
			IR_Loop_Info loop = context_block_get_loop(bc);
			LLVMBuildBr(bc->builder, loop.break_block);
			
			context_block_pop_back(bc);
			return Block_Terminator::Break;
		} break;
		case Ast_Statement::Tag::Return:
		{
			build_defer(bc, Block_Terminator::Return);
			if (statement->as_return->expr)
				LLVMBuildRet(bc->builder, build_expr(bc, statement->as_return->expr.value()));
			else LLVMBuildRetVoid(bc->builder);
			
			context_block_pop_back(bc);
			return Block_Terminator::Return;
		} break;
		case Ast_Statement::Tag::Switch: build_switch(bc, statement->as_switch); break;
		case Ast_Statement::Tag::Continue:
		{
			build_defer(bc, Block_Terminator::Continue);
			IR_Loop_Info loop = context_block_get_loop(bc);
			if (loop.var_assign) build_var_assign(bc, loop.var_assign.value());
			LLVMBuildBr(bc->builder, loop.continue_block);

			context_block_pop_back(bc);
			return Block_Terminator::Continue;
		} break;
		case Ast_Statement::Tag::Var_Decl: build_var_decl(bc, statement->as_var_decl); break;
		case Ast_Statement::Tag::Var_Assign: build_var_assign(bc, statement->as_var_assign); break;
		case Ast_Statement::Tag::Proc_Call: build_proc_call(bc, statement->as_proc_call, Proc_Call_Flags::Is_Statement); break;
		}
	}

	build_defer(bc, Block_Terminator::None);
	context_block_pop_back(bc);
	return Block_Terminator::None;
}

void build_defer(IR_Builder_Context* bc, Block_Terminator terminator)
{
	IR_Block_Info block_info = bc->blocks[bc->blocks.size() - 1];
	u64 total_defer_count = bc->defer_stack.size();
	u32 block_defer_count = block_info.defer_count;

	int start_defer_id = (int)(total_defer_count - 1);
	int end_defer_id = (int)(total_defer_count - block_defer_count);
	if (terminator == Block_Terminator::Return) end_defer_id = 0;

	for (int i = start_defer_id; i >= end_defer_id; i -= 1) 
	build_block(bc, bc->defer_stack[i]->block, Block_Flags::None);
}

void build_if(IR_Builder_Context* bc, Ast_If* _if, Basic_Block cont_block)
{
	Value cond_value = build_expr(bc, _if->condition_expr);

	if (_if->_else)
	{
		Basic_Block then_block = context_add_bb(bc, "then");
		Basic_Block else_block = context_add_bb(bc, "else");
		LLVMBuildCondBr(bc->builder, cond_value, then_block, else_block);
		context_set_bb(bc, then_block);

		Block_Terminator terminator = build_block(bc, _if->block, Block_Flags::None);
		if (terminator == Block_Terminator::None) LLVMBuildBr(bc->builder, cont_block);
		context_set_bb(bc, else_block);

		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else::Tag::If)
		{
			build_if(bc, _else->as_if, cont_block);
		}
		else
		{
			Block_Terminator else_terminator = build_block(bc, _else->as_block, Block_Flags::None);
			if (else_terminator == Block_Terminator::None) LLVMBuildBr(bc->builder, cont_block);
			context_set_bb(bc, cont_block);
		}
	}
	else
	{
		Basic_Block then_block = context_add_bb(bc, "then");
		LLVMBuildCondBr(bc->builder, cond_value, then_block, cont_block);
		context_set_bb(bc, then_block);

		Block_Terminator terminator = build_block(bc, _if->block, Block_Flags::None);
		if (terminator == Block_Terminator::None) LLVMBuildBr(bc->builder, cont_block);
		context_set_bb(bc, cont_block);
	}
}

void build_for(IR_Builder_Context* bc, Ast_For* _for)
{
	if (_for->var_decl) build_var_decl(bc, _for->var_decl.value());

	Basic_Block cond_block = context_add_bb(bc, "loop_cond");
	LLVMBuildBr(bc->builder, cond_block);
	context_set_bb(bc, cond_block);

	Basic_Block body_block = context_add_bb(bc, "loop_body");
	Basic_Block exit_block = context_add_bb(bc, "loop_exit");
	if (_for->condition_expr)
	{
		Value cond_value = build_expr(bc, _for->condition_expr.value());
		LLVMBuildCondBr(bc->builder, cond_value, body_block, exit_block);
	}
	else LLVMBuildBr(bc->builder, body_block);
	context_set_bb(bc, body_block);

	context_block_add(bc);
	context_block_add_loop(bc, IR_Loop_Info { exit_block, cond_block, _for->var_assign });
	Block_Terminator terminator = build_block(bc, _for->block, Block_Flags::Already_Added);
	if (terminator == Block_Terminator::None)
	{
		if (_for->var_assign) build_var_assign(bc, _for->var_assign.value());
		LLVMBuildBr(bc->builder, cond_block);
	}
	context_set_bb(bc, exit_block);
}

void build_switch(IR_Builder_Context* bc, Ast_Switch* _switch)
{
	Value on_value = build_term(bc, _switch->term);
	Basic_Block exit_block = context_add_bb(bc, "switch_exit");
	Value switch_value = LLVMBuildSwitch(bc->builder, on_value, exit_block, (u32)_switch->cases.size());

	for (Ast_Switch_Case& _case : _switch->cases)
	{
		_case.basic_block = context_add_bb(bc, "case_block");
	}

	for (u64 i = 0; i < _switch->cases.size(); i += 1)
	{
		Ast_Switch_Case _case = _switch->cases[i];
		Value case_value = build_term(bc, _case.term);
		Basic_Block case_block = _case.basic_block;
		LLVMAddCase(switch_value, case_value, case_block);
		context_set_bb(bc, case_block);
		
		if (_case.block)
		{
			Block_Terminator terminator = build_block(bc, _case.block.value(), Block_Flags::None);
			if (terminator == Block_Terminator::None) LLVMBuildBr(bc->builder, exit_block);
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

	context_set_bb(bc, exit_block);
}

void build_var_decl(IR_Builder_Context* bc, Ast_Var_Decl* var_decl)
{
	Type type = type_from_ast_type(bc, var_decl->type.value());
	Value var_ptr = LLVMBuildAlloca(bc->builder, type, ident_to_cstr(var_decl->ident));
	
	if (var_decl->expr)
	{
		//@Special case for struct initialization may generalize to work as normal expression
		Ast_Expr* decl_expr = var_decl->expr.value();
		if (decl_expr->tag == Ast_Expr::Tag::Term && decl_expr->as_term->tag == Ast_Term::Tag::Struct_Init)
		{
			u32 count = 0;
			Ast_Struct_Init* struct_init = decl_expr->as_term->as_struct_init;
			for (Ast_Expr* expr : struct_init->input_exprs)
			{
				Value expr_value = build_expr(bc, expr);
				Type field_type = LLVMStructGetTypeAtIndex(type, count);
				build_implicit_cast(bc, &expr_value, LLVMTypeOf(expr_value), field_type);
				Value field_ptr = LLVMBuildStructGEP2(bc->builder, type, var_ptr, count, "fieldptr");
				LLVMBuildStore(bc->builder, expr_value, field_ptr);
				count += 1;
			}
		}
		else
		{
			Value expr_value = build_expr(bc, var_decl->expr.value());
			build_implicit_cast(bc, &expr_value, LLVMTypeOf(expr_value), type);
			LLVMBuildStore(bc->builder, expr_value, var_ptr);
		}
	}
	else LLVMBuildStore(bc->builder, LLVMConstNull(type), var_ptr);

	context_block_add_var(bc, IR_Var_Info { var_decl->ident.str, var_ptr, var_decl->type.value() });
}

void build_var_assign(IR_Builder_Context* bc, Ast_Var_Assign* var_assign)
{
	IR_Access_Info access_info = build_var(bc, var_assign->var);
	
	//@Special case for struct initialization may generalize to work as normal expression
	Ast_Expr* assign_expr = var_assign->expr;
	if (assign_expr->tag == Ast_Expr::Tag::Term)
	{
		Ast_Term* term = assign_expr->as_term;
		if (term->tag == Ast_Term::Tag::Struct_Init)
		{
			u32 count = 0;
			Ast_Struct_Init* struct_init = term->as_struct_init;
			for (Ast_Expr* expr : struct_init->input_exprs)
			{
				Value expr_value = build_expr(bc, expr);
				Type field_type = LLVMStructGetTypeAtIndex(access_info.type, count);
				build_implicit_cast(bc, &expr_value, LLVMTypeOf(expr_value), field_type);
				Value field_ptr = LLVMBuildStructGEP2(bc->builder, access_info.type, access_info.ptr, count, "fieldptr");
				LLVMBuildStore(bc->builder, expr_value, field_ptr);
				count += 1;
			}
			return;
		}
	}

	Value expr_value = build_expr(bc, var_assign->expr);
	build_implicit_cast(bc, &expr_value, LLVMTypeOf(expr_value), access_info.type);
	LLVMBuildStore(bc->builder, expr_value, access_info.ptr);
}

Value build_proc_call(IR_Builder_Context* bc, Ast_Proc_Call* proc_call, Proc_Call_Flags flags)
{
	Ast_Proc_Meta proc_meta = bc->program->procedures[proc_call->proc_id];

	std::vector<Value> input_values = {}; //@Perf memory overhead
	input_values.reserve(proc_call->input_exprs.size());
	for (Ast_Expr* expr : proc_call->input_exprs)
	input_values.emplace_back(build_expr(bc, expr));

	return LLVMBuildCall2(bc->builder, proc_meta.proc_type, proc_meta.proc_value, 
	input_values.data(), (u32)input_values.size(), flags == Proc_Call_Flags::Is_Statement ? "" : "call_val");
}

Value build_expr(IR_Builder_Context* bc, Ast_Expr* expr)
{
	switch (expr->tag)
	{
	case Ast_Expr::Tag::Term: return build_term(bc, expr->as_term);
	case Ast_Expr::Tag::Unary_Expr: return build_unary_expr(bc, expr->as_unary_expr);
	case Ast_Expr::Tag::Binary_Expr: return build_binary_expr(bc, expr->as_binary_expr);
	}
}

Value build_term(IR_Builder_Context* bc, Ast_Term* term)
{
	switch (term->tag)
	{
	case Ast_Term::Tag::Var: 
	{	
		//@Todo handle unary adress op by returning ptr without load
		IR_Access_Info access_info = build_var(bc, term->as_var);
		return LLVMBuildLoad2(bc->builder, access_info.type, access_info.ptr, "load_val");
	}
	case Ast_Term::Tag::Enum:
	{
		Ast_Enum* _enum = term->as_enum;
		return bc->program->enums[_enum->enum_id].enum_decl->variants[_enum->variant_id].constant;
	}
	case Ast_Term::Tag::Literal:
	{
		Token token = term->as_literal.token;
		if (token.type == TOKEN_BOOL_LITERAL) return LLVMConstInt(LLVMInt1Type(), (int)token.bool_value, 0);
		else if (token.type == TOKEN_FLOAT_LITERAL) return LLVMConstReal(LLVMDoubleType(), token.float64_value);
		else if (token.type == TOKEN_INTEGER_LITERAL) return LLVMConstInt(LLVMInt32Type(), token.integer_value, 0); //@Todo sign extend?
		else return LLVMConstInt(LLVMInt32Type(), 0, 0); //@Notice string literal isnt supported
	}
	case Ast_Term::Tag::Proc_Call: return build_proc_call(bc, term->as_proc_call, Proc_Call_Flags::None);
	case Ast_Term::Tag::Struct_Init: return NULL; //@Notice returning null on struct init, it should be handled on var assigned for now
	}
}

IR_Access_Info build_var(IR_Builder_Context* bc, Ast_Var* var)
{
	IR_Var_Info var_info = context_block_find_var(bc, var->ident);
	Value ptr = var_info.ptr;
	Ast_Type ast_type = var_info.ast_type;

	Ast_Access* access = var->access.has_value() ? var->access.value() : NULL;
	while (access != NULL)
	{
		if (access->tag == Ast_Access::Tag::Array)
		{
			printf("llvm ir builder 2 build_var: array access isnt supported\n");
			exit(EXIT_FAILURE);
			return {}; //@Todo arrays
		}
		else
		{
			if (ast_type.pointer_level > 0)
			{
				ptr = LLVMBuildLoad2(bc->builder, LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0), ptr, "ptr_load");
				ast_type.pointer_level -= 1;
			}

			Ast_Var_Access* var_access = access->as_var;
			Ast_Struct_Meta struct_meta = bc->program->structs[ast_type.as_struct.struct_id];
			ptr = LLVMBuildStructGEP2(bc->builder, struct_meta.struct_type, ptr, var_access->field_id, "struct_ptr");
			ast_type = struct_meta.struct_decl->fields[var_access->field_id].type;
			
			access = var_access->next.has_value() ? var_access->next.value() : NULL;
		}
	}

	return IR_Access_Info { ptr, type_from_ast_type(bc, ast_type) };
}

Value build_unary_expr(IR_Builder_Context* bc, Ast_Unary_Expr* unary_expr)
{
	UnaryOp op = unary_expr->op;
	Value rhs = build_expr(bc, unary_expr->right);

	switch (op)
	{
	case UNARY_OP_MINUS:
	{
		Type rhs_type = LLVMTypeOf(rhs);
		if (LLVMGetTypeKind(rhs_type) == LLVMFloatTypeKind) return LLVMBuildFNeg(bc->builder, rhs, "utmp");
		else return LLVMBuildNeg(bc->builder, rhs, "utmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	}
	case UNARY_OP_LOGIC_NOT: return LLVMBuildNot(bc->builder, rhs, "utmp");
	case UNARY_OP_BITWISE_NOT: return LLVMBuildNot(bc->builder, rhs, "utmp");
	case UNARY_OP_ADDRESS_OF: return rhs;
	case UNARY_OP_DEREFERENCE: return NULL; //@Notice dereference isnt handled yet
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
	case BINARY_OP_LOGIC_AND: return LLVMBuildAnd(bc->builder, lhs, rhs, "btmp");
	case BINARY_OP_LOGIC_OR: return LLVMBuildOr(bc->builder, lhs, rhs, "btmp");
	// CmpOps [< > <= >= == !=] //@RealPredicates using ordered (no nans) variants
	case BINARY_OP_LESS:           if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOLT, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSLT, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_GREATER:        if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOGT, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSGT, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_LESS_EQUALS:    if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOLE, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSLE, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_GREATER_EQUALS: if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOGE, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntSGE, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_IS_EQUALS:      if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealOEQ, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntEQ, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_NOT_EQUALS:     if (float_kind) return LLVMBuildFCmp(bc->builder, LLVMRealONE, lhs, rhs, "btmp"); else return LLVMBuildICmp(bc->builder, LLVMIntNE, lhs, rhs, "btmp"); //@Determine S / U predicates
	// MathOps [+ - * / %]
	case BINARY_OP_PLUS:  if (float_kind) return LLVMBuildFAdd(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildAdd(bc->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BINARY_OP_MINUS: if (float_kind) return LLVMBuildFSub(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildSub(bc->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BINARY_OP_TIMES: if (float_kind) return LLVMBuildFMul(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildMul(bc->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BINARY_OP_DIV:   if (float_kind) return LLVMBuildFDiv(bc->builder, lhs, rhs, "btmp"); else return LLVMBuildSDiv(bc->builder, lhs, rhs, "btmp"); //@ SU variants: LLVMBuildSDiv, LLVMBuildExactSDiv, LLVMBuildUDiv, LLVMBuildExactUDiv
	case BINARY_OP_MOD: return LLVMBuildSRem(bc->builder, lhs, rhs, "btmp"); //@ SU rem variants: LLVMBuildSRem, LLVMBuildURem (using SRem always now)
	// BitwiseOps [& | ^ << >>]
	case BINARY_OP_BITWISE_AND: return LLVMBuildAnd(bc->builder, lhs, rhs, "btmp"); // @Design only allow those for uints ideally
	case BINARY_OP_BITWISE_OR: return LLVMBuildOr(bc->builder, lhs, rhs, "btmp");
	case BINARY_OP_BITWISE_XOR: return LLVMBuildXor(bc->builder, lhs, rhs, "btmp");
	case BINARY_OP_BITSHIFT_LEFT: return LLVMBuildShl(bc->builder, lhs, rhs, "btmp");
	case BINARY_OP_BITSHIFT_RIGHT: return LLVMBuildLShr(bc->builder, lhs, rhs, "btmp"); //@LLVMBuildAShr used for maintaining the sign?
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
	case BASIC_TYPE_I8: return LLVMInt8Type();
	case BASIC_TYPE_U8: return LLVMInt8Type();
	case BASIC_TYPE_I16: return LLVMInt16Type();
	case BASIC_TYPE_U16: return LLVMInt16Type();
	case BASIC_TYPE_I32: return LLVMInt32Type();
	case BASIC_TYPE_U32: return LLVMInt32Type();
	case BASIC_TYPE_I64: return LLVMInt64Type();
	case BASIC_TYPE_U64: return LLVMInt64Type();
	case BASIC_TYPE_F32: return LLVMFloatType();
	case BASIC_TYPE_F64: return LLVMDoubleType();
	case BASIC_TYPE_BOOL: return LLVMInt1Type();
	default: return LLVMVoidType(); //@Notice string is void type
	}
}

Type type_from_ast_type(IR_Builder_Context* bc, Ast_Type type)
{
	if (type.pointer_level > 0) return LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0);

	switch (type.tag)
	{
	case Ast_Type::Tag::Basic: return type_from_basic_type(type.as_basic);
	case Ast_Type::Tag::Array: return LLVMVoidType(); //@Notice array type isnt supported
	case Ast_Type::Tag::Struct: return bc->program->structs[type.as_struct.struct_id].struct_type;
	case Ast_Type::Tag::Enum: return bc->program->enums[type.as_enum.enum_id].enum_type;
	default: return LLVMVoidType();
	}
}
