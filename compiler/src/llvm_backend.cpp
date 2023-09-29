#include "llvm_backend.h"

#include "llvm-c/TargetMachine.h"

void Backend_LLVM::backend_build(Ast* ast)
{
	context = LLVMContextCreate();
	module = LLVMModuleCreateWithNameInContext("module", context);
	builder = LLVMCreateBuilderInContext(context);
	build_ir(ast);
	build_binaries();
}

void Backend_LLVM::build_ir(Ast* ast)
{
	for (Ast_Enum_Decl* enum_decl : ast->enums) { build_enum_decl(enum_decl); }
	struct_decl_map.init(32);
	for (Ast_Struct_Decl* struct_decl : ast->structs) { build_struct_decl(struct_decl); }
	proc_decl_map.init(32);
	for (Ast_Proc_Decl* proc_decl : ast->procs) { build_proc_decl(proc_decl); }
	for (Ast_Proc_Decl* proc_decl : ast->procs) { build_proc_body(proc_decl); }
	LLVMDisposeBuilder(builder);
}

void Backend_LLVM::build_enum_decl(Ast_Enum_Decl* enum_decl)
{
	for (u32 i = 0; i < enum_decl->variants.size(); i++)
	{
		LLVMValueRef enum_constant = LLVMAddGlobal(module, LLVMInt32TypeInContext(context), get_c_string(enum_decl->variants[i].token));
		LLVMSetInitializer(enum_constant, LLVMConstInt(LLVMInt32TypeInContext(context), enum_decl->constants[i], 0));
		LLVMSetGlobalConstant(enum_constant, 1);
	}
}

void Backend_LLVM::build_struct_decl(Ast_Struct_Decl* struct_decl)
{
	LLVMTypeRef struct_type = LLVMStructCreateNamed(context, get_c_string(struct_decl->type.token));
	std::vector<LLVMTypeRef> members; //@Perf decide on better member storage

	for (const Ast_Ident_Type_Pair& field : struct_decl->fields)
	{
		Ast_Type* type = field.type;
		LLVMTypeRef type_ref = NULL;

		switch (type->tag)
		{
			case Ast_Type::Tag::Basic:
			{
				type_ref = basic_type_convert(type->as_basic);
			} break;
			case Ast_Type::Tag::Custom:
			{
				error_exit("struct field custom types not supported");
			} break;
			case Ast_Type::Tag::Pointer:
			{
				error_exit("struct field pointer types not supported");
			} break;
			case Ast_Type::Tag::Array:
			{
				error_exit("struct field arrays not supported");
			} break;
		}

		members.emplace_back(type_ref);
	}

	LLVMStructSetBody(struct_type, members.data(), (u32)members.size(), 0);
	Struct_Meta meta = { struct_decl, struct_type };
	struct_decl_map.add(struct_decl->type.token.string_value, meta, hash_fnv1a_32(struct_decl->type.token.string_value));

	//@Hack adding global var to see the struct declaration in the ir
	LLVMValueRef globalVar = LLVMAddGlobal(module, struct_type, "_global_struct_check");
}

// @Usefull functions:
// LLVMTypeRef LLVMTypeOf(LLVMValueRef Val);
// LLVMGetAllocatedType - get type of value on a stack

void Backend_LLVM::build_proc_decl(Ast_Proc_Decl* proc_decl)
{
	LLVMTypeRef ret_type = LLVMVoidType();
	if (proc_decl->return_type.has_value())
	{
		Ast_Type* type = proc_decl->return_type.value();
		switch (type->tag)
		{
			case Ast_Type::Tag::Basic:
			{
				ret_type = basic_type_convert(type->as_basic);
			} break;
			case Ast_Type::Tag::Custom:
			{
				auto struct_meta = struct_decl_map.find(type->as_custom.token.string_value, hash_fnv1a_32(type->as_custom.token.string_value));
				if (!struct_meta) error_exit("proc decl return custom type not found");
				ret_type = struct_meta.value().struct_type;
			} break;
			case Ast_Type::Tag::Pointer:
			{
				error_exit("proc decl return pointer types not supported");
			} break;
			case Ast_Type::Tag::Array:
			{
				error_exit("proc decl return arrays not supported");
			} break;
		}
	}
	if (!proc_decl->input_params.empty()) error_exit("procedure declaration with input params isnt supported");
	
	LLVMTypeRef proc_type = LLVMFunctionType(ret_type, NULL, 0, 0); //@Temp Discarding input args
	LLVMValueRef proc_val = LLVMAddFunction(module, get_c_string(proc_decl->ident.token), proc_type);
	Proc_Meta meta = { proc_type, proc_val };
	proc_decl_map.add(proc_decl->ident.token.string_value, meta, hash_fnv1a_32(proc_decl->ident.token.string_value));
}

void Backend_LLVM::build_proc_body(Ast_Proc_Decl* proc_decl)
{
	auto proc_meta = proc_decl_map.find(proc_decl->ident.token.string_value, hash_fnv1a_32(proc_decl->ident.token.string_value));
	if (!proc_meta) { error_exit("failed to find proc declaration while building its body"); return; }
	LLVMBasicBlockRef entry_block = LLVMAppendBasicBlockInContext(context, proc_meta->proc_val, "entry");
	LLVMPositionBuilderAtEnd(builder, entry_block);
	
	Backend_Block_Scope bc = {};
	bc.add_block();
	//Add proc param vars

	Ast_Block* block = proc_decl->block;
	for (Ast_Statement* statement : block->statements)
	{
		switch (statement->tag)
		{
			case Ast_Statement::Tag::If:
			{
				error_exit("if statement not supported");
			} break;
			case Ast_Statement::Tag::For:
			{
				error_exit("for statement not supported");
			} break;
			case Ast_Statement::Tag::Break:
			{
				error_exit("break statement not supported");
			} break;
			case Ast_Statement::Tag::Return:
			{
				Ast_Return* _return = statement->as_return;
				if (_return->expr.has_value()) 
					LLVMBuildRet(builder, build_expr_value(_return->expr.value(), &bc));
				else LLVMBuildRet(builder, NULL);
			} break;
			case Ast_Statement::Tag::Continue:
			{
				error_exit("continue statement not supported");
			} break;
			case Ast_Statement::Tag::Proc_Call:
			{
				Ast_Proc_Call* proc_call = statement->as_proc_call;
				if (!proc_call->input_exprs.empty()) error_exit("proc call with input exprs is not supported");

				std::optional<Proc_Meta> proc_meta = proc_decl_map.find(proc_call->ident.token.string_value, hash_fnv1a_32(proc_call->ident.token.string_value));
				if (!proc_meta) { error_exit("failed to find proc declaration while trying to call it"); return; }
				//@Notice usage of return values must be enforced on checking stage, statement proc call should return nothing
				LLVMValueRef ret_val = LLVMBuildCall2(builder, proc_meta.value().proc_type, proc_meta.value().proc_val, NULL, 0, "call_val");
			} break;
			case Ast_Statement::Tag::Var_Decl:
			{
				Ast_Var_Decl* var_decl = statement->as_var_decl;
				if (!var_decl->type.has_value()) error_exit("var decl expected type to be known");
				
				Ast_Type* type = var_decl->type.value();
				LLVMTypeRef type_ref = NULL;
				Ast_Struct_Decl* struct_decl = NULL;

				switch (type->tag) 
				{
					case Ast_Type::Tag::Basic:
					{
						type_ref = basic_type_convert(type->as_basic);
					} break;
					case Ast_Type::Tag::Custom:
					{
						auto struct_meta = struct_decl_map.find(type->as_custom.token.string_value, hash_fnv1a_32(type->as_custom.token.string_value));
						if (!struct_meta) error_exit("var decl custom type not found");
						type_ref = struct_meta.value().struct_type;
						struct_decl = struct_meta.value().struct_decl;
					} break;
					case Ast_Type::Tag::Pointer:
					{
						error_exit("var decl pointer types not supported");
					} break;
					case Ast_Type::Tag::Array:
					{
						error_exit("var decl arrays not supported");
					} break;
				}

				LLVMValueRef var_ptr = LLVMBuildAlloca(builder, type_ref, get_c_string(var_decl->ident.token));
				if (var_decl->expr.has_value())
				{
					LLVMValueRef value = build_expr_value(var_decl->expr.value(), &bc);
					if (type_ref != LLVMTypeOf(value)) error_exit("type mismatch in variable declaration");
					LLVMBuildStore(builder, value, var_ptr);
				}
				else LLVMBuildStore(builder, LLVMConstNull(type_ref), var_ptr);
				
				//@Assuming that Custom means struct type
				bc.add_var(Var_Meta { type->tag == Ast_Type::Tag::Custom, struct_decl, var_decl->ident.token.string_value, type_ref, var_ptr });
			} break;
			case Ast_Statement::Tag::Var_Assign:
			{
				Ast_Var_Assign* var_assign = statement->as_var_assign;
				if (var_assign->op != ASSIGN_OP_NONE) error_exit("var assign: only = op is supported");
				LLVMValueRef expr_value = build_expr_value(var_assign->expr, &bc);
				
				Ast_Var* var = var_assign->var;
				auto var_meta = bc.find_var(var->ident.token.string_value);
				if (!var_meta) error_exit("var assign: variable wasnt found in block scope");

				if (var->access.has_value())
				{
					if (LLVMGetTypeKind(var_meta.value().var_type) != LLVMStructTypeKind) //@Utilities move to helper function
						error_exit("var assign: attempting to access on the non struct type");
					if (var_meta.value().is_struct == false) error_exit("var assign: expected var to be a struct during access");

					Ast_Access* access = var->access.value();
					if (access->tag == Ast_Access::Tag::Array) error_exit("var assign: array access isnt supported");

					auto field_id = find_struct_field_id(var_meta.value().struct_decl, access->as_var->ident.token.string_value);
					if (!field_id) error_exit("var assign: failed to find a struct field using identifier");

					LLVMValueRef gep_ptr = LLVMBuildStructGEP2(builder, var_meta.value().var_type, var_meta.value().var_value, field_id.value(), "gep_ptr");
					//@Determine which field type is being accessed by series of geps
					//if (accessed_type != LLVMTypeOf(expr_value)) error_exit("type mismatch in variable assignment with access chain");
					LLVMBuildStore(builder, expr_value, gep_ptr);
				}
				else
				{
					LLVMValueRef var_ptr = var_meta.value().var_value;
					LLVMTypeRef var_type = var_meta.value().var_type;
					if (var_type != LLVMTypeOf(expr_value)) error_exit("type mismatch in variable assignment");
					LLVMBuildStore(builder, expr_value, var_ptr);
				}
			} break;
			default: break;
		}
	}

	bc.pop_block(); //exiting proc scope
	
	//@For non void return values return statement is expected to exist during checking stage
	if (!proc_decl->return_type.has_value()) 
		LLVMBuildRet(builder, NULL);
}

LLVMValueRef Backend_LLVM::build_expr_value(Ast_Expr* expr, Backend_Block_Scope* bc)
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
					auto var_meta = bc->find_var(var->ident.token.string_value);
					if (!var_meta) error_exit("var term: variable wasnt found in block scope");

					if (var->access.has_value())
					{
						if (LLVMGetTypeKind(var_meta.value().var_type) != LLVMStructTypeKind) //@Utilities move to helper function
							error_exit("var term: attempting to access on the non struct type");
						if(var_meta.value().is_struct == false) error_exit("var term: expected var to be a struct during access");
						
						Ast_Access* access = var->access.value();
						if (access->tag == Ast_Access::Tag::Array) error_exit("var term: array access isnt supported");

						auto field_id = find_struct_field_id(var_meta.value().struct_decl, access->as_var->ident.token.string_value);
						if (!field_id) error_exit("var term: failed to find a struct field using identifier");
						LLVMValueRef gep_ptr = LLVMBuildStructGEP2(builder, var_meta.value().var_type, var_meta.value().var_value, field_id.value(), "gep_ptr");
						
						//@Hack assuming always i32 type of the field
						//@Determine which field type is being accessed by series of geps
						value_ref = LLVMBuildLoad2(builder, LLVMInt32Type(), gep_ptr, "load_val");
					}
					else
					{
						//@Not sure about needing a load when returning struct / almost 100% sure
						//Load is needed for basic types during expressions
						value_ref = LLVMBuildLoad2(builder, var_meta.value().var_type, var_meta.value().var_value, "load_val");
					}

				} break;
				case Ast_Term::Tag::Literal:
				{
					Token token = term->as_literal.token;
					if (token.type == TOKEN_BOOL_LITERAL)
					{
						value_ref = LLVMConstInt(LLVMInt1Type(), (int)token.bool_value, 0);
					}
					else if (token.type == TOKEN_FLOAT_LITERAL) //@Choose Double or float? defaulting to double
					{
						value_ref = LLVMConstReal(LLVMDoubleType(), token.float64_value);
					}
					else if (token.type == TOKEN_INTEGER_LITERAL) //@Todo sign extend?
					{
						value_ref = LLVMConstInt(LLVMInt32Type(), token.integer_value, 0); 
					}
					else error_exit("unsupported literal type");
				} break;
				case Ast_Term::Tag::Proc_Call:
				{
					Ast_Proc_Call* proc_call = term->as_proc_call;
					if (!proc_call->input_exprs.empty()) error_exit("proc call with input exprs is not supported");
					if (proc_call->access.has_value()) error_exit("access trail from function return values is not supported");
					auto proc_meta = proc_decl_map.find(proc_call->ident.token.string_value, hash_fnv1a_32(proc_call->ident.token.string_value));
					if (!proc_meta) error_exit("failed to find proc declaration while trying to call it");

					value_ref = LLVMBuildCall2(builder, proc_meta.value().proc_type, proc_meta.value().proc_val, NULL, 0, "call_ret_val");
				} break;
			}
		} break;
		case Ast_Expr::Tag::Unary_Expr:
		{
			Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
			UnaryOp op = unary_expr->op;
			LLVMValueRef rhs = build_expr_value(unary_expr->right, bc);

			LLVMTypeRef rhs_type = LLVMTypeOf(rhs);
			LLVMTypeKind rhs_kind = LLVMGetTypeKind(rhs_type);

			if (!kind_is_ifd(rhs_kind)) error_exit("unary_expr rhs kind != ifd");
			bool fd_kind = kind_is_fd(rhs_kind);
			bool int_kind = kind_is_i(rhs_kind);
			bool bool_kind = type_is_bool(rhs_kind, rhs_type);
			if (!fd_kind && !int_kind && !bool_kind) error_exit("unary_expr rhs allowed types are: [fd] [iX] [i1(bool)]");

			switch (op)
			{
				case UNARY_OP_MINUS:
				{
					if (fd_kind) value_ref = LLVMBuildFNeg(builder, rhs, "utmp");
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
					error_exit("unary_expr & not supported");
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
			LLVMValueRef rhs = build_expr_value(binary_expr->right, bc);

			LLVMTypeRef lhs_type = LLVMTypeOf(lhs);
			LLVMTypeRef rhs_type = LLVMTypeOf(rhs);
			LLVMTypeKind lhs_kind = LLVMGetTypeKind(lhs_type);
			LLVMTypeKind rhs_kind = LLVMGetTypeKind(rhs_type);

			if (!kind_is_ifd(lhs_kind)) error_exit("bin_expr lhs kind != ifd");
			if (!kind_is_ifd(rhs_kind)) error_exit("bin_expr rhs kind != ifd");
			bool fd_kind = (kind_is_fd(lhs_kind) && kind_is_fd(rhs_kind));
			bool int_kind = (kind_is_i(lhs_kind) && kind_is_i(rhs_kind));
			bool bool_kind = (type_is_bool(lhs_kind, lhs_type) && type_is_bool(rhs_kind, rhs_type));
			if (!fd_kind && !int_kind && !bool_kind) error_exit("bin_expr lhs rhs dont match, allowed types are: [fd : fd] [iX : iX] [i1(bool) : i1(bool)]");
			
			//@Might use LLVMBuildBinOp() unitility with op code
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
					if (fd_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOLT, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSLT, lhs, rhs, "btmp"); //@Determine S / U predicates
					else error_exit("bin_expr < expected fd or i got bool");
				} break;
				case BINARY_OP_GREATER:
				{
					if (fd_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOGT, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSGT, lhs, rhs, "btmp"); //@Determine S / U predicates
					else error_exit("bin_expr > expected fd or i got bool");
				} break;
				case BINARY_OP_LESS_EQUALS:
				{
					if (fd_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOLE, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSLE, lhs, rhs, "btmp"); //@Determine S / U predicates
					else error_exit("bin_expr <= expected fd or i got bool");
				} break;
				case BINARY_OP_GREATER_EQUALS:
				{
					if (fd_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOGE, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntSGE, lhs, rhs, "btmp"); //@Determine S / U predicates
					else error_exit("bin_expr >= expected fd or i got bool");
				} break;
				case BINARY_OP_IS_EQUALS:
				{
					if (fd_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealOEQ, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntEQ, lhs, rhs, "btmp"); //@Determine S / U predicates
					else error_exit("bin_expr == expected fd or i got bool");
				} break;
				case BINARY_OP_NOT_EQUALS:
				{
					if (fd_kind) value_ref = LLVMBuildFCmp(builder, LLVMRealONE, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildICmp(builder, LLVMIntNE, lhs, rhs, "btmp"); //@Determine S / U predicates
					else error_exit("bin_expr != expected fd or i got bool");
				} break;
				// MathOps [+ - * / %]
				case BINARY_OP_PLUS:
				{
					if (fd_kind) value_ref = LLVMBuildFAdd(builder, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildAdd(builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
					else error_exit("bin_expr + expected fd or i got bool");
				} break;
				case BINARY_OP_MINUS:
				{
					if (fd_kind) value_ref = LLVMBuildFSub(builder, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildSub(builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
					else error_exit("bin_expr + expected fd or i got bool");
				} break;
				case BINARY_OP_TIMES:
				{
					if (fd_kind) value_ref = LLVMBuildFMul(builder, lhs, rhs, "btmp");
					else if (int_kind) value_ref = LLVMBuildMul(builder, lhs, rhs, "btmp"); //@Safety NoSignedWrap & NoUnsignedWrap variants exist
					else error_exit("bin_expr * expected fd or i got bool");
				} break;
				case BINARY_OP_DIV:
				{
					if (fd_kind) value_ref = LLVMBuildFDiv(builder, lhs, rhs, "btmp");
					//@ SU variants: LLVMBuildSDiv, LLVMBuildExactSDiv, LLVMBuildUDiv, LLVMBuildExactUDiv
					else if (int_kind) value_ref = LLVMBuildSDiv(builder, lhs, rhs, "btmp");
					else error_exit("bin_expr / expected fd or i got bool");
				} break;
				case BINARY_OP_MOD: //@Design floating modulo is possible but isnt too usefull
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

	if (value_ref == NULL) error_exit("bin_expr value_ref is null on return");
	return value_ref;
}

LLVMTypeRef Backend_LLVM::basic_type_convert(BasicType basic_type) //@Uints shouild use different bitwith?
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
		//@Undefined for now, might use a struct: case BASIC_TYPE_STRING:
		default: error_exit("basic type not found (basic_type_convert)"); break;
	}
	return LLVMVoidType();
}

bool Backend_LLVM::kind_is_ifd(LLVMTypeKind type_kind)
{
	return type_kind == LLVMIntegerTypeKind || type_kind == LLVMFloatTypeKind || type_kind == LLVMDoubleTypeKind;
}

bool Backend_LLVM::kind_is_fd(LLVMTypeKind type_kind)
{
	return type_kind == LLVMFloatTypeKind || type_kind == LLVMDoubleTypeKind;
}

bool Backend_LLVM::kind_is_i(LLVMTypeKind type_kind)
{
	return type_kind == LLVMIntegerTypeKind;
}

bool Backend_LLVM::type_is_bool(LLVMTypeKind type_kind, LLVMTypeRef type_ref)
{
	return type_kind == LLVMIntegerTypeKind && LLVMGetIntTypeWidth(type_ref) == 1;
}

std::optional<u32> Backend_LLVM::find_struct_field_id(Ast_Struct_Decl* struct_decl, StringView field_str)
{
	u32 count = 0;
	for (const auto& field : struct_decl->fields)
	{
		if (field.ident.token.string_value == field_str) return count;
		count += 1;
	}
	return {};
}

char* Backend_LLVM::get_c_string(Token& token) //@Unsafe hack to get c string from string view of source file string, need to do smth better
{
	token.string_value.data[token.string_value.count] = 0;
	return (char*)token.string_value.data;
}

void Backend_LLVM::error_exit(const char* message)
{
	printf("backend error: %s.\n", message);
	exit(EXIT_FAILURE);
}

void Backend_LLVM::build_ir_example(Ast* ast)
{
	LLVMContextRef context = LLVMContextCreate();
	LLVMModuleRef mod = LLVMModuleCreateWithNameInContext("module", context);
	LLVMBuilderRef builder = LLVMCreateBuilderInContext(context);
	
	// Create and add function prototype for sum
	LLVMTypeRef param_types[] = { LLVMInt32Type(), LLVMInt32Type() };
	LLVMTypeRef sum_proc_type = LLVMFunctionType(LLVMInt32Type(), param_types, 2, 0);
	LLVMValueRef sum_proc = LLVMAddFunction(mod, "sum", sum_proc_type);
	LLVMBasicBlockRef block = LLVMAppendBasicBlockInContext(context, sum_proc, "block");
	LLVMPositionBuilderAtEnd(builder, block);
	LLVMValueRef sum_value = LLVMBuildAdd(builder, LLVMGetParam(sum_proc, 0), LLVMGetParam(sum_proc, 1), "sum_value");
	LLVMBuildRet(builder, sum_value);

	// Create and add function prototype for main
	LLVMTypeRef main_ret_type = LLVMFunctionType(LLVMInt32Type(), NULL, 0, 0);
	LLVMValueRef main_func = LLVMAddFunction(mod, "main", main_ret_type);
	LLVMBasicBlockRef main_block = LLVMAppendBasicBlockInContext(context, main_func, "main_block");
	LLVMPositionBuilderAtEnd(builder, main_block);

	// Call the sum function with arguments
	LLVMValueRef args[] = { LLVMConstInt(LLVMInt32Type(), 39, 0), LLVMConstInt(LLVMInt32Type(), 30, 0) };
	LLVMValueRef sum_result = LLVMBuildCall2(builder, sum_proc_type, sum_proc, args, 2, "sum_result");
	LLVMBuildRet(builder, sum_result);

	LLVMDisposeBuilder(builder);
}

void Backend_LLVM::build_binaries()
{
	//@Todo setup ErrorHandler from ErrorHandling.h to not crash with exit(1)
	//even during IR building for dev period
	//@Performance: any benefits of doing only init for one platform?
	//LLVMInitializeX86TargetInfo() ...
	LLVMInitializeAllTargetInfos();
	LLVMInitializeAllTargets();
	LLVMInitializeAllTargetMCs();
	LLVMInitializeAllAsmParsers();
	LLVMInitializeAllAsmPrinters();

	LLVMTargetRef target;
	char* error = 0;
	char* cpu = LLVMGetHostCPUName();
	char* cpu_features = LLVMGetHostCPUFeatures();
	char* triple = LLVMGetDefaultTargetTriple();
	LLVMGetTargetFromTriple(triple, &target, &error);
	LLVMSetTarget(module, triple);

	LLVMTargetMachineRef machine = LLVMCreateTargetMachine
	(target, triple, cpu, cpu_features, LLVMCodeGenLevelDefault, LLVMRelocDefault, LLVMCodeModelDefault);
	
	LLVMTargetDataRef datalayout = LLVMCreateTargetDataLayout(machine);
	char* datalayout_str = LLVMCopyStringRepOfTargetData(datalayout);
	LLVMSetDataLayout(module, datalayout_str);
	LLVMDisposeMessage(datalayout_str);
	debug_print_module();

	LLVMTargetMachineEmitToFile(machine, module, "result.o", LLVMObjectFile, &error);
	if (error != NULL) printf("error: %s\n", error);
	
	LLVMDisposeModule(module);
	LLVMContextDispose(context);

	LLVMDisposeMessage(error);
	LLVMDisposeMessage(cpu);
	LLVMDisposeMessage(cpu_features);
	LLVMDisposeMessage(triple);
}

void Backend_LLVM::debug_print_module()
{
	LLVMPrintModuleToFile(module, "output.ll", NULL);
	char* message = LLVMPrintModuleToString(module);
	printf("Module: %s", message);
	LLVMDisposeMessage(message);
}

void Backend_Block_Scope::add_block()
{
	block_stack.emplace_back(Backend_Block_Info { 0 });
}

void Backend_Block_Scope::pop_block()
{
	Backend_Block_Info info = block_stack[block_stack.size() - 1];
	for (u32 i = 0; i < info.var_count; i++)
		var_stack.pop_back();
	block_stack.pop_back();
}

void Backend_Block_Scope::add_var(const Var_Meta& var)
{
	block_stack[block_stack.size() - 1].var_count += 1;
	var_stack.emplace_back(var);
}

std::optional<Var_Meta> Backend_Block_Scope::find_var(StringView str)
{
	for (const Var_Meta& var : var_stack)
	if (var.str == str) return var;
	return {};
}
