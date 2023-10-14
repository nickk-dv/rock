#include "llvm_ir_builder_2.h"

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
			if (basic_type <= BASIC_TYPE_U64) variant.constant = LLVMConstInt(type, variant.literal.token.integer_value, basic_type % 2 == 0); //@Check if sign extend is correct or needed
			else if (basic_type <= BASIC_TYPE_F64) variant.constant = LLVMConstReal(type, variant.literal.token.float64_value);
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
		char* name = proc_decl->is_external ? ident_to_cstr(proc_decl->ident) : "proc";
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
		u32 count = 0;
		for (Ast_Ident_Type_Pair& param : proc_decl->input_params)
		{
			LLVMTypeRef type = type_to_llvm_type(&context, param.type);
			LLVMValueRef param_value = LLVMGetParam(proc_meta.proc_value, count);
			LLVMValueRef copy_ptr = LLVMBuildAlloca(context.builder, type, "copy_ptr");
			LLVMBuildStore(context.builder, param_value, copy_ptr);
			//@Notice need to add to block stack. bc.add_var(Var_Meta{ param.ident.str, copy_ptr, var_type });
			count += 1;
		}
		build_block(&context, &bc, proc_decl->block, BlockFlags::DisableBlockAdd);
	}

	build_context_deinit(&context);

	LLVMDumpModule(context.module);
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

IR_Loop_Info block_stack_get_loop(IR_Block_Stack* bc)
{
	return bc->loop_stack[bc->loop_stack.size() - 1];
}

Terminator build_block(IR_Context* context, IR_Block_Stack* bc, Ast_Block* block, BlockFlags flags)
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
			Terminator terminator = build_block(context, bc, statement->as_block, BlockFlags::None);
			if (terminator != Terminator::None)
			{
				build_defer(context, bc, terminator);
				block_stack_pop_back(bc);
				return terminator;
			}
		} break;
		case Ast_Statement::Tag::Defer: block_stack_add_defer(bc, statement->as_defer); break;
		case Ast_Statement::Tag::Break:
		{
			build_defer(context, bc, Terminator::Break);
			IR_Loop_Info loop = block_stack_get_loop(bc);
			LLVMBuildBr(context->builder, loop.break_block);
			
			block_stack_pop_back(bc);
			return Terminator::Break;
		} break;
		case Ast_Statement::Tag::Return:
		{
			build_defer(context, bc, Terminator::Return);
			if (statement->as_return->expr)
				LLVMBuildRet(context->builder, build_expr(context, bc, statement->as_return->expr.value()));
			else LLVMBuildRetVoid(context->builder);
			
			block_stack_pop_back(bc);
			return Terminator::Return;
		} break;
		case Ast_Statement::Tag::Continue:
		{
			build_defer(context, bc, Terminator::Continue);
			IR_Loop_Info loop = block_stack_get_loop(bc);
			if (loop.var_assign) build_var_assign(context, bc, loop.var_assign.value());
			LLVMBuildBr(context->builder, loop.continue_block);

			block_stack_pop_back(bc);
			return Terminator::Continue;
		} break;
		case Ast_Statement::Tag::Var_Decl: build_var_decl(context, bc, statement->as_var_decl); break;
		case Ast_Statement::Tag::Var_Assign: build_var_assign(context, bc, statement->as_var_assign); break;
		case Ast_Statement::Tag::Proc_Call: build_proc_call(context, bc, statement->as_proc_call, ProcCallFlags::AsStatement); break;
		}
	}

	build_defer(context, bc, Terminator::None);
	block_stack_pop_back(bc);
	return Terminator::None;
}

void build_defer(IR_Context* context, IR_Block_Stack* bc, Terminator terminator)
{
	IR_Block_Info block_info = bc->blocks[bc->blocks.size() - 1];
	int start_defer_id = bc->defer_stack.size() - 1;
	int end_defer_id = terminator == Terminator::Return ? 0 : start_defer_id - block_info.defer_count;
	
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

		Terminator terminator = build_block(context, bc, _if->block, BlockFlags::None);
		if (terminator == Terminator::None) LLVMBuildBr(context->builder, cont_block);
		set_bb(context, else_block);

		Ast_Else* _else = _if->_else.value();
		if (_else->tag == Ast_Else::Tag::If)
		{
			build_if(context, bc, _else->as_if, cont_block);
		}
		else
		{
			Terminator else_terminator = build_block(context, bc, _else->as_block, BlockFlags::None);
			if (else_terminator == Terminator::None) LLVMBuildBr(context->builder, cont_block);
			set_bb(context, cont_block);
		}
	}
	else
	{
		LLVMBasicBlockRef then_block = add_bb(bc, "then");
		LLVMBuildCondBr(context->builder, cond_value, then_block, cont_block);
		set_bb(context, then_block);

		Terminator terminator = build_block(context, bc, _if->block, BlockFlags::None);
		if (terminator == Terminator::None) LLVMBuildBr(context->builder, cont_block);
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
	Terminator terminator = build_block(context, bc, _for->block, BlockFlags::DisableBlockAdd);
	if (terminator == Terminator::None)
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

	//bc->add_var(Var_Meta{ var_decl->ident.str, var_ptr, var_type });
}

void build_var_assign(IR_Context* context, IR_Block_Stack* bc, Ast_Var_Assign* var_assign)
{
	Ast_Var* var = var_assign->var;
	//Var_Access_Meta var_access = get_var_access_meta(var, bc);

	LLVMValueRef expr_value = build_expr(context, bc, var_assign->expr);
	//expr_value = build_value_cast(expr_value, var_access.type);

	//LLVMBuildStore(context->builder, expr_value, var_access.ptr);
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
		return NULL;
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

LLVMValueRef build_unary_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Unary_Expr* unary_expr)
{
	return NULL;
}

LLVMValueRef build_binary_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Binary_Expr* binary_expr)
{
	return NULL;
}
