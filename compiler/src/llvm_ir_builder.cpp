#include "llvm_ir_builder.h"

#include "llvm-c/Core.h"
#include "debug_printer.h"

LLVMModuleRef LLVM_IR_Builder::build_module(Ast* ast)
{
	module = LLVMModuleCreateWithName("module");
	builder = LLVMCreateBuilder();

	enum_decl_map.init(32);
	struct_decl_map.init(32);
	proc_decl_map.init(32);
	for (Ast_Enum_Decl* enum_decl : ast->enums) { build_enum_decl(enum_decl); }
	for (Ast_Struct_Decl* struct_decl : ast->structs) { build_struct_decl(struct_decl); }
	for (Ast_Proc_Decl* proc_decl : ast->procs) { build_proc_decl(proc_decl); }
	for (Ast_Proc_Decl* proc_decl : ast->procs) { build_proc_body(proc_decl); }
	
	LLVMDisposeBuilder(builder);
	return module;
}

void LLVM_IR_Builder::build_enum_decl(Ast_Enum_Decl* enum_decl)
{
	LLVMTypeRef type = LLVMInt32Type();
	if (enum_decl->basic_type.has_value()) 
	type = get_basic_type(enum_decl->basic_type.value());

	bool int_kind = type_is_int(type);
	bool bool_kind = type_is_bool(type);
	bool float_kind = type_is_float(type);

	Enum_Meta meta = { enum_decl, type, {} };
	for (Ast_Ident_Literal_Pair& variant: enum_decl->variants)
	{
		LLVMValueRef enum_constant = LLVMAddGlobal(module, type, get_c_string(variant.ident.token));
		int sign = variant.is_negative ? -1 : 1; //@Issue with negative value limits, need robust range checking and ir validation of generated values
		if (int_kind) LLVMSetInitializer(enum_constant, LLVMConstInt(type, sign * variant.literal.token.integer_value, 0)); //@Todo sign extend?
		else if (bool_kind) LLVMSetInitializer(enum_constant, LLVMConstInt(type, (int)variant.literal.token.bool_value, 0));
		else if (float_kind) LLVMSetInitializer(enum_constant, LLVMConstReal(type, sign * variant.literal.token.float64_value));
		LLVMSetGlobalConstant(enum_constant, 1);
		meta.variants.emplace_back(enum_constant);
	}
	enum_decl_map.add(enum_decl->type.token.string_value, meta, hash_fnv1a_32(enum_decl->type.token.string_value));
}

//@Todo struct recursive dependencies, currently exiting when inner type not found
void LLVM_IR_Builder::build_struct_decl(Ast_Struct_Decl* struct_decl)
{
	std::vector<LLVMTypeRef> members; //@Perf deside on better member storage
	for (const Ast_Ident_Type_Pair& field : struct_decl->fields)
	{
		LLVMTypeRef type_ref = get_type_meta(field.type).type;
		members.emplace_back(type_ref);
	}

	LLVMTypeRef struct_type = LLVMStructCreateNamed(LLVMGetGlobalContext(), get_c_string(struct_decl->type.token));
	LLVMStructSetBody(struct_type, members.data(), (u32)members.size(), 0);
	Struct_Meta meta = { struct_decl, struct_type };
	struct_decl_map.add(struct_decl->type.token.string_value, meta, hash_fnv1a_32(struct_decl->type.token.string_value));
}

void LLVM_IR_Builder::build_proc_decl(Ast_Proc_Decl* proc_decl)
{
	std::vector<LLVMTypeRef> param_types = {};
	param_types.reserve(proc_decl->input_params.size());
	for (u32 i = 0; i < proc_decl->input_params.size(); i += 1)
	param_types.emplace_back(get_type_meta(proc_decl->input_params[i].type).type);

	LLVMTypeRef ret_type = LLVMVoidType();
	if (proc_decl->return_type.has_value())
	ret_type = get_type_meta(proc_decl->return_type.value()).type;

	LLVMTypeRef proc_type = LLVMFunctionType(ret_type, param_types.data(), (u32)param_types.size(), 0); //@Temp Discarding input args
	LLVMValueRef proc_val = LLVMAddFunction(module, get_c_string(proc_decl->ident.token), proc_type);

	Proc_Meta meta = { proc_type, proc_val };
	proc_decl_map.add(proc_decl->ident.token.string_value, meta, hash_fnv1a_32(proc_decl->ident.token.string_value));
}

void LLVM_IR_Builder::build_proc_body(Ast_Proc_Decl* proc_decl)
{
	if (proc_decl->is_external) return;
	auto proc_meta = proc_decl_map.find(proc_decl->ident.token.string_value, hash_fnv1a_32(proc_decl->ident.token.string_value));
	if (!proc_meta) { error_exit("failed to find proc declaration while building its body"); return; }
	
	proc_value = proc_meta.value().proc_val;
	LLVMBasicBlockRef entry_block = LLVMAppendBasicBlock(proc_value, "entry");
	set_curr_block(entry_block);

	Var_Block_Scope bc = {}; //@Perf allocating new memory for each function
	bc.add_block();

	u32 count = 0;
	for (Ast_Ident_Type_Pair& param : proc_decl->input_params)
	{
		Type_Meta var_type = get_type_meta(param.type);
		LLVMValueRef param_value = LLVMGetParam(proc_value, count);
		LLVMValueRef copy_ptr = LLVMBuildAlloca(builder, var_type.type, "copy_ptr");
		LLVMBuildStore(builder, param_value, copy_ptr);
		bc.add_var(Var_Meta{ param.ident.token.string_value, copy_ptr, var_type });
		count += 1;
	}

	Terminator_Type terminator = build_block(proc_decl->block, &bc, false);
	if (terminator == Terminator_Type::None && !proc_decl->return_type.has_value()) LLVMBuildRet(builder, NULL);
}

Terminator_Type LLVM_IR_Builder::build_block(Ast_Block* block, Var_Block_Scope* bc, bool defer, std::optional<Loop_Meta> loop_meta, bool entry)
{
	if (!entry) bc->add_block();

	for (Ast_Statement* statement : block->statements)
	{
		switch (statement->tag)
		{
			case Ast_Statement::Tag::If: 
			{
				LLVMBasicBlockRef cont_block = LLVMAppendBasicBlock(proc_value, "cont");
				build_if(statement->as_if, cont_block, bc, defer, loop_meta);
			} break;
			case Ast_Statement::Tag::For: 
			{
				build_for(statement->as_for, bc, defer); 
			} break;
			case Ast_Statement::Tag::Defer:
			{
				if (defer) error_exit("defer block cannot contain nested 'defer'");
				bc->add_defer(statement->as_defer);
			} break;
			case Ast_Statement::Tag::Break:
			{
				if (!loop_meta.has_value() && defer) error_exit("defer block cannot contain 'break'");
				build_defer(block, bc, false);

				if (!loop_meta) error_exit("break statement: no loop meta data provided");
				LLVMBuildBr(builder, loop_meta.value().break_target);
				
				bc->pop_block();
				return Terminator_Type::Break;
			} break;
			case Ast_Statement::Tag::Return:
			{
				if (defer) error_exit("defer block cannot contain 'return'");
				build_defer(block, bc, true);

				Ast_Return* _return = statement->as_return;
				if (_return->expr.has_value())
					LLVMBuildRet(builder, build_expr_value(_return->expr.value(), bc));
				else LLVMBuildRet(builder, NULL);
				
				bc->pop_block();
				return Terminator_Type::Return;
			} break;
			case Ast_Statement::Tag::Continue:
			{
				if (!loop_meta.has_value() && defer) error_exit("defer block cannot contain 'continue'");
				build_defer(block, bc, false);
				
				if (!loop_meta) error_exit("continue statement: no loop meta data provided");
				if (loop_meta.value().continue_action)
					build_var_assign(loop_meta.value().continue_action.value(), bc);
				LLVMBuildBr(builder, loop_meta.value().continue_target);

				bc->pop_block();
				return Terminator_Type::Continue;
			} break;
			case Ast_Statement::Tag::Proc_Call: build_proc_call(statement->as_proc_call, bc, true); break;
			case Ast_Statement::Tag::Var_Decl: build_var_decl(statement->as_var_decl, bc); break;
			case Ast_Statement::Tag::Var_Assign: build_var_assign(statement->as_var_assign, bc); break;
			default: break;
		}
	}

	build_defer(block, bc, false);
	bc->pop_block();
	return Terminator_Type::None;
}

void LLVM_IR_Builder::build_defer(Ast_Block* block, Var_Block_Scope* bc, bool all_defers)
{
	if (all_defers)
	{
		for (auto it = bc->defer_stack.rbegin(); it != bc->defer_stack.rend(); ++it)
		{
			Ast_Defer* defer = *it;
			build_block(defer->block, bc, true);
		}
	}
	else
	{
		u32 defer_count = bc->get_curr_defer_count();
		auto begin = bc->defer_stack.rbegin();
		auto end = std::next(begin, defer_count);
		for (auto it = begin; it != end; ++it)
		{
			Ast_Defer* defer = *it;
			build_block(defer->block, bc, true);
		}
	}
}

void LLVM_IR_Builder::build_if(Ast_If* _if, LLVMBasicBlockRef cont_block, Var_Block_Scope* bc, bool defer, std::optional<Loop_Meta> loop_meta)
{
	LLVMValueRef cond_value = build_expr_value(_if->condition_expr, bc);
	if (LLVMInt1Type() != LLVMTypeOf(cond_value)) error_exit("if: expected i1(bool) expression value");

	if (_if->_else.has_value())
	{
		LLVMBasicBlockRef then_block = LLVMAppendBasicBlock(proc_value, "then");
		LLVMBasicBlockRef else_block = LLVMAppendBasicBlock(proc_value, "else");
		LLVMBuildCondBr(builder, cond_value, then_block, else_block);
		set_curr_block(then_block);

		Terminator_Type terminator = build_block(_if->block, bc, defer, loop_meta);
		if (terminator == Terminator_Type::None) LLVMBuildBr(builder, cont_block);
		set_curr_block(else_block);

		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else::Tag::If)
		{
			build_if(_else->as_if, cont_block, bc, defer, loop_meta);
		}
		else
		{
			Terminator_Type terminator = build_block(_else->as_block, bc, defer, loop_meta);
			if (terminator == Terminator_Type::None) LLVMBuildBr(builder, cont_block);
			set_curr_block(cont_block);
		}
	}
	else
	{
		LLVMBasicBlockRef then_block = LLVMAppendBasicBlock(proc_value, "then");
		LLVMBuildCondBr(builder, cond_value, then_block, cont_block);
		set_curr_block(then_block);

		Terminator_Type terminator = build_block(_if->block, bc, defer, loop_meta);
		if (terminator == Terminator_Type::None) LLVMBuildBr(builder, cont_block);
		set_curr_block(cont_block);
	}
}

void LLVM_IR_Builder::build_for(Ast_For* _for, Var_Block_Scope* bc, bool defer)
{
	if (_for->var_decl) build_var_decl(_for->var_decl.value(), bc);
	
	LLVMBasicBlockRef cond_block = LLVMAppendBasicBlock(proc_value, "loop_cond");
	LLVMBuildBr(builder, cond_block);
	set_curr_block(cond_block);
	
	LLVMBasicBlockRef body_block = LLVMAppendBasicBlock(proc_value, "loop_body");
	LLVMBasicBlockRef exit_block = LLVMAppendBasicBlock(proc_value, "loop_exit");
	if (_for->condition_expr)
	{
		LLVMValueRef cond_value = build_expr_value(_for->condition_expr.value(), bc);
		if (LLVMInt1Type() != LLVMTypeOf(cond_value)) error_exit("if: expected i1(bool) expression value");
		LLVMBuildCondBr(builder, cond_value, body_block, exit_block);
	}
	else LLVMBuildBr(builder, body_block);
	set_curr_block(body_block);

	Terminator_Type terminator = build_block(_for->block, bc, defer, Loop_Meta { exit_block, cond_block, _for->var_assign });
	if (terminator == Terminator_Type::None)
	{
		if (_for->var_assign) build_var_assign(_for->var_assign.value(), bc);
		LLVMBuildBr(builder, cond_block);
	}
	set_curr_block(exit_block);
}

LLVMValueRef LLVM_IR_Builder::build_proc_call(Ast_Proc_Call* proc_call, Var_Block_Scope* bc, bool is_statement)
{
	std::optional<Proc_Meta> proc_meta = proc_decl_map.find(proc_call->ident.token.string_value, hash_fnv1a_32(proc_call->ident.token.string_value));
	if (!proc_meta) { error_exit("failed to find proc declaration while trying to call it"); }

	std::vector<LLVMValueRef> input_values = {};
	input_values.reserve(proc_call->input_exprs.size());
	for (u32 i = 0; i < proc_call->input_exprs.size(); i += 1)
	input_values.emplace_back(build_expr_value(proc_call->input_exprs[i], bc));

	LLVMValueRef ret_val = LLVMBuildCall2(builder, proc_meta.value().proc_type, proc_meta.value().proc_val, input_values.data(), (u32)input_values.size(), is_statement ? "" : "call_val");
	return ret_val;
}

void LLVM_IR_Builder::build_var_decl(Ast_Var_Decl* var_decl, Var_Block_Scope* bc)
{
	if (!var_decl->type.has_value()) error_exit("var decl expected type to be known");
	Type_Meta var_type = get_type_meta(var_decl->type.value());

	LLVMValueRef var_ptr = LLVMBuildAlloca(builder, var_type.type, get_c_string(var_decl->ident.token));
	if (var_decl->expr.has_value())
	{
		LLVMValueRef expr_value = build_expr_value(var_decl->expr.value(), bc);
		expr_value = build_value_cast(expr_value, var_type.type);

		if (var_type.type != LLVMTypeOf(expr_value))
		{
			debug_print_llvm_type("Expected", var_type.type);
			debug_print_llvm_type("GotExpr", LLVMTypeOf(expr_value));
			error_exit("type mismatch in variable declaration");
		}
		LLVMBuildStore(builder, expr_value, var_ptr);
	}
	else LLVMBuildStore(builder, LLVMConstNull(var_type.type), var_ptr);

	bc->add_var(Var_Meta{ var_decl->ident.token.string_value, var_ptr, var_type });
}

void LLVM_IR_Builder::build_var_assign(Ast_Var_Assign* var_assign, Var_Block_Scope* bc)
{
	if (var_assign->op != ASSIGN_OP_NONE) error_exit("var assign: only = op is supported");

	Ast_Var* var = var_assign->var;
	Var_Access_Meta var_access = get_var_access_meta(var, bc);

	LLVMValueRef expr_value = build_expr_value(var_assign->expr, bc);
	expr_value = build_value_cast(expr_value, var_access.type);

	if (var_access.type != LLVMTypeOf(expr_value))
	{
		debug_print_llvm_type("Expected", var_access.type);
		debug_print_llvm_type("GotExpr", LLVMTypeOf(expr_value));
		error_exit("type mismatch in variable assignment");
	}
	LLVMBuildStore(builder, expr_value, var_access.ptr);
}

LLVMValueRef LLVM_IR_Builder::build_expr_value(Ast_Expr* expr, Var_Block_Scope* bc, bool adress_op)
{
	LLVMValueRef value_ref = NULL;

	switch (expr->tag)
	{
	case Ast_Expr::Tag::Term:
	{
		Ast_Term* term = expr->as_term;

		switch (term->tag)
		{
		case Ast_Term::Tag::Var:
		{
			Ast_Var* var = term->as_var;
			Var_Access_Meta var_access = get_var_access_meta(var, bc);
			if (adress_op) value_ref = var_access.ptr;
			else value_ref = LLVMBuildLoad2(builder, var_access.type, var_access.ptr, "load_val");
		} break;
		case Ast_Term::Tag::Enum:
		{
			Ast_Enum* _enum = term->as_enum;
			value_ref = get_enum_value(_enum);
		} break;
		case Ast_Term::Tag::Literal:
		{
			Token token = term->as_literal.token;
			if (token.type == TOKEN_BOOL_LITERAL) value_ref = LLVMConstInt(LLVMInt1Type(), (int)token.bool_value, 0);
			else if (token.type == TOKEN_FLOAT_LITERAL) value_ref = LLVMConstReal(LLVMDoubleType(), token.float64_value);
			else if (token.type == TOKEN_INTEGER_LITERAL) value_ref = LLVMConstInt(LLVMInt32Type(), token.integer_value, 0); //@Todo sign extend?
			else error_exit("unary_expr: unknown literal type");
		} break;
		case Ast_Term::Tag::Proc_Call:
		{
			value_ref = build_proc_call(term->as_proc_call, bc, false);
		} break;
		}
	} break;
	case Ast_Expr::Tag::Unary_Expr:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		UnaryOp op = unary_expr->op;

		LLVMValueRef rhs = build_expr_value(unary_expr->right, bc, op == UNARY_OP_ADRESS_OF);
		LLVMTypeRef rhs_type = LLVMTypeOf(rhs);
		bool int_kind = type_is_int(rhs_type);
		bool bool_kind = type_is_bool(rhs_type);
		bool float_kind = type_is_float(rhs_type);
		bool pointer_kind = type_is_pointer(rhs_type);
		if (!int_kind && !bool_kind && !float_kind && !pointer_kind)
			error_exit("unary_expr: expected float int bool or pointer type");

		switch (op)
		{
		case UNARY_OP_MINUS:
		{
			if (float_kind) value_ref = LLVMBuildFNeg(builder, rhs, "utmp");
			else if (int_kind) value_ref = LLVMBuildNeg(builder, rhs, "utmp'"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
			else error_exit("unary_expr - expected fd or i");
		} break;
		case UNARY_OP_LOGIC_NOT:
		{
			if (bool_kind) value_ref = LLVMBuildNot(builder, rhs, "utmp");
			else error_exit("unary_expr ! expected bool");
		} break;
		case UNARY_OP_ADRESS_OF:
		{
			if (pointer_kind) value_ref = rhs;
			else error_exit("unary_expr & expected pointer value");
		} break;
		case UNARY_OP_BITWISE_NOT:
		{
			if (int_kind) value_ref = LLVMBuildNot(builder, rhs, "utmp"); //@Design only allow uint
			else error_exit("unary_expr ~ expected i");
		} break;
		default: error_exit("unary_expr unknown unary op"); break;
		}
	} break;
	case Ast_Expr::Tag::Binary_Expr:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		BinaryOp op = binary_expr->op;

		LLVMValueRef lhs = build_expr_value(binary_expr->left, bc);
		LLVMTypeRef lhs_type = LLVMTypeOf(lhs);
		LLVMValueRef rhs = build_expr_value(binary_expr->right, bc);
		LLVMTypeRef rhs_type = LLVMTypeOf(rhs);
		bool int_kind = type_is_int(lhs_type) && type_is_int(rhs_type);
		bool bool_kind = type_is_bool(lhs_type) && type_is_bool(rhs_type);
		bool float_kind = type_is_float(lhs_type) && type_is_float(rhs_type);
		if (!int_kind && !bool_kind && !float_kind)
			error_exit("binary_expr: expected matching float int or bool types");

		//@Incomplete for now this performs float-double upcast if nesessary
		build_binary_value_cast(lhs, rhs, lhs_type, rhs_type);

		switch (op)
		{
		// LogicOps [&& ||]
		case BINARY_OP_LOGIC_AND:
		{
			if (!bool_kind) error_exit("bin_expr && expected bool");
			value_ref = LLVMBuildAnd(builder, lhs, rhs, "btmp");
		} break;
		case BINARY_OP_LOGIC_OR:
		{
			if (!bool_kind) error_exit("bin_expr || expected bool");
			value_ref = LLVMBuildOr(builder, lhs, rhs, "btmp");
		} break;
		// CmpOps [< > <= >= == !=]
		case BINARY_OP_LESS: //@RealPredicates using ordered (no nans) variants
		{
			if (float_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOLT, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSLT, lhs, rhs, "btmp"); //@Determine S / U predicates
			else error_exit("bin_expr < expected fd or i got bool");
		} break;
		case BINARY_OP_GREATER:
		{
			if (float_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOGT, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSGT, lhs, rhs, "btmp"); //@Determine S / U predicates
			else error_exit("bin_expr > expected fd or i got bool");
		} break;
		case BINARY_OP_LESS_EQUALS:
		{
			if (float_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOLE, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSLE, lhs, rhs, "btmp"); //@Determine S / U predicates
			else error_exit("bin_expr <= expected fd or i got bool");
		} break;
		case BINARY_OP_GREATER_EQUALS:
		{
			if (float_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOGE, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSGE, lhs, rhs, "btmp"); //@Determine S / U predicates
			else error_exit("bin_expr >= expected fd or i got bool");
		} break;
		case BINARY_OP_IS_EQUALS:
		{
			if (float_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOEQ, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntEQ, lhs, rhs, "btmp"); //@Determine S / U predicates
			else error_exit("bin_expr == expected fd or i got bool");
		} break;
		case BINARY_OP_NOT_EQUALS:
		{
			if (float_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealONE, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntNE, lhs, rhs, "btmp"); //@Determine S / U predicates
			else error_exit("bin_expr != expected fd or i got bool");
		} break;
		// MathOps [+ - * / %]
		case BINARY_OP_PLUS:
		{
			if (float_kind) value_ref = LLVMBuildFAdd(builder, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildAdd(builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
			else error_exit("bin_expr + expected fd or i got bool");
		} break;
		case BINARY_OP_MINUS:
		{
			if (float_kind) value_ref = LLVMBuildFSub(builder, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildSub(builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
			else error_exit("bin_expr + expected fd or i got bool");
		} break;
		case BINARY_OP_TIMES:
		{
			if (float_kind) value_ref = LLVMBuildFMul(builder, lhs, rhs, "btmp");
			else if (int_kind) value_ref = LLVMBuildMul(builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
			else error_exit("bin_expr * expected fd or i got bool");
		} break;
		case BINARY_OP_DIV:
		{
			if (float_kind) value_ref = LLVMBuildFDiv(builder, lhs, rhs, "btmp");
			//@ SU variants: LLVMBuildSDiv, LLVMBuildExactSDiv, LLVMBuildUDiv, LLVMBuildExactUDiv
			else if (int_kind) value_ref = LLVMBuildSDiv(builder, lhs, rhs, "btmp");
			else error_exit("bin_expr / expected fd or i got bool");
		} break;
		case BINARY_OP_MOD: //@Design float modulo is possible but isnt too usefull
		{
			//@ SU rem variants: LLVMBuildSRem, LLVMBuildURem (using SRem always now)
			if (int_kind) value_ref = LLVMBuildSRem(builder, lhs, rhs, "btmp");
			else error_exit("bin_expr % expected i");
		} break;
		// BitwiseOps [& | ^ << >>]
		case BINARY_OP_BITWISE_AND: // @Design only allow those for uints ideally
		{
			if (int_kind) value_ref = LLVMBuildAnd(builder, lhs, rhs, "btmp");
			else error_exit("bin_expr & expected i");
		} break;
		case BINARY_OP_BITWISE_OR:
		{
			if (int_kind) value_ref = LLVMBuildOr(builder, lhs, rhs, "btmp");
			else error_exit("bin_expr | expected i");
		} break;
		case BINARY_OP_BITWISE_XOR:
		{
			if (int_kind) value_ref = LLVMBuildXor(builder, lhs, rhs, "btmp");
			else error_exit("bin_expr ^ expected i");
		} break;
		case BINARY_OP_BITSHIFT_LEFT:
		{
			if (int_kind) value_ref = LLVMBuildShl(builder, lhs, rhs, "btmp");
			else error_exit("bin_expr << expected i");
		} break;
		case BINARY_OP_BITSHIFT_RIGHT: //@LLVMBuildAShr used for maintaining the sign?
		{
			if (int_kind) value_ref = LLVMBuildLShr(builder, lhs, rhs, "btmp");
			else error_exit("bin_expr >> expected i");
		} break;
		default: error_exit("bin_expr unknown binary op"); break;
		}
	} break;
	}

	if (value_ref == NULL) error_exit("build_expr_value: value_ref is null on return");
	return value_ref;
}

LLVMValueRef LLVM_IR_Builder::build_value_cast(LLVMValueRef value, LLVMTypeRef target_type)
{
	LLVMTypeRef value_type = LLVMTypeOf(value);
	if (value_type == target_type) return value;

	if (type_is_float(value_type) && type_is_float(target_type))
		return LLVMBuildFPCast(builder, value, target_type, "fpcast_val");

	if (type_is_int(value_type) && type_is_int(target_type))
	{
		if (type_int_bit_witdh(value_type) < type_int_bit_witdh(target_type))
		return LLVMBuildSExt(builder, value, target_type, "icast_val"); // @Possible issue SExt might not work as expected with uints, might also work since we upcast
	}

	return value;
}

void LLVM_IR_Builder::build_binary_value_cast(LLVMValueRef& value_lhs, LLVMValueRef& value_rhs, LLVMTypeRef type_lhs, LLVMTypeRef type_rhs)
{
	if (type_lhs == type_rhs) return;

	if (type_is_float(type_lhs) && type_is_float(type_rhs))
	{
		if (type_is_f32(type_lhs))
			value_lhs = LLVMBuildFPExt(builder, value_lhs, type_rhs, "fpcast_val");
		else value_rhs = LLVMBuildFPExt(builder, value_rhs, type_lhs, "fpcast_val");
		return;
	}

	if (type_is_int(type_lhs) && type_is_int(type_rhs))
	{
		if (type_int_bit_witdh(type_lhs) < type_int_bit_witdh(type_rhs))
			value_lhs = LLVMBuildSExt(builder, value_lhs, type_rhs, "icast_val");
		else value_rhs = LLVMBuildSExt(builder, value_rhs, type_lhs, "icast_val");
		return;
	}
}

LLVMTypeRef LLVM_IR_Builder::get_basic_type(BasicType type)
{
	switch (type)
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
		//case BASIC_TYPE_STRING: @Undefined for now, might use a struct for a string
		default: error_exit("get_basic_type: basic type not found"); break;
	}

	return NULL;
}

Type_Meta LLVM_IR_Builder::get_type_meta(Ast_Type* type)
{
	LLVMTypeRef type_ref = NULL;

	switch (type->tag)
	{
	case Ast_Type::Tag::Basic:
	{
		type_ref = get_basic_type(type->as_basic);
	} break;
	case Ast_Type::Tag::Custom:
	{
		auto struct_meta = struct_decl_map.find(type->as_custom.token.string_value, hash_fnv1a_32(type->as_custom.token.string_value));
		if (struct_meta)
		{
			return Type_Meta{ struct_meta.value().struct_type, true, struct_meta.value().struct_decl, false, NULL };
		}
		
		auto enum_meta = enum_decl_map.find(type->as_custom.token.string_value, hash_fnv1a_32(type->as_custom.token.string_value));
		if (enum_meta)
		{
			return Type_Meta{ enum_meta.value().variant_type, false, NULL, false, NULL };
		}
		error_exit("get_type_meta: custom type not found");
	} break;
	case Ast_Type::Tag::Pointer:
	{
		type_ref = LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0);
		return Type_Meta{ type_ref, false, NULL, true, type->as_pointer };
	} break;
	case Ast_Type::Tag::Array:
	{
		error_exit("get_type_meta: arrays not supported");
		return Type_Meta{};
	} break;
	}

	return Type_Meta{ type_ref, false, NULL, false, NULL };
}

//@Notice enums are threated like values of their basic type,
//its possible to set enum variable or field to the value outside its variant pool
//this shouild be checked for in checker, ir sholdnt care about this semantics
LLVMValueRef LLVM_IR_Builder::get_enum_value(Ast_Enum* _enum) //@Perf copying the vector of llvm values
{
	auto enum_meta = enum_decl_map.find(_enum->type.token.string_value, hash_fnv1a_32(_enum->type.token.string_value));
	if (!enum_meta) error_exit("get_enum_variant: failed to find enum type");
	
	u32 count = 0;
	for (const Ast_Ident_Literal_Pair& variant: enum_meta.value().enum_decl->variants)
	{
		if (variant.ident.token.string_value == _enum->variant.token.string_value)
		{
			LLVMValueRef ptr = enum_meta.value().variants[count];
			return LLVMBuildLoad2(builder, enum_meta.value().variant_type, ptr, "enum_val");
		}
		count += 1;
	}
	error_exit("get_enum_variant: failed to find enum variant");
	return NULL;
}

Field_Meta LLVM_IR_Builder::get_field_meta(Ast_Struct_Decl* struct_decl, StringView field_str)
{
	u32 count = 0;
	for (const auto& field : struct_decl->fields)
	{
		if (field.ident.token.string_value == field_str)
			return Field_Meta{ count, get_type_meta(field.type) };
		count += 1;
	}
	error_exit("get_field_meta: failed to find the field");
	return Field_Meta{};
}

Var_Access_Meta LLVM_IR_Builder::get_var_access_meta(Ast_Var* var, Var_Block_Scope* bc)
{
	Var_Meta var_meta = bc->find_var(var->ident.token.string_value);
	LLVMValueRef ptr = var_meta.var_value;
	Type_Meta type_meta = var_meta.type_meta;

	Ast_Access* access = var->access.has_value() ? var->access.value() : NULL;
	while (access != NULL)
	{
		if (access->tag == Ast_Access::Tag::Array)
		{
			if (!type_meta.is_pointer) error_exit("get_var_access_meta: trying array access on non pointer variable");
			
			Ast_Array_Access* array_access = access->as_array;
			
			LLVMValueRef index_value = build_expr_value(array_access->index_expr, bc);
			type_meta = get_type_meta(type_meta.pointer_ast_type);
			ptr = LLVMBuildGEP2(builder, type_meta.type, ptr, &index_value, 1, "array_access_ptr");

			access = array_access->next.has_value() ? array_access->next.value() : NULL;
		}
		else
		{
			if (type_meta.is_pointer)
			{
				ptr = LLVMBuildLoad2(builder, type_meta.type, ptr, "ptr_load");
				type_meta = get_type_meta(type_meta.pointer_ast_type);
			}

			if (!type_meta.is_struct) 
			{
				debug_print_token(access->as_var->ident.token, true, true);
				debug_print_llvm_type("Type: ", type_meta.type);
				error_exit("get_var_access_meta: trying var access on non struct variable");
			}

			Ast_Var_Access* var_access = access->as_var;
			Field_Meta field = get_field_meta(type_meta.struct_decl, var_access->ident.token.string_value);
			ptr = LLVMBuildStructGEP2(builder, type_meta.type, ptr, field.id, "struct_access_ptr");
			type_meta = field.type_meta;

			access = var_access->next.has_value() ? var_access->next.value() : NULL;
		}
	}

	return Var_Access_Meta { ptr, type_meta.type };
}

bool LLVM_IR_Builder::type_is_int(LLVMTypeRef type) { return LLVMGetTypeKind(type) == LLVMIntegerTypeKind && LLVMGetIntTypeWidth(type) != 1; }
bool LLVM_IR_Builder::type_is_bool(LLVMTypeRef type) { return LLVMGetTypeKind(type) == LLVMIntegerTypeKind && LLVMGetIntTypeWidth(type) == 1; }
bool LLVM_IR_Builder::type_is_float(LLVMTypeRef type) { return LLVMGetTypeKind(type) == LLVMFloatTypeKind || LLVMGetTypeKind(type) == LLVMDoubleTypeKind; }
bool LLVM_IR_Builder::type_is_f32(LLVMTypeRef type) { return LLVMGetTypeKind(type) == LLVMFloatTypeKind; }
bool LLVM_IR_Builder::type_is_f64(LLVMTypeRef type) { return LLVMGetTypeKind(type) == LLVMDoubleTypeKind; }
bool LLVM_IR_Builder::type_is_pointer(LLVMTypeRef type) { return LLVMGetTypeKind(type) == LLVMPointerTypeKind; }
u32 LLVM_IR_Builder::type_int_bit_witdh(LLVMTypeRef type) { return LLVMGetIntTypeWidth(type); }

char* LLVM_IR_Builder::get_c_string(Token& token) //@Unsafe hack to get c string from string view of source file string, need to do smth better
{
	token.string_value.data[token.string_value.count] = 0;
	return (char*)token.string_value.data;
}

void LLVM_IR_Builder::error_exit(const char* message)
{
	printf("backend error: %s.\n", message);
	exit(EXIT_FAILURE);
}

void LLVM_IR_Builder::debug_print_llvm_type(const char* message, LLVMTypeRef type)
{
	char* msg = LLVMPrintTypeToString(type);
	printf("%s %s\n", message, msg);
	LLVMDisposeMessage(msg);
}

void LLVM_IR_Builder::set_curr_block(LLVMBasicBlockRef block)
{
	LLVMPositionBuilderAtEnd(builder, block);
}
