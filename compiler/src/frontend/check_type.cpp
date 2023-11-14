#include "check_type.h"

#include "error_handler.h"
#include "printer.h"

Type_Kind type_kind(Ast_Type type)
{
	if (type.pointer_level > 0) return Type_Kind::Pointer;

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic:
	{
		switch (type.as_basic)
		{
		case BasicType::F32:
		case BasicType::F64: return Type_Kind::Float;
		case BasicType::BOOL: return Type_Kind::Bool;
		case BasicType::STRING: return Type_Kind::String;
		default: return Type_Kind::Integer;
		}
	}
	case Ast_Type_Tag::Array: return Type_Kind::Array;
	case Ast_Type_Tag::Struct: return Type_Kind::Struct;
	case Ast_Type_Tag::Enum: return Type_Kind::Enum;
	default: { err_internal("type_kind: invalid Ast_Type_Tag"); return Type_Kind::Bool; }
	}
}

Ast_Type type_from_poison()
{
	Ast_Type type = {};
	type.tag = Ast_Type_Tag::Poison;
	return type;
}

Ast_Type type_from_basic(BasicType basic_type)
{
	Ast_Type type = {};
	type.tag = Ast_Type_Tag::Basic;
	type.as_basic = basic_type;
	return type;
}

option<Ast_Struct_Type> type_extract_struct_value_type(Ast_Type type)
{
	if (type.pointer_level > 0) return {};

	switch (type.tag)
	{
	case Ast_Type_Tag::Array: return type_extract_struct_value_type(type.as_array->element_type);
	case Ast_Type_Tag::Struct: return type.as_struct;
	default: return {};
	}
}

option<Ast_Array_Type*> type_extract_array_value_type(Ast_Type type)
{
	if (type.pointer_level > 0) return {};
	switch (type.tag)
	{
	case Ast_Type_Tag::Array: return type.as_array;
	default: return {};
	}
}

void compute_struct_size(Ast_Struct_Decl* struct_decl)
{
	u32 field_count = (u32)struct_decl->fields.size();
	u32 total_size = 0;
	u32 max_align = 0;
	
	for (u32 i = 0; i < field_count; i += 1)
	{
		u32 field_size = type_size(struct_decl->fields[i].type);
		total_size += field_size;

		if (i + 1 < field_count)
		{
			u32 align = type_align(struct_decl->fields[i + 1].type);
			if (align > field_size)
			{
				u32 padding = align - field_size;
				total_size += padding;
			}
			if (align > max_align) max_align = align;
		}
		else
		{
			u32 align = max_align;
			if (align > field_size)
			{
				u32 padding = align - field_size;
				total_size += padding;
			}
		}
	}

	struct_decl->struct_size = total_size;
	struct_decl->max_align = max_align;
}

u32 type_basic_size(BasicType basic_type)
{
	switch (basic_type)
	{
	case BasicType::I8: return 1;
	case BasicType::U8: return 1;
	case BasicType::I16: return 2;
	case BasicType::U16: return 2;
	case BasicType::I32: return 4;
	case BasicType::U32: return 4;
	case BasicType::I64: return 8;
	case BasicType::U64: return 8;
	case BasicType::BOOL: return 1;
	case BasicType::F32: return 4;
	case BasicType::F64: return 8;
	case BasicType::STRING: return 0; //@Not implemented
	default: { err_internal("check_get_basic_type_size: invalid BasicType"); return 0; }
	}
}

u32 type_basic_align(BasicType basic_type)
{
	switch (basic_type)
	{
	case BasicType::I8: return 1;
	case BasicType::U8: return 1;
	case BasicType::I16: return 2;
	case BasicType::U16: return 2;
	case BasicType::I32: return 4;
	case BasicType::U32: return 4;
	case BasicType::I64: return 8;
	case BasicType::U64: return 8;
	case BasicType::BOOL: return 1;
	case BasicType::F32: return 4;
	case BasicType::F64: return 8;
	case BasicType::STRING: return 0; //@Not implemented
	default: { err_internal("check_get_basic_type_align: invalid BasicType"); return 0; }
	}
}

u32 type_size(Ast_Type type)
{
	if (type.pointer_level > 0) return 8; //@Assume 64bit

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic: return type_basic_size(type.as_basic);
	case Ast_Type_Tag::Array: 
	{
		if (type.as_array->size_expr->tag != Ast_Expr_Tag::Folded_Expr || type.as_array->size_expr->as_folded_expr.basic_type != BasicType::U32)
			err_internal("type_size: array size expr is not folded or has incorrect type");
		u32 count = (u32)type.as_array->size_expr->as_folded_expr.as_u64;
		return count * type_size(type.as_array->element_type);
	}
	case Ast_Type_Tag::Struct:
	{
		if (type.as_struct.struct_decl->size_eval != Consteval::Valid)
			err_internal("type_size: expected struct size_eval to be Consteval::Valid");
		return type.as_struct.struct_decl->struct_size;
	}
	case Ast_Type_Tag::Enum: return type_basic_size(type.as_enum.enum_decl->basic_type);
	default: { err_internal("check_get_type_size: invalid Ast_Type_Tag"); return 0; }
	}
}

u32 type_align(Ast_Type type)
{
	if (type.pointer_level > 0) return 8; //@Assume 64bit

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic: return type_basic_align(type.as_basic);
	case Ast_Type_Tag::Array: return type_align(type.as_array->element_type);
	case Ast_Type_Tag::Struct: 
	{
		if (type.as_struct.struct_decl->size_eval != Consteval::Valid) 
			err_internal("type_align: expected struct size_eval to be Consteval::Valid");
		return type.as_struct.struct_decl->max_align;
	}
	case Ast_Type_Tag::Enum: return type_basic_align(type.as_enum.enum_decl->basic_type);
	default: { err_internal("type_align: invalid Ast_Type_Tag"); return 0; }
	}
}

option<Ast_Type> check_expr_type(Check_Context* cc, Ast_Expr* expr, option<Ast_Type> expect_type, Expr_Constness constness)
{
	if (expect_type && type_is_poison(expect_type.value())) return {};

	Expr_Context context = { expect_type, constness };
	option<Ast_Type> type = check_expr(cc, context, expr);
	
	if (!type) return {};
	if (!expect_type) return type;

	type_implicit_cast(&type.value(), expect_type.value());

	if (!type_match(type.value(), expect_type.value()))
	{
		err_report(Error::TYPE_MISMATCH);
		printf("Expected: "); print_type(expect_type.value()); printf("\n");
		printf("Got:      "); print_type(type.value()); printf("\n");
		err_context(cc, expr->span);
		return {};
	}

	return type;
}

bool type_match(Ast_Type type_a, Ast_Type type_b)
{
	if (type_a.pointer_level != type_b.pointer_level) return false;
	if (type_a.tag != type_b.tag) return false;

	switch (type_a.tag)
	{
	case Ast_Type_Tag::Basic: return type_a.as_basic == type_b.as_basic;
	case Ast_Type_Tag::Struct: return type_a.as_struct.struct_id == type_b.as_struct.struct_id;
	case Ast_Type_Tag::Enum: return type_a.as_enum.enum_id == type_b.as_enum.enum_id;
	case Ast_Type_Tag::Array:
	{
		Ast_Array_Type* array_a = type_a.as_array;
		Ast_Array_Type* array_b = type_b.as_array;

		if (array_a->size_expr->tag != Ast_Expr_Tag::Folded_Expr || array_b->size_expr->tag != Ast_Expr_Tag::Folded_Expr) 
		{
			err_internal("type_match: expected size_expr to be Ast_Expr_Tag::Folded_Expr");
		}
		else if (array_a->size_expr->as_folded_expr.basic_type != BasicType::U32 || array_b->size_expr->as_folded_expr.basic_type != BasicType::U32)
		{
			err_internal("type_match: array size_expr expected folded type to be u32");
		}
		else
		{
			if (array_a->size_expr->as_folded_expr.as_u64 != array_b->size_expr->as_folded_expr.as_u64) return false;
		}

		return type_match(array_a->element_type, array_b->element_type);
	}
	default: { err_internal("type_match: invalid Ast_Type_Tag"); return false; }
	}
}

bool type_is_poison(Ast_Type type)
{
	return type.tag == Ast_Type_Tag::Poison;
}

void type_implicit_cast(Ast_Type* type, Ast_Type target_type)
{
	if (type->tag != Ast_Type_Tag::Basic) return;
	if (target_type.tag != Ast_Type_Tag::Basic) return;
	if (type->as_basic == target_type.as_basic) return;
	Type_Kind kind = type_kind(*type);
	Type_Kind target_kind = type_kind(target_type);

	if (kind == Type_Kind::Float && target_kind == Type_Kind::Float)
	{
		type->as_basic = target_type.as_basic;
		return;
	}

	if (kind == Type_Kind::Integer && target_kind == Type_Kind::Integer)
	{
		//
	}
}

void type_implicit_binary_cast(Ast_Type* type_a, Ast_Type* type_b)
{
	if (type_a->tag != Ast_Type_Tag::Basic) return;
	if (type_b->tag != Ast_Type_Tag::Basic) return;
	if (type_a->as_basic == type_b->as_basic) return;
	Type_Kind kind_a = type_kind(*type_a);
	Type_Kind kind_b = type_kind(*type_b);

	if (kind_a == Type_Kind::Float && kind_b == Type_Kind::Float)
	{
		if (type_a->as_basic == BasicType::F32)
			type_a->as_basic = BasicType::F64;
		else type_b->as_basic = BasicType::F64;
		return;
	}

	if (kind_a == Type_Kind::Integer && kind_b == Type_Kind::Integer)
	{
		//
	}
}

option<Ast_Type> check_expr(Check_Context* cc, Expr_Context context, Ast_Expr* expr)
{
	option<Expr_Kind> kind_result = resolve_expr(cc, context, expr);
	if (!kind_result) return {};
	Expr_Kind kind = kind_result.value();
	
	expr->is_const = kind == Expr_Kind::Const || kind == Expr_Kind::Constfold;
	if (context.constness == Expr_Constness::Const && !expr->is_const)
	{
		err_report(Error::EXPR_EXPECTED_CONSTANT);
		err_context(cc, expr->span);
		return {};
	}

	switch (kind)
	{
	case Expr_Kind::Normal:
	case Expr_Kind::Const:
	{
		switch (expr->tag)
		{
		case Ast_Expr_Tag::Term: return check_term(cc, context, expr->as_term);
		case Ast_Expr_Tag::Unary_Expr: return check_unary_expr(cc, context, expr->as_unary_expr);
		case Ast_Expr_Tag::Binary_Expr: return check_binary_expr(cc, context, expr->as_binary_expr);
		default: { err_internal("check_expr: invalid Ast_Expr_Tag"); return {}; }
		}
	}
	case Expr_Kind::Constfold:
	{
		if (expr->tag == Ast_Expr_Tag::Folded_Expr)
		{
			//@Debug potential multi check errors with multi dimentional arrays etc
			//printf("Folding folded expr\n");
			//err_context(cc, expr->span);
			return type_from_basic(expr->as_folded_expr.basic_type);
		}

		option<Literal> lit = check_folded_expr(cc, expr);
		if (!lit) return {};
		return check_apply_expr_fold(cc, context, expr, lit.value());
	}
	default: { err_internal("check_expr: invalid Expr_Kind"); return {}; }
	}
}

option<Expr_Kind> resolve_expr(Check_Context* cc, Expr_Context context, Ast_Expr* expr)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term:
	{
		Ast_Term* term = expr->as_term;
		switch (term->tag)
		{
		case Ast_Term_Tag::Var:
		{
			Ast_Var* var = term->as_var;
			resolve_var(cc, var);
			switch (var->tag)
			{
			case Ast_Var_Tag::Local:
			{
				option<Ast_Type> var_type = check_context_block_find_var_type(cc, var->local.ident);
				if (!var_type)
				{
					err_report(Error::VAR_LOCAL_NOT_FOUND);
					err_context(cc, var->local.ident.span);
					return {};
				}
				if (type_is_poison(var_type.value())) return {};
				else return Expr_Kind::Normal;
			}
			case Ast_Var_Tag::Global:
			{
				Ast_Global_Decl* global_decl = var->global.global_decl;
				Ast_Consteval_Expr* consteval_expr = global_decl->consteval_expr;
				if (consteval_expr->eval != Consteval::Valid) return {};

				return resolve_expr(cc, context, consteval_expr->expr);
			}
			default: return {};
			}
		}
		case Ast_Term_Tag::Enum:
		{
			Ast_Enum* _enum = term->as_enum;
			resolve_enum(cc, _enum);
			if (_enum->tag != Ast_Enum_Tag::Resolved) return {};

			Ast_Enum_Decl* enum_decl = _enum->resolved.type.enum_decl;
			Ast_Enum_Variant* enum_variant = &enum_decl->variants[_enum->resolved.variant_id];
			Ast_Consteval_Expr* consteval_expr = enum_variant->consteval_expr;
			if (consteval_expr->eval != Consteval::Valid) return {};

			return Expr_Kind::Constfold;
		}
		case Ast_Term_Tag::Cast:
		{
			Ast_Cast* cast = term->as_cast;
			resolve_cast(cc, context, cast);
			if (cast->tag == Ast_Cast_Tag::Invalid) return {};

			return resolve_expr(cc, context, cast->expr);
		}
		case Ast_Term_Tag::Sizeof:
		{
			Ast_Sizeof* size_of = term->as_sizeof;
			resolve_sizeof(cc, size_of, true);
			if (size_of->tag != Ast_Sizeof_Tag::Resolved) return {};
			return Expr_Kind::Constfold;
		}
		case Ast_Term_Tag::Literal:
		{
			Ast_Literal* lit = term->as_literal;
			if (lit->token.type == TokenType::STRING_LITERAL) return Expr_Kind::Const; //@Incomplete strings
			else return Expr_Kind::Constfold;
		}
		case Ast_Term_Tag::Proc_Call:
		{
			Ast_Proc_Call* proc_call = term->as_proc_call;
			resolve_proc_call(cc, proc_call);
			if (proc_call->tag != Ast_Proc_Call_Tag::Resolved) return {};
			return Expr_Kind::Normal;
		}
		case Ast_Term_Tag::Array_Init:
		{
			Ast_Array_Init* array_init = term->as_array_init;
			resolve_array_init(cc, context, array_init, true);
			if (array_init->tag != Ast_Array_Init_Tag::Resolved) return {};

			Expr_Kind kind = Expr_Kind::Const;

			Expr_Context input_context = Expr_Context{ {}, context.constness };
			if (array_init->type)
			{
				Ast_Array_Type* array = array_init->type.value().as_array;
				input_context.expect_type = array->element_type;
			}

			for (Ast_Expr* input_expr : array_init->input_exprs)
			{
				option<Expr_Kind> input_kind = resolve_expr(cc, input_context, input_expr);
				if (!input_kind) return {};
				if (input_kind.value() == Expr_Kind::Normal) kind = Expr_Kind::Normal;
			}
			return kind;
		}
		case Ast_Term_Tag::Struct_Init:
		{
			Ast_Struct_Init* struct_init = term->as_struct_init;
			resolve_struct_init(cc, context, struct_init);
			if (struct_init->tag != Ast_Struct_Init_Tag::Resolved) return {};

			Expr_Kind kind = Expr_Kind::Const;

			Ast_Struct_Decl* struct_decl = struct_init->resolved.type.value().struct_decl;
			u32 count = 0;

			for (Ast_Expr* input_expr : struct_init->input_exprs)
			{
				Expr_Context input_context = Expr_Context{ struct_decl->fields[count].type, context.constness };
				count += 1;

				option<Expr_Kind> input_kind = resolve_expr(cc, input_context, input_expr);
				if (!input_kind) return {};
				if (input_kind.value() == Expr_Kind::Normal) kind = Expr_Kind::Normal;
			}
			return kind;
		}
		default: { err_internal("resolve_expr: invalid Ast_Term_Tag"); return {}; }
		}
	}
	case Ast_Expr_Tag::Unary_Expr:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		option<Expr_Kind> rhs_kind = resolve_expr(cc, context, unary_expr->right);
		return rhs_kind;
	}
	case Ast_Expr_Tag::Binary_Expr:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		option<Expr_Kind> lhs_kind = resolve_expr(cc, context, binary_expr->left);
		option<Expr_Kind> rhs_kind = resolve_expr(cc, context, binary_expr->right);
		if (!lhs_kind || !rhs_kind) return {};
		return lhs_kind.value() < rhs_kind.value() ? lhs_kind.value() : rhs_kind.value();
	}
	case Ast_Expr_Tag::Folded_Expr:
	{
		return Expr_Kind::Constfold;
	}
	default: { err_internal("resolve_expr: invalid Ast_Expr_Tag"); return {}; }
	}
}

//@TODO temp allowing old errors:
#define err_set (void)0;

option<Ast_Type> check_term(Check_Context* cc, Expr_Context context, Ast_Term* term)
{
	switch (term->tag)
	{
	case Ast_Term_Tag::Var: return check_var(cc, term->as_var);
	case Ast_Term_Tag::Cast:
	{
		Ast_Cast* cast = term->as_cast;
		return type_from_basic(cast->basic_type);
	}
	case Ast_Term_Tag::Literal:
	{
		Ast_Literal literal = *term->as_literal;
		switch (literal.token.type)
		{
		case TokenType::STRING_LITERAL: //@ Strings are just *i8 cstrings for now
		{
			Ast_Type string_ptr = type_from_basic(BasicType::I8);
			string_ptr.pointer_level += 1;
			return string_ptr;
		}
		default: { err_internal("check_term: invalid literal TokenType"); return {}; }
		}
	}
	case Ast_Term_Tag::Proc_Call:
	{
		return check_proc_call(cc, term->as_proc_call, Checker_Proc_Call_Flags::In_Expr);
	}
	case Ast_Term_Tag::Struct_Init:
	{
		Ast_Struct_Init* struct_init = term->as_struct_init;
		resolve_struct_init(cc, context, struct_init);
		if (struct_init->tag == Ast_Struct_Init_Tag::Invalid) return {};

		// check input count
		Ast_Struct_Decl* struct_decl = struct_init->resolved.type.value().struct_decl;
		u32 field_count = (u32)struct_decl->fields.size();
		u32 input_count = (u32)struct_init->input_exprs.size();
		if (field_count != input_count)
		{
			//@Err
			err_set;
			printf("Unexpected number of fields in struct initializer:\n");
			printf("Expected: %lu Got: %lu \n", field_count, input_count);
		}

		// check input exprs
		for (u32 i = 0; i < input_count; i += 1)
		{
			if (i < field_count)
			{
				Ast_Type field_type = struct_decl->fields[i].type;
				check_expr_type(cc, struct_init->input_exprs[i], field_type, context.constness);
			}
		}

		Ast_Type type = {};
		type.tag = Ast_Type_Tag::Struct;
		type.as_struct = struct_init->resolved.type.value();
		return type;
	}
	case Ast_Term_Tag::Array_Init:
	{
		Ast_Array_Init* array_init = term->as_array_init;
		resolve_array_init(cc, context, array_init, true);
		if (array_init->tag == Ast_Array_Init_Tag::Invalid) return {};

		if (array_init->type.value().as_array->size_expr->tag != Ast_Expr_Tag::Folded_Expr)
		{
			err_internal("check_term: expected array size_expr to be Ast_Expr_Tag::Folded_Expr");
		}
		else if (array_init->type.value().as_array->size_expr->as_folded_expr.basic_type != BasicType::U32)
		{
			err_internal("check_term: array size_expr expected folded type to be u32");
			err_context(cc, array_init->type.value().as_array->size_expr->span);
		}

		u32 size_count = (u32)array_init->type.value().as_array->size_expr->as_folded_expr.as_u64;
		u32 input_count = (u32)array_init->input_exprs.size();
		if (size_count != input_count)
		{
			//@Err
			err_set;
			printf("Unexpected number of fields in array initializer:\n");
			printf("Expected: %lu Got: %lu \n", size_count, input_count);
		}

		Ast_Type type = array_init->type.value();
		Ast_Type element_type = type.as_array->element_type;

		// check input exprs
		for (u32 i = 0; i < input_count; i += 1)
		{
			if (i < size_count)
			{
				check_expr_type(cc, array_init->input_exprs[i], element_type, context.constness);
			}
		}

		return type;
	}
	default: { err_internal("check_term: invalid Ast_Term_Tag"); return {}; }
	}
}

option<Ast_Type> check_var(Check_Context* cc, Ast_Var* var)
{
	resolve_var(cc, var);
	if (var->tag == Ast_Var_Tag::Invalid) return {};

	switch (var->tag)
	{
	case Ast_Var_Tag::Local:
	{
		option<Ast_Type> type = check_context_block_find_var_type(cc, var->local.ident);
		if (!type)
		{
			err_report(Error::VAR_LOCAL_NOT_FOUND);
			err_context(cc, var->local.ident.span);
			return {};
		}
		return check_access(cc, type.value(), var->access);
	}
	case Ast_Var_Tag::Global:
	{
		option<Ast_Type> type = var->global.global_decl->type;
		if (!type) return {};
		return check_access(cc, type.value(), var->access);
	}
	default: { err_internal("check_var: invalid Ast_Var_Tag"); return {}; }
	}
}

option<Ast_Type> check_access(Check_Context* cc, Ast_Type type, option<Ast_Access*> optional_access)
{
	if (!optional_access) return type;
	Ast_Access* access = optional_access.value();

	switch (access->tag)
	{
	case Ast_Access_Tag::Var:
	{
		Ast_Var_Access* var_access = access->as_var;

		Type_Kind kind = type_kind(type);
		if (kind == Type_Kind::Pointer && type.pointer_level == 1 && type.tag == Ast_Type_Tag::Struct) kind = Type_Kind::Struct;
		if (kind != Type_Kind::Struct)
		{
			err_set; //@Err
			printf("Field access might only be used on variables of struct or pointer to a struct type\n");
			print_span_context(cc->ast, var_access->ident.span);
			printf("Type: "); print_type(type); printf("\n");
			return {};
		}

		Ast_Struct_Decl* struct_decl = type.as_struct.struct_decl;
		option<u32> field_id = find_struct_field(struct_decl, var_access->ident);
		if (!field_id)
		{
			err_set; //@Err
			printf("Failed to find struct field during access\n");
			print_span_context(cc->ast, var_access->ident.span);
			printf("Type: "); print_type(type); printf("\n");
			return {};
		}
		var_access->field_id = field_id.value();

		Ast_Type result_type = struct_decl->fields[var_access->field_id].type;
		return check_access(cc, result_type, var_access->next);
	}
	case Ast_Access_Tag::Array:
	{
		Ast_Array_Access* array_access = access->as_array;

		//@Notice allowing pointer array access, temp, slices later
		Type_Kind kind = type_kind(type);
		if (kind == Type_Kind::Pointer && type.pointer_level == 1 && type.tag == Ast_Type_Tag::Array) kind = Type_Kind::Array;
		if (kind != Type_Kind::Array)
		{
			//@Err
			err_set;
			printf("Array access might only be used on variables of array type:\n");
			return {};
		}

		//@Notice allowing u64 for dynamic array and slice access
		//@Temp changed u64 requirement to i32 since int casting isnt implemented yet
		check_expr_type(cc, array_access->index_expr, type_from_basic(BasicType::I32), Expr_Constness::Normal);

		Ast_Type result_type = type.as_array->element_type;
		return check_access(cc, result_type, array_access->next);
	}
	default: { err_internal("check_access: invalid Ast_Access_Tag"); return {}; }
	}
}

option<Ast_Type> check_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call, Checker_Proc_Call_Flags flags)
{
	resolve_proc_call(cc, proc_call);
	if (proc_call->tag == Ast_Proc_Call_Tag::Invalid) return {};

	// check input count
	Ast_Proc_Decl* proc_decl = proc_call->resolved.proc_decl;
	u32 param_count = (u32)proc_decl->input_params.size();
	u32 input_count = (u32)proc_call->input_exprs.size();
	bool is_variadic = proc_decl->is_variadic;

	if (is_variadic)
	{
		if (input_count < param_count)
		{
			//@Err
			err_set;
			printf("Unexpected number of arguments in variadic procedure call:\n");
			printf("Expected at least: %lu Got: %lu \n", param_count, input_count);
		}
	}
	else
	{
		if (param_count != input_count)
		{
			//@Err
			err_set;
			printf("Unexpected number of arguments in procedure call:\n");
			printf("Expected: %lu Got: %lu \n", param_count, input_count);
		}
	}

	// check input exprs
	for (u32 i = 0; i < input_count; i += 1)
	{
		if (i < param_count)
		{
			Ast_Type param_type = proc_decl->input_params[i].type;
			check_expr_type(cc, proc_call->input_exprs[i], param_type, Expr_Constness::Normal);
		}
		else if (is_variadic)
		{
			check_expr_type(cc, proc_call->input_exprs[i], {}, Expr_Constness::Normal);
		}
	}

	// check access & return
	if (flags == Checker_Proc_Call_Flags::In_Expr)
	{
		if (!proc_decl->return_type)
		{
			//@Err
			err_set;
			printf("Procedure call inside expression must have a return type:\n");
			return {};
		}

		return check_access(cc, proc_decl->return_type.value(), proc_call->access);
	}
	else
	{
		if (proc_call->access)
		{
			//@Err
			err_set;
			printf("Procedure call statement cannot have access chains:\n");
			printf("\n");
		}

		if (proc_decl->return_type)
		{
			//@Err
			err_set;
			printf("Procedure call result cannot be discarded:\n");
			printf("\n");
		}

		return {};
	}
}

option<Ast_Type> check_unary_expr(Check_Context* cc, Expr_Context context, Ast_Unary_Expr* unary_expr)
{
	option<Ast_Type> rhs_result = check_expr(cc, context, unary_expr->right);
	if (!rhs_result) return {};

	UnaryOp op = unary_expr->op;
	Ast_Type rhs = rhs_result.value();
	Type_Kind rhs_kind = type_kind(rhs);

	switch (op)
	{
	case UnaryOp::MINUS:
	{
		if (rhs_kind == Type_Kind::Float || rhs_kind == Type_Kind::Integer) return rhs;
		err_set; printf("UNARY OP - only works on float or integer\n");
		return {};
	}
	case UnaryOp::LOGIC_NOT:
	{
		if (rhs_kind == Type_Kind::Bool) return rhs;
		err_set; printf("UNARY OP ! only works on bool\n");
		return {};
	}
	case UnaryOp::BITWISE_NOT:
	{
		if (rhs_kind == Type_Kind::Integer) return rhs;
		err_set; printf("UNARY OP ~ only works on integer\n");
		return {};
	}
	case UnaryOp::ADDRESS_OF:
	{
		//@Todo prevent adress of temporary values
		rhs.pointer_level += 1;
		return rhs;
	}
	case UnaryOp::DEREFERENCE:
	{
		err_set; printf("UNARY OP << unsupported\n");
		return {};
	}
	default: { err_internal("check_unary_expr: invalid UnaryOp"); return {}; }
	}
}

option<Ast_Type> check_binary_expr(Check_Context* cc, Expr_Context context, Ast_Binary_Expr* binary_expr)
{
	option<Ast_Type> lhs_result = check_expr(cc, context, binary_expr->left);
	if (!lhs_result) return {};
	option<Ast_Type> rhs_result = check_expr(cc, context, binary_expr->right);
	if (!rhs_result) return {};

	BinaryOp op = binary_expr->op;
	Ast_Type lhs = lhs_result.value();
	Type_Kind lhs_kind = type_kind(lhs);
	Ast_Type rhs = rhs_result.value();
	Type_Kind rhs_kind = type_kind(rhs);
	bool same_kind = lhs_kind == rhs_kind;

	if (!same_kind)
	{
		err_set;
		printf("Binary expr cannot be done on different type kinds\n");
		return {};
	}

	type_implicit_binary_cast(&lhs, &rhs);
	//@Todo int basic types arent accounted for during this

	switch (op)
	{
	case BinaryOp::LOGIC_AND:
	case BinaryOp::LOGIC_OR:
	{
		if (lhs_kind == Type_Kind::Bool) return rhs;
		err_set; printf("BINARY Logic Ops (&& ||) only work on bools\n");
		return {};
	}
	case BinaryOp::LESS:
	case BinaryOp::GREATER:
	case BinaryOp::LESS_EQUALS:
	case BinaryOp::GREATER_EQUALS:
	case BinaryOp::IS_EQUALS: //@Todo == != on enums
	case BinaryOp::NOT_EQUALS:
	{
		if (lhs_kind == Type_Kind::Float || lhs_kind == Type_Kind::Integer) return type_from_basic(BasicType::BOOL);
		err_set; printf("BINARY Comparison Ops (< > <= >= == !=) only work on floats or integers\n");
		return {};
	}
	case BinaryOp::PLUS:
	case BinaryOp::MINUS:
	case BinaryOp::TIMES:
	case BinaryOp::DIV:
	{
		if (lhs_kind == Type_Kind::Float || lhs_kind == Type_Kind::Integer) return lhs;
		err_set; printf("BINARY Math Ops (+ - * /) only work on floats or integers\n");
		return {};
	}
	case BinaryOp::MOD:
	{
		if (lhs_kind == Type_Kind::Integer) return lhs;
		err_set; printf("BINARY Op %% only works on integers\n");
		return {};
	}
	case BinaryOp::BITWISE_AND:
	case BinaryOp::BITWISE_OR:
	case BinaryOp::BITWISE_XOR:
	case BinaryOp::BITSHIFT_LEFT:
	case BinaryOp::BITSHIFT_RIGHT:
	{
		if (lhs_kind == Type_Kind::Integer) return lhs;
		err_set; printf("BINARY Bitwise Ops (& | ^ << >>) only work on integers\n");
		return {};
	}
	default: { err_internal("check_binary_expr: invalid BinaryOp"); return {}; }
	}
}

option<Literal> check_folded_expr(Check_Context* cc, Ast_Expr* expr)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term:
	{
		Ast_Term* term = expr->as_term;
		switch (term->tag)
		{
		case Ast_Term_Tag::Var:
		{
			//@Todo access unsupported
			Ast_Var* var = term->as_var;
			Ast_Consteval_Expr* consteval_expr = var->global.global_decl->consteval_expr;
			return check_folded_expr(cc, consteval_expr->expr);
		}
		case Ast_Term_Tag::Enum:
		{
			err_internal("Enum folding isnt supported");
			return {};
		}
		case Ast_Term_Tag::Cast:
		{
			Ast_Cast* cast = term->as_cast;
			option<Literal> lit_result = check_folded_expr(cc, cast->expr);
			if (!lit_result) return {};
			Literal lit = lit_result.value();

			switch (cast->tag)
			{
			case Ast_Cast_Tag::Integer_No_Op:            { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
			case Ast_Cast_Tag::Int_Trunc____LLVMTrunc:   { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
			case Ast_Cast_Tag::Uint_Extend__LLVMZExt:    { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
			case Ast_Cast_Tag::Int_Extend___LLVMSExt:    { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
			case Ast_Cast_Tag::Float_Uint___LLVMFPToUI: lit.as_u64 = static_cast<u64>(lit.as_f64); break;
			case Ast_Cast_Tag::Float_Int____LLVMFPToSI: lit.as_i64 = static_cast<i64>(lit.as_f64); break;
			case Ast_Cast_Tag::Uint_Float___LLVMUIToFP: lit.as_f64 = static_cast<f64>(lit.as_u64); break;
			case Ast_Cast_Tag::Int_Float____LLVMSIToFP: lit.as_f64 = static_cast<f64>(lit.as_i64); break;
			case Ast_Cast_Tag::Float_Trunc__LLVMFPTrunc: { err_report(Error::CAST_FOLD_REDUNDANT_FLOAT_CAST); err_context(cc, expr->span); } break;
			case Ast_Cast_Tag::Float_Extend_LLVMFPExt:   { err_report(Error::CAST_FOLD_REDUNDANT_FLOAT_CAST); err_context(cc, expr->span); } break;
			default: { err_internal("check_folded_expr: invalid Ast_Cast_Tag"); return {}; }
			}

			return lit;
		}
		case Ast_Term_Tag::Sizeof:
		{
			Ast_Sizeof* size_of = term->as_sizeof;
			if (type_is_poison(size_of->type)) return {};

			Literal lit = {};
			lit.kind = Literal_Kind::UInt;
			lit.as_u64 = type_size(size_of->type);
			return lit;
		}
		case Ast_Term_Tag::Literal:
		{
			Token token = term->as_literal->token;
			Literal lit = {};

			switch (token.type)
			{
			case TokenType::BOOL_LITERAL:
			{
				lit.kind = Literal_Kind::Bool;
				lit.as_bool = token.bool_value;
			} break;
			case TokenType::FLOAT_LITERAL:
			{
				lit.kind = Literal_Kind::Float;
				lit.as_f64 = token.float64_value;
			} break;
			case TokenType::INTEGER_LITERAL:
			{
				lit.kind = Literal_Kind::UInt;
				lit.as_u64 = token.integer_value;
			} break;
			default: { err_internal("check_foldable_expr: invalid TokenType"); return {}; }
			}

			return lit;
		}
		default: { err_internal("check_foldable_expr: invalid Ast_Term_Tag"); return {}; }
		}
	}
	case Ast_Expr_Tag::Unary_Expr:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		option<Literal> rhs_result = check_folded_expr(cc, unary_expr->right);
		if (!rhs_result) return {};

		UnaryOp op = unary_expr->op;
		Literal rhs = rhs_result.value();
		Literal_Kind rhs_kind = rhs.kind;

		switch (op)
		{
		case UnaryOp::MINUS:
		{
			if (rhs_kind == Literal_Kind::Bool) { printf("Cannot apply unary - to bool expression\n"); return {}; }
			if (rhs_kind == Literal_Kind::Float) { rhs.as_f64 = -rhs.as_f64; return rhs; }
			if (rhs_kind == Literal_Kind::Int) { rhs.as_i64 = -rhs.as_i64; return rhs; }

			if (rhs.as_u64 <= 1 + static_cast<u64>(std::numeric_limits<i64>::max()))
			{
				rhs.kind = Literal_Kind::Int;
				rhs.as_i64 = -static_cast<i64>(rhs.as_u64);
				return rhs;
			}
			printf("Unary - results in integer oveflow\n");
			return {};
		}
		case UnaryOp::LOGIC_NOT:
		{
			if (rhs_kind == Literal_Kind::Bool) { rhs.as_bool = !rhs.as_bool; return rhs; }
			printf("Unary ! can only be applied to bool expression\n");
			return {};
		}
		case UnaryOp::BITWISE_NOT:
		{
			if (rhs_kind == Literal_Kind::Bool) { printf("Cannot apply unary ~ to bool expression\n"); return {}; }
			if (rhs_kind == Literal_Kind::Float) { printf("Cannot apply unary ~ to float expression\n"); return {}; }
			if (rhs_kind == Literal_Kind::Int) { rhs.as_i64 = ~rhs.as_i64; return rhs; }
			rhs.as_u64 = ~rhs.as_u64; return rhs;
		}
		case UnaryOp::ADDRESS_OF:
		{
			printf("Unary adress of * cannot be used on temporary values\n");
			return {};
		}
		case UnaryOp::DEREFERENCE:
		{
			printf("Unary dereference << cannot be used on temporary values\n");
			return {};
		}
		default: { err_internal("check_foldable_expr: invalid UnaryOp"); return {}; }
		}
	}
	case Ast_Expr_Tag::Binary_Expr:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		option<Literal> lhs_result = check_folded_expr(cc, binary_expr->left);
		if (!lhs_result) return {};
		option<Literal> rhs_result = check_folded_expr(cc, binary_expr->right);
		if (!rhs_result) return {};

		BinaryOp op = binary_expr->op;
		Literal lhs = lhs_result.value();
		Literal_Kind lhs_kind = lhs.kind;
		Literal rhs = rhs_result.value();
		Literal_Kind rhs_kind = rhs.kind;
		bool same_kind = lhs_kind == rhs_kind;

		//@Need apply i64 to u64 conversion if possible

		switch (op)
		{
		case BinaryOp::LOGIC_AND:
		{
			if (same_kind && lhs_kind == Literal_Kind::Bool) return Literal{ Literal_Kind::Bool, lhs.as_bool && rhs.as_bool };
			printf("Binary && can only be applied to bool expressions\n");
			return {};
		}
		case BinaryOp::LOGIC_OR:
		{
			if (same_kind && lhs_kind == Literal_Kind::Bool) return Literal{ Literal_Kind::Bool, lhs.as_bool || rhs.as_bool };
			printf("Binary || can only be applied to bool expressions\n");
			return {};
		}
		case BinaryOp::LESS:
		{
			if (!same_kind) { printf("Binary < can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply < to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) return Literal{ Literal_Kind::Bool, lhs.as_f64 < rhs.as_f64 };
			if (lhs_kind == Literal_Kind::Int) return Literal{ Literal_Kind::Bool, lhs.as_i64 < rhs.as_i64 };
			return Literal{ Literal_Kind::Bool, lhs.as_u64 < rhs.as_u64 };
		}
		case BinaryOp::GREATER:
		{
			if (!same_kind) { printf("Binary > can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply > to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) return Literal{ Literal_Kind::Bool, lhs.as_f64 > rhs.as_f64 };
			if (lhs_kind == Literal_Kind::Int) return Literal{ Literal_Kind::Bool, lhs.as_i64 > rhs.as_i64 };
			return Literal{ Literal_Kind::Bool, lhs.as_u64 > rhs.as_u64 };
		}
		case BinaryOp::LESS_EQUALS:
		{
			if (!same_kind) { printf("Binary <= can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply <= to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) return Literal{ Literal_Kind::Bool, lhs.as_f64 <= rhs.as_f64 };
			if (lhs_kind == Literal_Kind::Int) return Literal{ Literal_Kind::Bool, lhs.as_i64 <= rhs.as_i64 };
			return Literal{ Literal_Kind::Bool, lhs.as_u64 <= rhs.as_u64 };
		}
		case BinaryOp::GREATER_EQUALS:
		{
			if (!same_kind) { printf("Binary >= can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply >= to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) return Literal{ Literal_Kind::Bool, lhs.as_f64 >= rhs.as_f64 };
			if (lhs_kind == Literal_Kind::Int) return Literal{ Literal_Kind::Bool, lhs.as_i64 >= rhs.as_i64 };
			return Literal{ Literal_Kind::Bool, lhs.as_u64 >= rhs.as_u64 };
		}
		case BinaryOp::IS_EQUALS:
		{
			if (!same_kind) { printf("Binary == can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) return Literal{ Literal_Kind::Bool, lhs.as_bool == rhs.as_bool };
			if (lhs_kind == Literal_Kind::Float) return Literal{ Literal_Kind::Bool, lhs.as_f64 == rhs.as_f64 };
			if (lhs_kind == Literal_Kind::Int) return Literal{ Literal_Kind::Bool, lhs.as_i64 == rhs.as_i64 };
			return Literal{ Literal_Kind::Bool, lhs.as_u64 == rhs.as_u64 };
		}
		case BinaryOp::NOT_EQUALS:
		{
			if (!same_kind) { printf("Binary != can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) return Literal{ Literal_Kind::Bool, lhs.as_bool != rhs.as_bool };
			if (lhs_kind == Literal_Kind::Float) return Literal{ Literal_Kind::Bool, lhs.as_f64 != rhs.as_f64 };
			if (lhs_kind == Literal_Kind::Int) return Literal{ Literal_Kind::Bool, lhs.as_i64 != rhs.as_i64 };
			return Literal{ Literal_Kind::Bool, lhs.as_u64 != rhs.as_u64 };
		}
		case BinaryOp::PLUS:
		{
			if (!same_kind) { printf("Binary + can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply + to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) { lhs.as_f64 += rhs.as_f64; return lhs; }
			if (lhs_kind == Literal_Kind::Int) { lhs.as_i64 += rhs.as_i64; return lhs; } //@Range
			lhs.as_u64 += rhs.as_u64; return lhs; //@Range
		}
		case BinaryOp::MINUS:
		{
			if (!same_kind) { printf("Binary - can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply - to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) { lhs.as_f64 -= rhs.as_f64; return lhs; }
			if (lhs_kind == Literal_Kind::Int) { lhs.as_i64 -= rhs.as_i64; return lhs; } //@Range
			lhs.as_u64 -= rhs.as_u64; return lhs; //@Undeflow
		}
		case BinaryOp::TIMES:
		{
			if (!same_kind) { printf("Binary * can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply * to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) { lhs.as_f64 *= rhs.as_f64; return lhs; }
			if (lhs_kind == Literal_Kind::Int) { lhs.as_i64 *= rhs.as_i64; return lhs; } //@Range
			lhs.as_u64 *= rhs.as_u64; return lhs; //@Range
		}
		case BinaryOp::DIV:
		{
			if (!same_kind) { printf("Binary / can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply / to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) { lhs.as_f64 /= rhs.as_f64; return lhs; } //@Nan
			if (lhs_kind == Literal_Kind::Int) { lhs.as_i64 /= rhs.as_i64; return lhs; } //@Div 0
			lhs.as_u64 /= rhs.as_u64; return lhs; //@Div 0
		}
		case BinaryOp::MOD:
		{
			if (!same_kind) { printf("Binary %% can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind == Literal_Kind::Bool) { printf("Cannot apply %% to binary expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Float) { printf("Cannot apply %% to float expressions\n"); return {}; }
			if (lhs_kind == Literal_Kind::Int) { lhs.as_i64 %= rhs.as_i64; return lhs; } //@Mod 0
			lhs.as_u64 %= rhs.as_u64; return lhs; //@Mod 0
		}
		case BinaryOp::BITWISE_AND: //@Unnesesary error messages on same_kind
		{
			if (!same_kind) { printf("Binary & can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind != Literal_Kind::UInt) { printf("Binary & can only be applied to unsigned integers\n"); return {}; }
			lhs.as_u64 &= rhs.as_u64;
			return lhs;
		}
		case BinaryOp::BITWISE_OR:
		{
			if (!same_kind) { printf("Binary | can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind != Literal_Kind::UInt) { printf("Binary | can only be applied to unsigned integers\n"); return {}; }
			lhs.as_u64 |= rhs.as_u64;
			return lhs;
		}
		case BinaryOp::BITWISE_XOR:
		{
			if (!same_kind) { printf("Binary ^ can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind != Literal_Kind::UInt) { printf("Binary ^ can only be applied to unsigned integers\n"); return {}; }
			lhs.as_u64 ^= rhs.as_u64;
			return lhs;
		}
		case BinaryOp::BITSHIFT_LEFT:
		{
			if (!same_kind) { printf("Binary << can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind != Literal_Kind::UInt) { printf("Binary << can only be applied to unsigned integers\n"); return {}; }
			//@Check bitshift amount to be 64
			lhs.as_u64 <<= rhs.as_u64;
			return lhs;
		}
		case BinaryOp::BITSHIFT_RIGHT:
		{
			if (!same_kind) { printf("Binary >> can only be applied to expressions of same kind\n"); return {}; }
			if (lhs_kind != Literal_Kind::UInt) { printf("Binary >> can only be applied to unsigned integers\n"); return {}; }
			//@Check bitshift amount to be 64
			lhs.as_u64 >>= rhs.as_u64;
			return lhs;
		}
		default: { err_internal("check_foldable_expr: invalid BinaryOp"); return {}; }
		}
	}
	case Ast_Expr_Tag::Folded_Expr:
	{
		Ast_Folded_Expr folded_expr = expr->as_folded_expr;
		Literal lit = {};

		switch (folded_expr.basic_type)
		{
		case BasicType::BOOL:
		{
			lit.kind = Literal_Kind::Bool;
			lit.as_bool = folded_expr.as_bool;
			return lit;
		}
		case BasicType::F32:
		case BasicType::F64:
		{
			lit.kind = Literal_Kind::Float;
			lit.as_f64 = folded_expr.as_f64;
			return lit;
		}
		case BasicType::I8:
		case BasicType::I16:
		case BasicType::I32:
		case BasicType::I64:
		{
			lit.kind = Literal_Kind::Int;
			lit.as_i64 = folded_expr.as_i64;
			return lit;
		}
		case BasicType::U8:
		case BasicType::U16:
		case BasicType::U32:
		case BasicType::U64:
		{
			lit.kind = Literal_Kind::UInt;
			lit.as_u64 = folded_expr.as_u64;
			return lit;
		}
		default: { err_internal("check_foldable_expr: invalid folded_expr BasicType"); return {}; }
		}
	}
	default: { err_internal("check_foldable_expr: invalid Ast_Expr_Tag"); return {}; }
	}
}

option<Ast_Type> check_apply_expr_fold(Check_Context* cc, Expr_Context context, Ast_Expr* expr, Literal lit)
{
	Ast_Folded_Expr folded = {};

	switch (lit.kind)
	{
	case Literal_Kind::Bool:
	{
		folded.basic_type = BasicType::BOOL;
		folded.as_bool = lit.as_bool;
	} break;
	case Literal_Kind::Float:
	{
		folded.basic_type = BasicType::F64;
		folded.as_f64 = lit.as_f64;

		if (context.expect_type)
		{
			Ast_Type type = context.expect_type.value();
			if (type_match(type, type_from_basic(BasicType::F32)))
			{
				folded.basic_type = BasicType::F32;
			}
		}
	} break;
	case Literal_Kind::Int:
	{
		bool default = true;

		if (context.expect_type)
		{
			Ast_Type type = context.expect_type.value();
			if (type.tag == Ast_Type_Tag::Basic)
			{
				BasicType basic_type = type.as_basic;
				default = false;

				switch (basic_type)
				{
				case BasicType::I8:  if (lit.as_i64 < INT8_MIN || lit.as_i64 > INT8_MAX) { err_internal("fold int out of range of expected i8");  err_context(cc, expr->span); return {}; } break;
				case BasicType::I16: if (lit.as_i64 < INT16_MIN || lit.as_i64 > INT16_MAX) { err_internal("fold int out of range of expected i16"); err_context(cc, expr->span); return {}; } break;
				case BasicType::I32: if (lit.as_i64 < INT32_MIN || lit.as_i64 > INT32_MAX) { err_internal("fold int out of range of expected i32"); err_context(cc, expr->span); return {}; } break;
				case BasicType::I64: break;
				case BasicType::U8:  if (lit.as_i64 < 0 || lit.as_i64 > UINT8_MAX) { err_internal("fold int out of range of expected u8");  err_context(cc, expr->span); return {}; } break;
				case BasicType::U16: if (lit.as_i64 < 0 || lit.as_i64 > UINT16_MAX) { err_internal("fold int out of range of expected u16"); err_context(cc, expr->span); return {}; } break;
				case BasicType::U32: if (lit.as_i64 < 0 || lit.as_i64 > UINT32_MAX) { err_internal("fold int out of range of expected u32"); err_context(cc, expr->span); return {}; } break;
				case BasicType::U64: if (lit.as_i64 < 0) { err_internal("fold int out of range of expected u64"); err_context(cc, expr->span); return {}; } break;
				default: default = true; break;
				}

				switch (basic_type)
				{
				case BasicType::I8:
				case BasicType::I16:
				case BasicType::I32:
				case BasicType::I64:
				{
					folded.basic_type = basic_type;
					folded.as_i64 = lit.as_i64;
				} break;
				case BasicType::U8:
				case BasicType::U16:
				case BasicType::U32:
				case BasicType::U64:
				{
					folded.basic_type = basic_type;
					folded.as_u64 = static_cast<u64>(lit.as_i64);
				} break;
				default: default = true; break;
				}
			}
		}

		if (default)
		{
			//default to: i32, i64
			if (lit.as_i64 >= INT32_MIN && lit.as_i64 <= INT32_MAX)
			{
				folded.basic_type = BasicType::I32;
				folded.as_i64 = lit.as_i64;
			}
			else
			{
				folded.basic_type = BasicType::I64;
				folded.as_i64 = lit.as_i64;
			}
		}
	} break;
	case Literal_Kind::UInt:
	{
		bool default = true;

		if (context.expect_type)
		{
			Ast_Type type = context.expect_type.value();
			if (type.tag == Ast_Type_Tag::Basic)
			{
				BasicType basic_type = type.as_basic;
				default = false;

				switch (basic_type)
				{
				case BasicType::I8:  if (lit.as_u64 > INT8_MAX) { err_internal("fold uint out of range of expected i8");  err_context(cc, expr->span); return {}; } break;
				case BasicType::I16: if (lit.as_u64 > INT16_MAX) { err_internal("fold uint out of range of expected i16"); err_context(cc, expr->span); return {}; } break;
				case BasicType::I32: if (lit.as_u64 > INT32_MAX) { err_internal("fold uint out of range of expected i32"); err_context(cc, expr->span); return {}; } break;
				case BasicType::I64: if (lit.as_u64 > INT64_MAX) { err_internal("fold uint out of range of expected i64"); err_context(cc, expr->span); return {}; } break;
				case BasicType::U8:  if (lit.as_u64 > UINT8_MAX) { err_internal("fold uint out of range of expected u8");  err_context(cc, expr->span); return {}; } break;
				case BasicType::U16: if (lit.as_u64 > UINT16_MAX) { err_internal("fold uint out of range of expected u16"); err_context(cc, expr->span); return {}; } break;
				case BasicType::U32: if (lit.as_u64 > UINT32_MAX) { err_internal("fold uint out of range of expected u32"); err_context(cc, expr->span); return {}; } break;
				case BasicType::U64: break;
				default: default = true; break;
				}

				switch (basic_type)
				{
				case BasicType::I8:
				case BasicType::I16:
				case BasicType::I32:
				case BasicType::I64:
				{
					folded.basic_type = basic_type;
					folded.as_i64 = static_cast<i64>(lit.as_u64);
				} break;
				case BasicType::U8:
				case BasicType::U16:
				case BasicType::U32:
				case BasicType::U64:
				{
					folded.basic_type = basic_type;
					folded.as_u64 = lit.as_u64;
				} break;
				default: default = true; break;
				}
			}
		}

		if (default)
		{
			//default to: i32, i64, u64
			if (lit.as_u64 <= INT32_MAX)
			{
				folded.basic_type = BasicType::I32;
				folded.as_i64 = static_cast<i64>(lit.as_u64);
			}
			else if (lit.as_u64 <= INT64_MAX)
			{
				folded.basic_type = BasicType::I64;
				folded.as_i64 = static_cast<i64>(lit.as_u64);
			}
			else
			{
				folded.basic_type = BasicType::U64;
				folded.as_u64 = lit.as_u64;
			}
		}
	} break;
	default: { err_internal("check_apply_expr_fold: invalid Literal_Kind"); return {}; }
	}

	expr->tag = Ast_Expr_Tag::Folded_Expr;
	expr->as_folded_expr = folded;
	return type_from_basic(folded.basic_type);
}

#include "general/tree.h"
#include "general/arena.h"

Consteval_Dependency consteval_dependency_from_global(Ast_Global_Decl* global_decl, Span span)
{
	Consteval_Dependency constant = {};
	constant.tag = Consteval_Dependency_Tag::Global;
	constant.as_global = Global_Dependency { global_decl, span };
	return constant;
}

Consteval_Dependency consteval_dependency_from_enum_variant(Ast_Enum_Variant* enum_variant, Span span)
{
	Consteval_Dependency constant = {};
	constant.tag = Consteval_Dependency_Tag::Enum_Variant;
	constant.as_enum_variant = Enum_Variant_Dependency { enum_variant, span };
	return constant;
}

Consteval_Dependency consteval_dependency_from_struct_size(Ast_Struct_Decl* struct_decl, Span span)
{
	Consteval_Dependency constant = {};
	constant.tag = Consteval_Dependency_Tag::Struct_Size;
	constant.as_struct_size = Struct_Size_Dependency { struct_decl, span };
	return constant;
}

Consteval_Dependency consteval_dependency_from_array_size_expr(Ast_Expr* size_expr, Ast_Type* type)
{
	Consteval_Dependency constant = {};
	constant.tag = Consteval_Dependency_Tag::Array_Size_Expr;
	constant.as_array_size = Array_Size_Dependency { size_expr, type };
	return constant;
}

void check_consteval_expr(Check_Context* cc, Consteval_Dependency constant)
{
	if (constant.tag == Consteval_Dependency_Tag::Struct_Size)
	{
		Consteval size_eval = constant.as_struct_size.struct_decl->size_eval;
		if (size_eval == Consteval::Not_Evaluated)
		{
			Tree<Consteval_Dependency> tree(2048, constant);
			Consteval dependency_eval = check_struct_size_dependencies(cc, &tree.arena, constant.as_struct_size.struct_decl, tree.root);
			if (dependency_eval == Consteval::Invalid) return;
			check_evaluate_consteval_tree(cc, tree.root);
		}
	}
	else
	{
		Ast_Consteval_Expr* consteval_expr = consteval_dependency_get_consteval_expr(constant);
		if (consteval_expr->eval == Consteval::Not_Evaluated)
		{
			Tree<Consteval_Dependency> tree(2048, constant);
			Consteval dependency_eval = check_consteval_dependencies(cc, &tree.arena, consteval_expr->expr, tree.root);
			if (dependency_eval == Consteval::Invalid) return;
			check_evaluate_consteval_tree(cc, tree.root);
		}
	}
}

Consteval check_struct_size_dependencies(Check_Context* cc, Arena* arena, Ast_Struct_Decl* struct_decl, Tree_Node<Consteval_Dependency>* parent)
{
	if (struct_decl->size_eval == Consteval::Invalid)
	{
		tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
		return Consteval::Invalid;
	}

	for (Ast_Struct_Field& field : struct_decl->fields)
	{
		resolve_type(cc, &field.type, false);
		if (type_is_poison(field.type)) 
		{
			tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
			return Consteval::Invalid;
		}

		option<Ast_Struct_Type> struct_type = type_extract_struct_value_type(field.type);
		if (struct_type)
		{
			Ast_Struct_Decl* field_struct = struct_type.value().struct_decl;
			if (field_struct->size_eval == Consteval::Invalid)
			{
				tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}

			Consteval_Dependency constant = consteval_dependency_from_struct_size(field_struct, field_struct->ident.span);
			Tree_Node<Consteval_Dependency>* node = tree_node_add_child(arena, parent, constant);
			option<Tree_Node<Consteval_Dependency>*> cycle_node = tree_node_find_cycle(node, constant, match_const_dependency);
			if (cycle_node)
			{
				err_report(Error::CONSTEVAL_DEPENDENCY_CYCLE);
				tree_node_apply_proc_in_reverse_up_to_node(node, cycle_node.value(), cc, consteval_dependency_err_context);
				tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}

			Consteval eval = check_struct_size_dependencies(cc, arena, field_struct, node);
			if (eval == Consteval::Invalid) return Consteval::Invalid;
		}

		option<Ast_Array_Type*> array_type = type_extract_array_value_type(field.type);
		while (array_type)
		{
			tree_node_add_child(arena, parent, consteval_dependency_from_array_size_expr(array_type.value()->size_expr, &field.type));
			Consteval eval = check_consteval_dependencies(cc, arena, array_type.value()->size_expr, parent);
			if (eval == Consteval::Invalid) return Consteval::Invalid;
			array_type = type_extract_array_value_type(array_type.value()->element_type);
		}
	}

	return Consteval::Not_Evaluated;
}

Consteval check_consteval_dependencies(Check_Context* cc, Arena* arena, Ast_Expr* expr, Tree_Node<Consteval_Dependency>* parent, option<Expr_Context> context)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term:
	{
		Ast_Term* term = expr->as_term;
		switch (term->tag)
		{
		case Ast_Term_Tag::Var:
		{
			//@Consider access with expressions also

			Ast_Var* var = term->as_var;
			resolve_var(cc, var);
			
			if (var->tag == Ast_Var_Tag::Invalid) 
			{
				tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}

			if (var->tag == Ast_Var_Tag::Local)
			{
				err_report(Error::CONST_VAR_IS_NOT_GLOBAL);
				err_context(cc, expr->span);
				tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}
			
			Ast_Global_Decl* global_decl = var->global.global_decl;
			Consteval eval = global_decl->consteval_expr->eval;
			if (eval == Consteval::Invalid) return Consteval::Invalid;
			if (eval == Consteval::Valid) return Consteval::Not_Evaluated;

			Consteval_Dependency constant = consteval_dependency_from_global(global_decl, expr->span);
			Tree_Node<Consteval_Dependency>* node = tree_node_add_child(arena, parent, constant);
			option<Tree_Node<Consteval_Dependency>*> cycle_node = tree_node_find_cycle(node, constant, match_const_dependency);
			if (cycle_node)
			{
				err_report(Error::CONSTEVAL_DEPENDENCY_CYCLE);
				tree_node_apply_proc_in_reverse_up_to_node(node, cycle_node.value(), cc, consteval_dependency_err_context);
				tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}

			return check_consteval_dependencies(cc, arena, global_decl->consteval_expr->expr, node);
		}
		case Ast_Term_Tag::Cast:
		{
			Ast_Cast* cast = term->as_cast;
			return check_consteval_dependencies(cc, arena, cast->expr, parent);
		}
		case Ast_Term_Tag::Enum:
		{
			Ast_Enum* _enum = term->as_enum;
			resolve_enum(cc, _enum);
			if (_enum->tag == Ast_Enum_Tag::Invalid)
			{
				tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}

			Ast_Enum_Decl* enum_decl = _enum->resolved.type.enum_decl;
			Ast_Enum_Variant* enum_variant = &enum_decl->variants[_enum->resolved.variant_id];
			Consteval eval = enum_variant->consteval_expr->eval;
			if (eval == Consteval::Invalid) return Consteval::Invalid;
			if (eval == Consteval::Valid) return Consteval::Not_Evaluated;

			Consteval_Dependency constant = consteval_dependency_from_enum_variant(enum_variant, expr->span);
			Tree_Node<Consteval_Dependency>* node = tree_node_add_child(arena, parent, constant);
			option<Tree_Node<Consteval_Dependency>*> cycle_node = tree_node_find_cycle(node, constant, match_const_dependency);
			if (cycle_node)
			{
				err_report(Error::CONSTEVAL_DEPENDENCY_CYCLE);
				tree_node_apply_proc_in_reverse_up_to_node(node, cycle_node.value(), cc, consteval_dependency_err_context);
				tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}
			
			return check_consteval_dependencies(cc, arena, enum_variant->consteval_expr->expr, node);
		}
		case Ast_Term_Tag::Sizeof:
		{
			Ast_Sizeof* size_of = term->as_sizeof;
			resolve_sizeof(cc, size_of, false);
			if (size_of->tag == Ast_Sizeof_Tag::Invalid)
			{
				tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}

			option<Ast_Struct_Type> struct_type = type_extract_struct_value_type(size_of->type);
			if (struct_type)
			{
				Consteval_Dependency constant = consteval_dependency_from_struct_size(struct_type.value().struct_decl, expr->span);
				Tree_Node<Consteval_Dependency>* node = tree_node_add_child(arena, parent, constant);
				option<Tree_Node<Consteval_Dependency>*> cycle_node = tree_node_find_cycle(node, constant, match_const_dependency);
				if (cycle_node)
				{
					err_report(Error::CONSTEVAL_DEPENDENCY_CYCLE);
					tree_node_apply_proc_in_reverse_up_to_node(node, cycle_node.value(), cc, consteval_dependency_err_context);
					tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
					return Consteval::Invalid;
				}

				Consteval eval = check_struct_size_dependencies(cc, arena, struct_type.value().struct_decl, node);
				if (eval == Consteval::Invalid) return Consteval::Invalid;
			}

			option<Ast_Array_Type*> array_type = type_extract_array_value_type(size_of->type);
			while (array_type)
			{
				tree_node_add_child(arena, parent, consteval_dependency_from_array_size_expr(array_type.value()->size_expr, &size_of->type));
				Consteval eval = check_consteval_dependencies(cc, arena, array_type.value()->size_expr, parent);
				if (eval == Consteval::Invalid) return Consteval::Invalid;
				array_type = type_extract_array_value_type(array_type.value()->element_type);
			}

			return Consteval::Not_Evaluated;
		}
		case Ast_Term_Tag::Literal:
		{
			return Consteval::Not_Evaluated;
		}
		case Ast_Term_Tag::Proc_Call:
		{
			err_report(Error::CONST_PROC_IS_NOT_CONST);
			err_context(cc, expr->span);
			tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
			return Consteval::Invalid;
		}
		case Ast_Term_Tag::Array_Init:
		{
			Ast_Array_Init* array_init = term->as_array_init;
			if (context) resolve_array_init(cc, context.value(), array_init, false);
			else resolve_array_init(cc, Expr_Context{ {}, Expr_Constness::Const }, array_init, false);
			
			if (array_init->tag == Ast_Array_Init_Tag::Invalid)
			{
				tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}

			option<Ast_Array_Type*> array_type = type_extract_array_value_type(array_init->type.value());
			while (array_type)
			{
				tree_node_add_child(arena, parent, consteval_dependency_from_array_size_expr(array_type.value()->size_expr, &array_init->type.value()));
				Consteval eval = check_consteval_dependencies(cc, arena, array_type.value()->size_expr, parent);
				if (eval == Consteval::Invalid) return Consteval::Invalid;
				array_type = type_extract_array_value_type(array_type.value()->element_type);
			}

			option<Expr_Context> element_context = {};
			if (array_init->type)
			{
				element_context = Expr_Context { array_init->type.value().as_array->element_type, Expr_Constness::Const};
			}

			for (Ast_Expr* input_expr : array_init->input_exprs)
			{
				Consteval input_eval = check_consteval_dependencies(cc, arena, input_expr, parent, element_context);
				if (input_eval == Consteval::Invalid) return Consteval::Invalid;
			}

			return Consteval::Not_Evaluated;
		}
		case Ast_Term_Tag::Struct_Init:
		{
			Ast_Struct_Init* struct_init = term->as_struct_init;
			if (context) resolve_struct_init(cc, context.value(), struct_init);
			else resolve_struct_init(cc, Expr_Context { {}, Expr_Constness::Const }, struct_init);

			if (struct_init->tag == Ast_Struct_Init_Tag::Invalid)
			{
				tree_node_apply_proc_up_to_root(parent, cc, consteval_dependency_mark_invalid);
				return Consteval::Invalid;
			}
			
			u32 count = 0;
			for (Ast_Expr* input_expr : struct_init->input_exprs)
			{
				Ast_Struct_Decl* struct_decl = struct_init->resolved.type.value().struct_decl;
				Expr_Context field_context = Expr_Context { struct_decl->fields[count].type, Expr_Constness::Const };
				count += 1;

				Consteval input_eval = check_consteval_dependencies(cc, arena, input_expr, parent, field_context);
				if (input_eval == Consteval::Invalid) return Consteval::Invalid;
			}

			return Consteval::Not_Evaluated;
		}
		}
	}
	case Ast_Expr_Tag::Unary_Expr:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		if (check_consteval_dependencies(cc, arena, unary_expr->right, parent) == Consteval::Invalid) return Consteval::Invalid;
		return Consteval::Not_Evaluated;
	}
	case Ast_Expr_Tag::Binary_Expr:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		if (check_consteval_dependencies(cc, arena, binary_expr->left, parent) == Consteval::Invalid) return Consteval::Invalid;
		if (check_consteval_dependencies(cc, arena, binary_expr->right, parent) == Consteval::Invalid) return Consteval::Invalid;
		return Consteval::Not_Evaluated;
	}
	case Ast_Expr_Tag::Folded_Expr:
	{
		return Consteval::Not_Evaluated;
	}
	default: { err_internal("check_const_expr_dependencies: invalid Ast_Expr_Tag"); return Consteval::Invalid; }
	}
}

Consteval check_evaluate_consteval_tree(Check_Context* cc, Tree_Node<Consteval_Dependency>* node)
{
	Consteval_Dependency constant = node->value;

	if (node->next_sibling != nullptr) 
	{
		Consteval eval = check_evaluate_consteval_tree(cc, node->next_sibling);
		if (eval == Consteval::Invalid) return Consteval::Invalid;
	}
	
	if (node->first_child != nullptr) 
	{
		Consteval eval = check_evaluate_consteval_tree(cc, node->first_child);
		if (eval == Consteval::Invalid) return Consteval::Invalid;
	}

	switch (constant.tag)
	{
	case Consteval_Dependency_Tag::Global:
	{
		Ast_Consteval_Expr* consteval_expr = constant.as_global.global_decl->consteval_expr;
		option<Ast_Type> type = check_expr_type(cc, consteval_expr->expr, {}, Expr_Constness::Const);
		if (!type)
		{
			tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
			return Consteval::Invalid;
		}
		consteval_expr->eval = Consteval::Valid;
		constant.as_global.global_decl->type = type.value();
	} break;
	case Consteval_Dependency_Tag::Enum_Variant:
	{
		//@Need to have access to the basic type of the enum as expected type
		Ast_Consteval_Expr* consteval_expr = constant.as_enum_variant.enum_variant->consteval_expr;
		option<Ast_Type> type = check_expr_type(cc, consteval_expr->expr, {}, Expr_Constness::Const);
		if (!type)
		{
			tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
			return Consteval::Invalid;
		}
		consteval_expr->eval = Consteval::Valid;
	} break;
	case Consteval_Dependency_Tag::Struct_Size:
	{
		compute_struct_size(constant.as_struct_size.struct_decl);
		constant.as_struct_size.struct_decl->size_eval = Consteval::Valid;
	} break;
	case Consteval_Dependency_Tag::Array_Size_Expr:
	{
		option<Ast_Type> type = check_expr_type(cc, constant.as_array_size.size_expr, type_from_basic(BasicType::U32), Expr_Constness::Const);
		if (!type)
		{
			tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
			return Consteval::Invalid;
		}
	} break;
	default: break;
	}

	return Consteval::Valid;
}

Ast_Consteval_Expr* consteval_dependency_get_consteval_expr(Consteval_Dependency constant)
{
	switch (constant.tag)
	{
	case Consteval_Dependency_Tag::Global: return constant.as_global.global_decl->consteval_expr;
	case Consteval_Dependency_Tag::Enum_Variant: return constant.as_enum_variant.enum_variant->consteval_expr;
	default: { err_internal("const_dependency_get_consteval_expr: invalid Const_Dependency_Tag"); return NULL; }
	}
}

bool match_const_dependency(Consteval_Dependency a, Consteval_Dependency b)
{
	if (a.tag != b.tag) return false;
	switch (a.tag)
	{
	case Consteval_Dependency_Tag::Global: return a.as_global.global_decl == b.as_global.global_decl;
	case Consteval_Dependency_Tag::Enum_Variant: return a.as_enum_variant.enum_variant == b.as_enum_variant.enum_variant;
	case Consteval_Dependency_Tag::Struct_Size: return a.as_struct_size.struct_decl == b.as_struct_size.struct_decl;
	case Consteval_Dependency_Tag::Array_Size_Expr: return false;
	default: { err_internal("match_const_dependency: invalid Const_Dependency_Tag"); return false; }
	}
}

void consteval_dependency_mark_invalid(Check_Context* cc, Tree_Node<Consteval_Dependency>* node)
{
	Consteval_Dependency constant = node->value;
	switch (constant.tag)
	{
	case Consteval_Dependency_Tag::Global: constant.as_global.global_decl->consteval_expr->eval = Consteval::Invalid; break;
	case Consteval_Dependency_Tag::Enum_Variant: constant.as_enum_variant.enum_variant->consteval_expr->eval = Consteval::Invalid; break;
	case Consteval_Dependency_Tag::Struct_Size: constant.as_struct_size.struct_decl->size_eval = Consteval::Invalid; break;
	case Consteval_Dependency_Tag::Array_Size_Expr: constant.as_array_size.type->tag = Ast_Type_Tag::Poison; break;
	default: err_internal("consteval_dependency_mark_invalid: invalid Const_Dependency_Tag"); break;
	}
}

void consteval_dependency_err_context(Check_Context* cc, Tree_Node<Consteval_Dependency>* node)
{
	//@Notice for sizeof struct the struct_decl ident is printed as context, might be improved to print sizeof(Ident) instead
	//preferably from the span of sizeof term which was evaluated
	Consteval_Dependency constant = node->value;
	switch (constant.tag)
	{
	case Consteval_Dependency_Tag::Global: err_context(cc, constant.as_global.span); break;
	case Consteval_Dependency_Tag::Enum_Variant: err_context(cc, constant.as_enum_variant.span); break;
	case Consteval_Dependency_Tag::Struct_Size: err_context(cc, constant.as_struct_size.span); break;
	case Consteval_Dependency_Tag::Array_Size_Expr: err_context(cc, constant.as_array_size.size_expr->span); break;
	default: err_internal("consteval_dependency_err_context: invalid Const_Dependency_Tag"); break;
	}
}

option<Ast_Struct_Info> find_struct(Ast* target_ast, Ast_Ident ident)
{
	return target_ast->struct_table.find(ident, hash_ident(ident));
}

option<Ast_Enum_Info> find_enum(Ast* target_ast, Ast_Ident ident)
{
	return target_ast->enum_table.find(ident, hash_ident(ident));
}

option<Ast_Proc_Info> find_proc(Ast* target_ast, Ast_Ident ident)
{
	return target_ast->proc_table.find(ident, hash_ident(ident));
}

option<Ast_Global_Info> find_global(Ast* target_ast, Ast_Ident ident)
{
	return target_ast->global_table.find(ident, hash_ident(ident));
}

option<u32> find_enum_variant(Ast_Enum_Decl* enum_decl, Ast_Ident ident)
{
	for (u64 i = 0; i < enum_decl->variants.size(); i += 1)
	{
		if (match_ident(enum_decl->variants[i].ident, ident)) return (u32)i;
	}
	return {};
}

option<u32> find_struct_field(Ast_Struct_Decl* struct_decl, Ast_Ident ident)
{
	for (u64 i = 0; i < struct_decl->fields.size(); i += 1)
	{
		if (match_ident(struct_decl->fields[i].ident, ident)) return (u32)i;
	}
	return {};
}

Ast* resolve_import(Check_Context* cc, option<Ast_Ident> import)
{
	if (!import) return cc->ast;
	Ast_Ident import_ident = import.value();
	option<Ast_Import_Decl*> import_decl = cc->ast->import_table.find(import_ident, hash_ident(import_ident));
	if (!import_decl)
	{
		err_report(Error::RESOLVE_IMPORT_NOT_FOUND);
		err_context(cc, import_ident.span);
		return NULL;
	}
	return import_decl.value()->import_ast;
}

//@Todo think about when the size_expr needs to be resolved during conteval expr resolution
void resolve_type(Check_Context* cc, Ast_Type* type, bool check_array_size)
{
	switch (type->tag)
	{
	case Ast_Type_Tag::Basic: break;
	case Ast_Type_Tag::Array:
	{
		if (check_array_size)
		{
			Ast_Expr* size_expr = type->as_array->size_expr;
			
			option<Ast_Type> size_type = check_expr_type(cc, size_expr, type_from_basic(BasicType::U32), Expr_Constness::Const);
			if (!size_type)
			{
				type->tag = Ast_Type_Tag::Poison;
				return;
			}
			
			Ast_Folded_Expr size_folded = size_expr->as_folded_expr;
			if (size_folded.as_u64 == 0) 
			{
				err_report(Error::RESOLVE_TYPE_ARRAY_ZERO_SIZE);
				err_context(cc, size_expr->span);
				type->tag = Ast_Type_Tag::Poison;
				return;
			}
		}

		Ast_Type* element_type = &type->as_array->element_type;
		resolve_type(cc, element_type, check_array_size);
		if (type_is_poison(*element_type)) type->tag = Ast_Type_Tag::Poison;
	} break;
	case Ast_Type_Tag::Struct: break;
	case Ast_Type_Tag::Enum: break;
	case Ast_Type_Tag::Unresolved:
	{
		Ast* target_ast = resolve_import(cc, type->as_unresolved->import);
		if (target_ast == NULL) 
		{
			type->tag = Ast_Type_Tag::Poison;
			return;
		}

		option<Ast_Struct_Info> struct_info = find_struct(target_ast, type->as_unresolved->ident);
		if (struct_info)
		{
			type->tag = Ast_Type_Tag::Struct;
			type->as_struct.struct_id = struct_info.value().struct_id;
			type->as_struct.struct_decl = struct_info.value().struct_decl;
			return;
		}
		
		option<Ast_Enum_Info> enum_info = find_enum(target_ast, type->as_unresolved->ident);
		if (enum_info)
		{
			type->tag = Ast_Type_Tag::Enum;
			type->as_enum.enum_id = enum_info.value().enum_id;
			type->as_enum.enum_decl = enum_info.value().enum_decl;
			return;
		}

		type->tag = Ast_Type_Tag::Poison;
		err_report(Error::RESOLVE_TYPE_NOT_FOUND);
		err_context(cc, type->span);
	} break;
	case Ast_Type_Tag::Poison: break;
	}
}

void resolve_var(Check_Context* cc, Ast_Var* var)
{
	if (var->tag != Ast_Var_Tag::Unresolved) return;

	Ast_Ident ident = var->unresolved.ident;
	Ast* target_ast = cc->ast;

	option<Ast_Import_Decl*> import_decl = cc->ast->import_table.find(ident, hash_ident(ident));
	if (import_decl && var->access && var->access.value()->tag == Ast_Access_Tag::Var)
	{
		Ast_Var_Access* var_access = var->access.value()->as_var;
		Ast_Ident global_ident = var_access->ident;
		var->access = var_access->next;

		target_ast = import_decl.value()->import_ast;
		if (target_ast == NULL)
		{
			var->tag = Ast_Var_Tag::Invalid;
			return;
		}

		option<Ast_Global_Info> global = find_global(target_ast, global_ident);
		if (global) 
		{
			var->tag = Ast_Var_Tag::Global;
			var->global.global_id = global.value().global_id;
			var->global.global_decl = global.value().global_decl;
			return;
		}
		else
		{
			err_report(Error::RESOLVE_VAR_GLOBAL_NOT_FOUND);
			err_context(cc, global_ident.span);
			var->tag = Ast_Var_Tag::Invalid;
			return;
		}
	}
	else
	{
		option<Ast_Global_Info> global = find_global(target_ast, ident);
		if (global)
		{
			var->tag = Ast_Var_Tag::Global;
			var->global.global_id = global.value().global_id;
			var->global.global_decl = global.value().global_decl;
			return;
		}
	}

	var->tag = Ast_Var_Tag::Local;
	var->local.ident = var->unresolved.ident;
}

void resolve_enum(Check_Context* cc, Ast_Enum* _enum)
{
	if (_enum->tag != Ast_Enum_Tag::Unresolved) return;

	Ast* target_ast = resolve_import(cc, _enum->unresolved.import);
	if (target_ast == NULL) 
	{
		_enum->tag = Ast_Enum_Tag::Invalid; 
		return;
	}

	option<Ast_Enum_Info> enum_info = find_enum(target_ast, _enum->unresolved.ident);
	if (!enum_info)
	{
		err_report(Error::RESOLVE_ENUM_NOT_FOUND);
		err_context(cc, _enum->unresolved.ident.span);
		_enum->tag = Ast_Enum_Tag::Invalid;
		return;
	}

	Ast_Enum_Decl* enum_decl = enum_info.value().enum_decl;
	option<u32> variant_id = find_enum_variant(enum_decl, _enum->unresolved.variant);
	if (!variant_id)
	{
		err_report(Error::RESOLVE_ENUM_VARIANT_NOT_FOUND);
		err_context(cc, _enum->unresolved.variant.span);
		_enum->tag = Ast_Enum_Tag::Invalid;
		return;
	}

	_enum->tag = Ast_Enum_Tag::Resolved;
	_enum->resolved.type = Ast_Enum_Type { enum_info.value().enum_id, enum_info.value().enum_decl };
	_enum->resolved.variant_id = variant_id.value();
}

void resolve_cast(Check_Context* cc, Expr_Context context, Ast_Cast* cast)
{
	option<Ast_Type> type = check_expr_type(cc, cast->expr, {}, context.constness);
	if (!type) 
	{ 
		cast->tag = Ast_Cast_Tag::Invalid; 
		return; 
	}

	if (type.value().tag != Ast_Type_Tag::Basic)
	{
		err_report(Error::CAST_EXPR_NON_BASIC_TYPE);
		printf("Type: "); print_type(type.value());
		err_context(cc, cast->expr->span);
		cast->tag = Ast_Cast_Tag::Invalid;
		return;
	}

	BasicType expr_type = type.value().as_basic;
	if (expr_type == BasicType::BOOL)   { err_report(Error::CAST_EXPR_BOOL_BASIC_TYPE);   err_context(cc, cast->expr->span); cast->tag = Ast_Cast_Tag::Invalid; return; }
	if (expr_type == BasicType::STRING) { err_report(Error::CAST_EXPR_STRING_BASIC_TYPE); err_context(cc, cast->expr->span); cast->tag = Ast_Cast_Tag::Invalid; return; }
	
	BasicType cast_into = cast->basic_type;
	if (cast_into == BasicType::BOOL)   { err_report(Error::CAST_INTO_BOOL_BASIC_TYPE);   err_context(cc, cast->expr->span); cast->tag = Ast_Cast_Tag::Invalid; return; }
	if (cast_into == BasicType::STRING) { err_report(Error::CAST_INTO_STRING_BASIC_TYPE); err_context(cc, cast->expr->span); cast->tag = Ast_Cast_Tag::Invalid; return; }

	Literal_Kind cast_kind = {};
	Literal_Kind expr_kind = {};

	switch (cast_into)
	{
	case BasicType::I8:
	case BasicType::I16:
	case BasicType::I32:
	case BasicType::I64: cast_kind = Literal_Kind::Int; break;
	case BasicType::U8:
	case BasicType::U16:
	case BasicType::U32:
	case BasicType::U64: cast_kind = Literal_Kind::UInt; break;
	case BasicType::F32:
	case BasicType::F64: cast_kind = Literal_Kind::Float; break;
	default: 
	{
		err_internal("resolve_cast: invalid cast_type BasicType");
		err_context(cc, cast->expr->span);
		cast->tag = Ast_Cast_Tag::Invalid;
		return;
	}
	}

	switch (expr_type)
	{
	case BasicType::I8:
	case BasicType::I16:
	case BasicType::I32:
	case BasicType::I64: expr_kind = Literal_Kind::Int; break;
	case BasicType::U8:
	case BasicType::U16:
	case BasicType::U32:
	case BasicType::U64: expr_kind = Literal_Kind::UInt; break;
	case BasicType::F32:
	case BasicType::F64: expr_kind = Literal_Kind::Float; break;
	default:
	{
		err_internal("resolve_cast: invalid expr_type BasicType");
		err_context(cc, cast->expr->span);
		cast->tag = Ast_Cast_Tag::Invalid;
		return;
	}
	}

	u32 cast_into_size = type_basic_size(cast_into);
	u32 expr_type_size = type_basic_size(expr_type);

	if (expr_type == cast_into && cast_into_size && expr_type_size)
	{
		switch (expr_kind)
		{
		case Literal_Kind::Float:
		{
			err_report(Error::CAST_REDUNDANT_FLOAT_CAST);
			err_context(cc, cast->expr->span);
			cast->tag = Ast_Cast_Tag::Invalid;
			return;
		}
		case Literal_Kind::Int:
		case Literal_Kind::UInt:
		{
			err_report(Error::CAST_REDUNDANT_INTEGER_CAST); 
			err_context(cc, cast->expr->span); 
			cast->tag = Ast_Cast_Tag::Invalid; 
			return;
		}
		default:
		{
			err_internal("resolve_cast: invalid expr_kind Literal_Kind"); 
			err_context(cc, cast->expr->span); 
			cast->tag = Ast_Cast_Tag::Invalid; 
			return;
		}
		}
	}

	switch (expr_kind)
	{
	case Literal_Kind::Float:
	{
		switch (cast_kind)
		{
		case Literal_Kind::Float:
		{
			if (cast_into_size > expr_type_size)      cast->tag = Ast_Cast_Tag::Float_Extend_LLVMFPExt;
			else cast->tag = Ast_Cast_Tag::Float_Trunc__LLVMFPTrunc;
		} break;
		case Literal_Kind::Int:  cast->tag = Ast_Cast_Tag::Float_Int____LLVMFPToSI; break;
		case Literal_Kind::UInt: cast->tag = Ast_Cast_Tag::Float_Uint___LLVMFPToUI; break;
		}
	} break;
	case Literal_Kind::Int:
	{
		switch (cast_kind)
		{
		case Literal_Kind::Float: cast->tag = Ast_Cast_Tag::Int_Float____LLVMSIToFP; break;
		case Literal_Kind::Int:
		{
			if (cast_into_size > expr_type_size) cast->tag = Ast_Cast_Tag::Int_Extend___LLVMSExt;
			else cast->tag = Ast_Cast_Tag::Int_Trunc____LLVMTrunc;
		} break;
		case Literal_Kind::UInt:
		{
			if (cast_into_size == expr_type_size) cast->tag = Ast_Cast_Tag::Integer_No_Op;
			else if (cast_into_size > expr_type_size) cast->tag = Ast_Cast_Tag::Int_Extend___LLVMSExt;
			else cast->tag = Ast_Cast_Tag::Int_Trunc____LLVMTrunc;
		} break;
		}
	} break;
	case Literal_Kind::UInt:
	{
		switch (cast_kind)
		{
		case Literal_Kind::Float: cast->tag = Ast_Cast_Tag::Uint_Float___LLVMUIToFP; break;
		case Literal_Kind::Int:
		{
			if (cast_into_size == expr_type_size) cast->tag = Ast_Cast_Tag::Integer_No_Op;
			else if (cast_into_size > expr_type_size) cast->tag = Ast_Cast_Tag::Uint_Extend__LLVMZExt;
			else cast->tag = Ast_Cast_Tag::Int_Trunc____LLVMTrunc;
		} break;
		case Literal_Kind::UInt:
		{
			if (cast_into_size > expr_type_size) cast->tag = Ast_Cast_Tag::Uint_Extend__LLVMZExt;
			else cast->tag = Ast_Cast_Tag::Int_Trunc____LLVMTrunc;
		} break;
		}
	} break;
	}
}

void resolve_sizeof(Check_Context* cc, Ast_Sizeof* size_of, bool check_array_size_expr)
{
	resolve_type(cc, &size_of->type, check_array_size_expr);
	if (type_is_poison(size_of->type))
	{
		size_of->tag = Ast_Sizeof_Tag::Invalid;
		return;
	}
	else size_of->tag = Ast_Sizeof_Tag::Resolved;
}

void resolve_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call)
{
	if (proc_call->tag != Ast_Proc_Call_Tag::Unresolved) return;

	Ast* target_ast = resolve_import(cc, proc_call->unresolved.import);
	if (target_ast == NULL)
	{
		proc_call->tag = Ast_Proc_Call_Tag::Invalid;
		return;
	}

	option<Ast_Proc_Info> proc_info = find_proc(target_ast, proc_call->unresolved.ident);
	if (!proc_info)
	{
		err_report(Error::RESOLVE_PROC_NOT_FOUND);
		err_context(cc, proc_call->unresolved.ident.span);
		proc_call->tag = Ast_Proc_Call_Tag::Invalid;
		return;
	}

	Ast_Proc_Decl* proc_decl = proc_info.value().proc_decl;
	if (proc_decl->return_type)
	{
		if (type_is_poison(proc_decl->return_type.value()))
		{
			proc_call->tag = Ast_Proc_Call_Tag::Invalid;
			return;
		}
	}

	proc_call->tag = Ast_Proc_Call_Tag::Resolved;
	proc_call->resolved.proc_id = proc_info.value().proc_id;
	proc_call->resolved.proc_decl = proc_info.value().proc_decl;
}

//@Think about what happenes to poisoned type of array init when its part of consteval tree resolution
void resolve_array_init(Check_Context* cc, Expr_Context context, Ast_Array_Init* array_init, bool check_array_size_expr)
{
	if (array_init->type)
	{
		resolve_type(cc, &array_init->type.value(), check_array_size_expr);
		if (type_is_poison(array_init->type.value()))
		{
			array_init->tag = Ast_Array_Init_Tag::Invalid;
			return;
		}
		else array_init->tag = Ast_Array_Init_Tag::Resolved;
	}

	if (context.expect_type)
	{
		Ast_Type expect_type = context.expect_type.value();
		if (type_kind(expect_type) != Type_Kind::Array)
		{
			//@err_context
			err_report(Error::RESOLVE_ARRAY_WRONG_CONTEXT);
			print_type(expect_type); printf("\n");
			array_init->tag = Ast_Array_Init_Tag::Invalid;
			return;
		}

		if (array_init->type)
		{
			if (!type_match(array_init->type.value(), expect_type))
			{
				//@err_context
				err_report(Error::RESOLVE_ARRAY_TYPE_MISMATCH);
				printf("Expected: "); print_type(expect_type); printf("\n");
				printf("Got:      "); print_type(array_init->type.value()); printf("\n");
				array_init->tag = Ast_Array_Init_Tag::Invalid;
				return;
			}
		}
		else array_init->type = expect_type;
	}

	if (!array_init->type)
	{
		//@err_context
		err_report(Error::RESOLVE_ARRAY_NO_CONTEXT);
		array_init->tag = Ast_Array_Init_Tag::Invalid;
		return;
	}
}

void resolve_struct_init(Check_Context* cc, Expr_Context context, Ast_Struct_Init* struct_init)
{
	if (struct_init->tag != Ast_Struct_Init_Tag::Unresolved) return;

	Ast* target_ast = resolve_import(cc, struct_init->unresolved.import);
	if (target_ast == NULL)
	{
		struct_init->tag = Ast_Struct_Init_Tag::Invalid;
		return;
	}

	if (struct_init->unresolved.ident)
	{
		option<Ast_Struct_Info> struct_info = find_struct(target_ast, struct_init->unresolved.ident.value());
		if (!struct_info)
		{
			err_report(Error::RESOLVE_STRUCT_NOT_FOUND);
			err_context(cc, struct_init->unresolved.ident.value().span);
			struct_init->tag = Ast_Struct_Init_Tag::Invalid;
			return;
		}

		struct_init->tag = Ast_Struct_Init_Tag::Resolved;
		struct_init->resolved.type = Ast_Struct_Type { struct_info.value().struct_id, struct_info.value().struct_decl };
	}
	else
	{
		struct_init->tag = Ast_Struct_Init_Tag::Resolved;
		struct_init->resolved.type = {};
	}

	if (context.expect_type)
	{
		Ast_Type expect_type = context.expect_type.value();
		if (type_kind(expect_type) != Type_Kind::Struct)
		{
			//@err_context
			err_report(Error::RESOLVE_STRUCT_WRONG_CONTEXT);
			print_type(expect_type); printf("\n");
			struct_init->tag = Ast_Struct_Init_Tag::Invalid;
			return;
		}

		Ast_Struct_Type expected_struct = expect_type.as_struct;
		if (struct_init->resolved.type)
		{
			Ast_Struct_Type struct_type = struct_init->resolved.type.value();
			if (struct_type.struct_id != expected_struct.struct_id)
			{
				//@err_context
				err_report(Error::RESOLVE_STRUCT_TYPE_MISMATCH);
				struct_init->tag = Ast_Struct_Init_Tag::Invalid;
				return;
			}
		}
		else struct_init->resolved.type = expected_struct;
	}

	if (!struct_init->resolved.type)
	{
		//@err_context
		err_report(Error::RESOLVE_STRUCT_NO_CONTEXT);
		struct_init->tag = Ast_Struct_Init_Tag::Invalid;
		return;
	}
}
