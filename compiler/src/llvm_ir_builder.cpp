#include "llvm_ir_builder.h"

#include "llvm-c/Core.h"

LLVMModuleRef build_module(Ast_Program* program)
{
	IR_Context context = build_context_init(program);

	for (Ast_Enum_Meta& enum_meta : program->enums)
	{
		BasicType basic_type = BASIC_TYPE_I32; //@Notice maybe store i32 at checking stage
		if (enum_meta.enum_decl->basic_type) basic_type = enum_meta.enum_decl->basic_type.value();
		LLVMTypeRef type = basic_type_to_llvm_type(basic_type);
		enum_meta.enum_type = type;

		for (Ast_Ident_Literal_Pair& variant : enum_meta.enum_decl->variants)
		{
			int sign = variant.is_negative ? -1 : 1;
			if (basic_type <= BASIC_TYPE_U64) variant.constant = LLVMConstInt(type, sign * variant.literal.token.integer_value, basic_type % 2 == 0); //@Check if sign extend is correct or needed
			else if (basic_type <= BASIC_TYPE_F64) variant.constant = LLVMConstReal(type, sign * variant.literal.token.float64_value);
			else variant.constant = LLVMConstInt(type, (int)variant.literal.token.bool_value, 0);
		}
	}

	std::vector<LLVMTypeRef> type_array(32);
	
	for (Ast_Struct_Meta& struct_meta : program->structs)
	{
		struct_meta.struct_type = LLVMStructCreateNamed(LLVMGetGlobalContext(), "struct");
	}

	for (Ast_Struct_Meta& struct_meta : program->structs)
	{
		type_array.clear();
		Ast_Struct_Decl* struct_decl = struct_meta.struct_decl;
		for (Ast_Ident_Type_Pair& field : struct_decl->fields) 
		type_array.emplace_back(type_to_llvm_type(&context, field.type));
		
		LLVMStructSetBody(struct_meta.struct_type, type_array.data(), (u32)type_array.size(), 0);

		LLVMAddGlobal(context.module, struct_meta.struct_type, "global_test");
	}

	for (Ast_Proc_Meta& proc_meta : program->procedures)
	{
		type_array.clear();
		Ast_Proc_Decl* proc_decl = proc_meta.proc_decl;
		for (Ast_Ident_Type_Pair& param : proc_decl->input_params) 
		type_array.emplace_back(type_to_llvm_type(&context, param.type));

		LLVMTypeRef ret_type = proc_decl->return_type ? type_to_llvm_type(&context, proc_decl->return_type.value()) : LLVMVoidType();
		char* name = (proc_decl->is_external || proc_decl->is_main) ? ident_to_cstr(proc_decl->ident) : "proc";
		proc_meta.proc_type = LLVMFunctionType(ret_type, type_array.data(), (u32)type_array.size(), 0);
		proc_meta.proc_value = LLVMAddFunction(context.module, name, proc_meta.proc_type);
	}

	IR_Block_Stack bc = {};
	for (Ast_Proc_Meta& proc_meta : program->procedures)
	{
		Ast_Proc_Decl* proc_decl = proc_meta.proc_decl;
		if (proc_decl->is_external) continue;

		block_stack_reset(&bc, proc_meta.proc_value);
		block_stack_add(&bc);
		LLVMBasicBlockRef entry_block = add_bb(&bc, "entry");
		set_bb(&context, entry_block);
		u32 count = 0;
		for (Ast_Ident_Type_Pair& param : proc_decl->input_params)
		{
			LLVMTypeRef type = type_to_llvm_type(&context, param.type);
			LLVMValueRef param_value = LLVMGetParam(proc_meta.proc_value, count);
			LLVMValueRef copy_ptr = LLVMBuildAlloca(context.builder, type, "copy_ptr");
			LLVMBuildStore(context.builder, param_value, copy_ptr);
			block_stack_add_var(&bc, IR_Var_Info { param.ident.str, copy_ptr, type, param.type });
			count += 1;
		}
		build_block(&context, &bc, proc_decl->block, BlockFlags::DisableBlockAdd);
	}

	build_context_deinit(&context);

	return context.module;
}

IR_Context build_context_init(Ast_Program* program)
{
	IR_Context context = {};
	context.program = program;
	context.builder = LLVMCreateBuilder();
	context.module = LLVMModuleCreateWithName("program");
	return context;
}

void build_context_deinit(IR_Context* context)
{
	LLVMDisposeBuilder(context->builder);
}

char* ident_to_cstr(Ast_Ident& ident)
{
	ident.str.data[ident.str.count] = '\0';
	return (char*)ident.str.data;
}

LLVMBasicBlockRef add_bb(IR_Block_Stack* bc, const char* name)
{
	return LLVMAppendBasicBlock(bc->proc_value, name);
}

void set_bb(IR_Context* context, LLVMBasicBlockRef block)
{
	LLVMPositionBuilderAtEnd(context->builder, block);
}

LLVMTypeRef basic_type_to_llvm_type(BasicType basic_type)
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

LLVMTypeRef type_to_llvm_type(IR_Context* context, Ast_Type type)
{
	if (type.pointer_level > 0) return LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0);

	switch (type.tag)
	{
	case Ast_Type::Tag::Basic: return basic_type_to_llvm_type(type.as_basic);
	case Ast_Type::Tag::Array: return LLVMVoidType(); //@Notice array type isnt supported
	case Ast_Type::Tag::Struct: return context->program->structs[type.as_struct.struct_id].struct_type;
	case Ast_Type::Tag::Enum: return context->program->enums[type.as_enum.enum_id].enum_type;
	default: return LLVMVoidType();
	}
}

void block_stack_reset(IR_Block_Stack* bc, LLVMValueRef proc_value)
{
	bc->proc_value = proc_value;
	bc->blocks.clear();
	bc->defer_stack.clear();
	bc->loop_stack.clear();
}

void block_stack_add(IR_Block_Stack* bc)
{
	bc->blocks.emplace_back(IR_Block_Info { 0, 0 });
}

void block_stack_pop_back(IR_Block_Stack* bc)
{
	IR_Block_Info block_info = bc->blocks[bc->blocks.size() - 1];
	for (u32 i = 0; i < block_info.defer_count; i += 1) bc->defer_stack.pop_back();
	for (u32 i = 0; i < block_info.loop_count; i += 1) bc->loop_stack.pop_back();
	for (u32 i = 0; i < block_info.var_count; i += 1) bc->var_stack.pop_back();
	bc->blocks.pop_back();
}

void block_stack_add_defer(IR_Block_Stack* bc, Ast_Defer* defer)
{
	bc->blocks[bc->blocks.size() - 1].defer_count += 1;
	bc->defer_stack.emplace_back(defer);
}

void block_stack_add_loop(IR_Block_Stack* bc, IR_Loop_Info loop_info)
{
	bc->blocks[bc->blocks.size() - 1].loop_count += 1;
	bc->loop_stack.emplace_back(loop_info);
}

void block_stack_add_var(IR_Block_Stack* bc, IR_Var_Info var_info)
{
	bc->blocks[bc->blocks.size() - 1].var_count += 1;
	bc->var_stack.emplace_back(var_info);
}

IR_Var_Info block_stack_find_var(IR_Block_Stack* bc, Ast_Ident ident)
{
	for (IR_Var_Info& var : bc->var_stack)
	{
		if (match_string_view(var.str, ident.str)) return var;
	}
	return {};
}

IR_Loop_Info block_stack_get_loop(IR_Block_Stack* bc)
{
	return bc->loop_stack[bc->loop_stack.size() - 1];
}

Terminator2 build_block(IR_Context* context, IR_Block_Stack* bc, Ast_Block* block, BlockFlags flags)
{
	if (flags != BlockFlags::DisableBlockAdd) block_stack_add(bc);

	for (Ast_Statement* statement : block->statements)
	{
		switch (statement->tag)
		{
		case Ast_Statement::Tag::If: build_if(context, bc, statement->as_if, add_bb(bc, "cont")); break;
		case Ast_Statement::Tag::For: build_for(context, bc, statement->as_for); break;
		case Ast_Statement::Tag::Block:
		{
			Terminator2 terminator = build_block(context, bc, statement->as_block, BlockFlags::None);
			if (terminator != Terminator2::None)
			{
				build_defer(context, bc, terminator);
				block_stack_pop_back(bc);
				return terminator;
			}
		} break;
		case Ast_Statement::Tag::Defer: block_stack_add_defer(bc, statement->as_defer); break;
		case Ast_Statement::Tag::Break:
		{
			build_defer(context, bc, Terminator2::Break);
			IR_Loop_Info loop = block_stack_get_loop(bc);
			LLVMBuildBr(context->builder, loop.break_block);
			
			block_stack_pop_back(bc);
			return Terminator2::Break;
		} break;
		case Ast_Statement::Tag::Return:
		{
			build_defer(context, bc, Terminator2::Return);
			if (statement->as_return->expr)
				LLVMBuildRet(context->builder, build_expr(context, bc, statement->as_return->expr.value()));
			else LLVMBuildRetVoid(context->builder);
			
			block_stack_pop_back(bc);
			return Terminator2::Return;
		} break;
		case Ast_Statement::Tag::Continue:
		{
			build_defer(context, bc, Terminator2::Continue);
			IR_Loop_Info loop = block_stack_get_loop(bc);
			if (loop.var_assign) build_var_assign(context, bc, loop.var_assign.value());
			LLVMBuildBr(context->builder, loop.continue_block);

			block_stack_pop_back(bc);
			return Terminator2::Continue;
		} break;
		case Ast_Statement::Tag::Var_Decl: build_var_decl(context, bc, statement->as_var_decl); break;
		case Ast_Statement::Tag::Var_Assign: build_var_assign(context, bc, statement->as_var_assign); break;
		case Ast_Statement::Tag::Proc_Call: build_proc_call(context, bc, statement->as_proc_call, ProcCallFlags::AsStatement); break;
		}
	}

	build_defer(context, bc, Terminator2::None);
	block_stack_pop_back(bc);
	return Terminator2::None;
}

void build_defer(IR_Context* context, IR_Block_Stack* bc, Terminator2 terminator)
{
	IR_Block_Info block_info = bc->blocks[bc->blocks.size() - 1];
	int start_defer_id = bc->defer_stack.size() - 1;
	int end_defer_id = terminator == Terminator2::Return ? 0 : start_defer_id - (block_info.defer_count - 1); //@Todo confusing indices fix later
	
	for (int i = start_defer_id; i >= end_defer_id; i -= 1) 
	build_block(context, bc, bc->defer_stack[i]->block, BlockFlags::None);
}

void build_if(IR_Context* context, IR_Block_Stack* bc, Ast_If* _if, LLVMBasicBlockRef cont_block)
{
	LLVMValueRef cond_value = build_expr(context, bc, _if->condition_expr);

	if (_if->_else)
	{
		LLVMBasicBlockRef then_block = add_bb(bc, "then");
		LLVMBasicBlockRef else_block = add_bb(bc, "else");
		LLVMBuildCondBr(context->builder, cond_value, then_block, else_block);
		set_bb(context, then_block);

		Terminator2 terminator = build_block(context, bc, _if->block, BlockFlags::None);
		if (terminator == Terminator2::None) LLVMBuildBr(context->builder, cont_block);
		set_bb(context, else_block);

		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else::Tag::If)
		{
			build_if(context, bc, _else->as_if, cont_block);
		}
		else
		{
			Terminator2 else_terminator = build_block(context, bc, _else->as_block, BlockFlags::None);
			if (else_terminator == Terminator2::None) LLVMBuildBr(context->builder, cont_block);
			set_bb(context, cont_block);
		}
	}
	else
	{
		LLVMBasicBlockRef then_block = add_bb(bc, "then");
		LLVMBuildCondBr(context->builder, cond_value, then_block, cont_block);
		set_bb(context, then_block);

		Terminator2 terminator = build_block(context, bc, _if->block, BlockFlags::None);
		if (terminator == Terminator2::None) LLVMBuildBr(context->builder, cont_block);
		set_bb(context, cont_block);
	}
}

void build_for(IR_Context* context, IR_Block_Stack* bc, Ast_For* _for)
{
	if (_for->var_decl) build_var_decl(context, bc, _for->var_decl.value());

	LLVMBasicBlockRef cond_block = add_bb(bc, "loop_cond");
	LLVMBuildBr(context->builder, cond_block);
	set_bb(context, cond_block);

	LLVMBasicBlockRef body_block = add_bb(bc, "loop_body");
	LLVMBasicBlockRef exit_block = add_bb(bc, "loop_exit");
	if (_for->condition_expr)
	{
		LLVMValueRef cond_value = build_expr(context, bc, _for->condition_expr.value());
		LLVMBuildCondBr(context->builder, cond_value, body_block, exit_block);
	}
	else LLVMBuildBr(context->builder, body_block);
	set_bb(context, body_block);

	block_stack_add(bc);
	block_stack_add_loop(bc, IR_Loop_Info { exit_block, cond_block, _for->var_assign });
	Terminator2 terminator = build_block(context, bc, _for->block, BlockFlags::DisableBlockAdd);
	if (terminator == Terminator2::None)
	{
		if (_for->var_assign) build_var_assign(context, bc, _for->var_assign.value());
		LLVMBuildBr(context->builder, cond_block);
	}
	set_bb(context, exit_block);
}

void build_var_decl(IR_Context* context, IR_Block_Stack* bc, Ast_Var_Decl* var_decl)
{
	LLVMTypeRef type = type_to_llvm_type(context, var_decl->type.value());

	LLVMValueRef var_ptr = LLVMBuildAlloca(context->builder, type, ident_to_cstr(var_decl->ident));
	if (var_decl->expr)
	{
		LLVMValueRef expr_value = build_expr(context, bc, var_decl->expr.value());
		//expr_value = build_value_cast(expr_value, type);
		LLVMBuildStore(context->builder, expr_value, var_ptr);
	}
	else LLVMBuildStore(context->builder, LLVMConstNull(type), var_ptr);

	block_stack_add_var(bc, IR_Var_Info { var_decl->ident.str, var_ptr, type, var_decl->type.value() });
}

void build_var_assign(IR_Context* context, IR_Block_Stack* bc, Ast_Var_Assign* var_assign)
{
	IR_Var_Access_Info var_access = build_var(context, bc, var_assign->var);
	LLVMValueRef expr_value = build_expr(context, bc, var_assign->expr);
	//expr_value = build_value_cast(expr_value, var_access.var_type);
	LLVMBuildStore(context->builder, expr_value, var_access.var_ptr);
}

LLVMValueRef build_proc_call(IR_Context* context, IR_Block_Stack* bc, Ast_Proc_Call* proc_call, ProcCallFlags flags)
{
	Ast_Proc_Meta proc_meta = context->program->procedures[proc_call->proc_id];

	std::vector<LLVMValueRef> input_values = {}; //@Perf memory overhead
	input_values.reserve(proc_call->input_exprs.size());
	for (Ast_Expr* expr : proc_call->input_exprs)
	input_values.emplace_back(build_expr(context, bc, expr));

	return LLVMBuildCall2(context->builder, proc_meta.proc_type, proc_meta.proc_value, 
	input_values.data(), (u32)input_values.size(), flags == ProcCallFlags::AsStatement ? "" : "call_val");
}

LLVMValueRef build_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Expr* expr)
{
	switch (expr->tag)
	{
	case Ast_Expr::Tag::Term: return build_term(context, bc, expr->as_term);
	case Ast_Expr::Tag::Unary_Expr: return build_unary_expr(context, bc, expr->as_unary_expr);
	case Ast_Expr::Tag::Binary_Expr: return build_binary_expr(context, bc, expr->as_binary_expr);
	}
}

LLVMValueRef build_term(IR_Context* context, IR_Block_Stack* bc, Ast_Term* term)
{
	switch (term->tag)
	{
	case Ast_Term::Tag::Var: 
	{	
		//@Todo handle unary adress op by returning ptr without load
		IR_Var_Access_Info var_access = build_var(context, bc, term->as_var);
		return LLVMBuildLoad2(context->builder, var_access.var_type, var_access.var_ptr, "load_val");
	}
	case Ast_Term::Tag::Enum:
	{
		Ast_Enum* _enum = term->as_enum;
		return context->program->enums[_enum->enum_id].enum_decl->variants[_enum->variant_id].constant;
	}
	case Ast_Term::Tag::Literal:
	{
		Token token = term->as_literal.token;
		if (token.type == TOKEN_BOOL_LITERAL) return LLVMConstInt(LLVMInt1Type(), (int)token.bool_value, 0);
		else if (token.type == TOKEN_FLOAT_LITERAL) return LLVMConstReal(LLVMDoubleType(), token.float64_value);
		else if (token.type == TOKEN_INTEGER_LITERAL) return LLVMConstInt(LLVMInt32Type(), token.integer_value, 0); //@Todo sign extend?
		else return LLVMConstInt(LLVMInt32Type(), 0, 0); //@Notice string literal isnt supported
	}
	case Ast_Term::Tag::Proc_Call: return build_proc_call(context, bc, term->as_proc_call, ProcCallFlags::None);
	}
}

IR_Var_Access_Info build_var(IR_Context* context, IR_Block_Stack* bc, Ast_Var* var)
{
	IR_Var_Info var_info = block_stack_find_var(bc, var->ident);
	LLVMValueRef ptr = var_info.var_ptr;
	LLVMTypeRef type = var_info.var_type;
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
				ptr = LLVMBuildLoad2(context->builder, LLVMPointerTypeInContext(LLVMGetGlobalContext(), 0), ptr, "ptr_load");
				ast_type.pointer_level -= 1;
			}

			Ast_Var_Access* var_access = access->as_var;
			Ast_Struct_Meta struct_meta = context->program->structs[ast_type.as_struct.struct_id];
			ptr = LLVMBuildStructGEP2(context->builder, struct_meta.struct_type, ptr, var_access->field_id, "struct_ptr");
			ast_type = struct_meta.struct_decl->fields[var_access->field_id].type;
			
			access = var_access->next.has_value() ? var_access->next.value() : NULL;
		}
	}

	return IR_Var_Access_Info { ptr, type_to_llvm_type(context, ast_type) };
}

LLVMValueRef build_unary_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Unary_Expr* unary_expr)
{
	UnaryOp op = unary_expr->op;
	LLVMValueRef rhs = build_expr(context, bc, unary_expr->right);

	switch (op)
	{
	case UNARY_OP_MINUS:
	{
		LLVMTypeRef rhs_type = LLVMTypeOf(rhs);
		if (LLVMGetTypeKind(rhs_type) == LLVMFloatTypeKind) return LLVMBuildFNeg(context->builder, rhs, "utmp");
		else return LLVMBuildNeg(context->builder, rhs, "utmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	}
	case UNARY_OP_LOGIC_NOT: return LLVMBuildNot(context->builder, rhs, "utmp");
	case UNARY_OP_ADDRESS_OF: return rhs;
	case UNARY_OP_BITWISE_NOT: return LLVMBuildNot(context->builder, rhs, "utmp");
	}
}

LLVMValueRef build_binary_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Binary_Expr* binary_expr)
{
	BinaryOp op = binary_expr->op;
	LLVMValueRef lhs = build_expr(context, bc, binary_expr->left);
	LLVMValueRef rhs = build_expr(context, bc, binary_expr->right);
	LLVMTypeRef lhs_type = LLVMTypeOf(lhs);
	LLVMTypeRef rhs_type = LLVMTypeOf(rhs);
	bool float_kind = LLVMGetTypeKind(lhs_type) == LLVMFloatTypeKind;

	//build_binary_value_cast(lhs, rhs, lhs_type, rhs_type);

	switch (op)
	{
	// LogicOps [&& ||]
	case BINARY_OP_LOGIC_AND: return LLVMBuildAnd(context->builder, lhs, rhs, "btmp");
	case BINARY_OP_LOGIC_OR: return LLVMBuildOr(context->builder, lhs, rhs, "btmp");
	// CmpOps [< > <= >= == !=] //@RealPredicates using ordered (no nans) variants
	case BINARY_OP_LESS:           if (float_kind) return LLVMBuildFCmp(context->builder, LLVMRealOLT, lhs, rhs, "btmp"); else return LLVMBuildICmp(context->builder, LLVMIntSLT, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_GREATER:        if (float_kind) return LLVMBuildFCmp(context->builder, LLVMRealOGT, lhs, rhs, "btmp"); else return LLVMBuildICmp(context->builder, LLVMIntSGT, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_LESS_EQUALS:    if (float_kind) return LLVMBuildFCmp(context->builder, LLVMRealOLE, lhs, rhs, "btmp"); else return LLVMBuildICmp(context->builder, LLVMIntSLE, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_GREATER_EQUALS: if (float_kind) return LLVMBuildFCmp(context->builder, LLVMRealOGE, lhs, rhs, "btmp"); else return LLVMBuildICmp(context->builder, LLVMIntSGE, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_IS_EQUALS:      if (float_kind) return LLVMBuildFCmp(context->builder, LLVMRealOEQ, lhs, rhs, "btmp"); else return LLVMBuildICmp(context->builder, LLVMIntEQ, lhs, rhs, "btmp"); //@Determine S / U predicates
	case BINARY_OP_NOT_EQUALS:     if (float_kind) return LLVMBuildFCmp(context->builder, LLVMRealONE, lhs, rhs, "btmp"); else return LLVMBuildICmp(context->builder, LLVMIntNE, lhs, rhs, "btmp"); //@Determine S / U predicates
	// MathOps [+ - * / %]
	case BINARY_OP_PLUS:  if (float_kind) return LLVMBuildFAdd(context->builder, lhs, rhs, "btmp"); else return LLVMBuildAdd(context->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BINARY_OP_MINUS: if (float_kind) return LLVMBuildFSub(context->builder, lhs, rhs, "btmp"); else return LLVMBuildSub(context->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BINARY_OP_TIMES: if (float_kind) return LLVMBuildFMul(context->builder, lhs, rhs, "btmp"); else return LLVMBuildMul(context->builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
	case BINARY_OP_DIV:   if (float_kind) return LLVMBuildFDiv(context->builder, lhs, rhs, "btmp"); else return LLVMBuildSDiv(context->builder, lhs, rhs, "btmp"); //@ SU variants: LLVMBuildSDiv, LLVMBuildExactSDiv, LLVMBuildUDiv, LLVMBuildExactUDiv
	case BINARY_OP_MOD: return LLVMBuildSRem(context->builder, lhs, rhs, "btmp"); //@ SU rem variants: LLVMBuildSRem, LLVMBuildURem (using SRem always now)
	// BitwiseOps [& | ^ << >>]
	case BINARY_OP_BITWISE_AND: return LLVMBuildAnd(context->builder, lhs, rhs, "btmp"); // @Design only allow those for uints ideally
	case BINARY_OP_BITWISE_OR: return LLVMBuildOr(context->builder, lhs, rhs, "btmp");
	case BINARY_OP_BITWISE_XOR: return LLVMBuildXor(context->builder, lhs, rhs, "btmp");
	case BINARY_OP_BITSHIFT_LEFT: return LLVMBuildShl(context->builder, lhs, rhs, "btmp");
	case BINARY_OP_BITSHIFT_RIGHT: return LLVMBuildLShr(context->builder, lhs, rhs, "btmp"); //@LLVMBuildAShr used for maintaining the sign?
	}
}
