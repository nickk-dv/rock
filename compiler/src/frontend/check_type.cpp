#include "check_type.h"

#include "error_handler.h"
#include "printer.h"
#include "check_constfold.h"

Type_Kind type_kind(Ast_Type type)
{
	if (type.pointer_level > 0) return Type_Kind::Pointer;

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic:
	{
		switch (type.as_basic)
		{
		case BasicType::I8:
		case BasicType::I16:
		case BasicType::I32:
		case BasicType::I64: return Type_Kind::Int;
		case BasicType::U8:
		case BasicType::U16:
		case BasicType::U32:
		case BasicType::U64: return Type_Kind::Uint;
		case BasicType::F32:
		case BasicType::F64: return Type_Kind::Float;
		case BasicType::BOOL: return Type_Kind::Bool;
		case BasicType::STRING: return Type_Kind::String;
		default: { err_internal("type_kind: invalid Ast_Type_Tag"); return Type_Kind::Bool; }
		}
	}
	case Ast_Type_Tag::Enum: return Type_Kind::Enum;
	case Ast_Type_Tag::Struct: return Type_Kind::Struct;
	case Ast_Type_Tag::Array: return Type_Kind::Array;
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

option<BasicType> type_extract_basic_and_enum_type(Ast_Type type)
{
	if (type.pointer_level > 0) return {};

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic: return type.as_basic;
	case Ast_Type_Tag::Enum: return type.as_enum.enum_decl->basic_type;
	default: return {};
	}
}

option<Ast_Type_Struct> type_extract_struct_value_type(Ast_Type type)
{
	if (type.pointer_level > 0) return {};

	switch (type.tag)
	{
	case Ast_Type_Tag::Struct: return type.as_struct;
	case Ast_Type_Tag::Array: return type_extract_struct_value_type(type.as_array->element_type);
	default: return {};
	}
}

option<Ast_Type_Array*> type_extract_array_type(Ast_Type type)
{
	if (type.pointer_level > 0) return {};
	
	switch (type.tag)
	{
	case Ast_Type_Tag::Array: return type.as_array;
	default: return {};
	}
}

void compute_struct_size(Ast_Decl_Struct* struct_decl)
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
	case Ast_Type_Tag::Enum: return type_basic_size(type.as_enum.enum_decl->basic_type);
	case Ast_Type_Tag::Struct:
	{
		if (type.as_struct.struct_decl->size_eval != Consteval::Valid)
			err_internal("type_size: expected struct size_eval to be Consteval::Valid");
		return type.as_struct.struct_decl->struct_size;
	}
	case Ast_Type_Tag::Array: 
	{
		if (type.as_array->size_expr->tag != Ast_Expr_Tag::Folded || type.as_array->size_expr->as_folded_expr.basic_type != BasicType::U32)
			err_internal("type_size: array size expr is not folded or has incorrect type");
		u32 count = (u32)type.as_array->size_expr->as_folded_expr.as_u64;
		return count * type_size(type.as_array->element_type);
	}
	default: { err_internal("check_get_type_size: invalid Ast_Type_Tag"); return 0; }
	}
}

u32 type_align(Ast_Type type)
{
	if (type.pointer_level > 0) return 8; //@Assume 64bit

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic: return type_basic_align(type.as_basic);
	case Ast_Type_Tag::Enum: return type_basic_align(type.as_enum.enum_decl->basic_type);
	case Ast_Type_Tag::Struct: 
	{
		if (type.as_struct.struct_decl->size_eval != Consteval::Valid) 
			err_internal("type_align: expected struct size_eval to be Consteval::Valid");
		return type.as_struct.struct_decl->max_align;
	}
	case Ast_Type_Tag::Array: return type_align(type.as_array->element_type);
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

	type_auto_cast(&type.value(), expect_type.value(), expr);

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
	case Ast_Type_Tag::Enum: return type_a.as_enum.enum_id == type_b.as_enum.enum_id;
	case Ast_Type_Tag::Struct: return type_a.as_struct.struct_id == type_b.as_struct.struct_id;
	case Ast_Type_Tag::Array:
	{
		Ast_Type_Array* array_a = type_a.as_array;
		Ast_Type_Array* array_b = type_b.as_array;

		if (array_a->size_expr->tag != Ast_Expr_Tag::Folded || array_b->size_expr->tag != Ast_Expr_Tag::Folded) 
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

void type_auto_cast(Ast_Type* type, Ast_Type expect_type, Ast_Expr* expr)
{
	if (type->tag != Ast_Type_Tag::Basic) return;
	if (expect_type.tag != Ast_Type_Tag::Basic) return;
	if (type->as_basic == expect_type.as_basic) return;
	
	Type_Kind kind = type_kind(*type);
	Type_Kind expect_kind = type_kind(expect_type);

	if (kind == Type_Kind::Float && expect_kind == Type_Kind::Float)
	{
		if (type->as_basic == BasicType::F32)
		{
			type->as_basic = BasicType::F64;
			expr->flags |= AST_EXPR_FLAG_AUTO_CAST_F32_F64_BIT;
		}
		else
		{
			type->as_basic = BasicType::F32;
			expr->flags |= AST_EXPR_FLAG_AUTO_CAST_F64_F32_BIT;
		}
		return;
	}

	u32 size = type_basic_size(type->as_basic);
	u32 expect_size = type_basic_size(expect_type.as_basic);
	if (expect_size <= size) return;
	
	if (kind == Type_Kind::Int && expect_kind == Type_Kind::Int)
	{
		expr->flags |= AST_EXPR_FLAG_AUTO_CAST_INT_SEXT_BIT;
		switch (expect_type.as_basic)
		{
		case BasicType::I16: { type->as_basic = BasicType::I16; expr->flags |= AST_EXPR_FLAG_AUTO_CAST_TO_INT_16_BIT; return; }
		case BasicType::I32: { type->as_basic = BasicType::I32; expr->flags |= AST_EXPR_FLAG_AUTO_CAST_TO_INT_32_BIT; return; }
		case BasicType::I64: { type->as_basic = BasicType::I64; expr->flags |= AST_EXPR_FLAG_AUTO_CAST_TO_INT_64_BIT; return; }
		default: { err_internal("type_auto_cast: invalid int upcast BasicType"); return; }
		}
	}
	
	if (kind == Type_Kind::Uint && expect_kind == Type_Kind::Uint)
	{
		expr->flags |= AST_EXPR_FLAG_AUTO_CAST_UINT_ZEXT_BIT;
		switch (expect_type.as_basic)
		{
		case BasicType::U16: { type->as_basic = BasicType::U16; expr->flags |= AST_EXPR_FLAG_AUTO_CAST_TO_INT_16_BIT; return; }
		case BasicType::U32: { type->as_basic = BasicType::U32; expr->flags |= AST_EXPR_FLAG_AUTO_CAST_TO_INT_32_BIT; return; }
		case BasicType::U64: { type->as_basic = BasicType::U64; expr->flags |= AST_EXPR_FLAG_AUTO_CAST_TO_INT_64_BIT; return; }
		default: { err_internal("type_auto_cast: invalid uint upcast BasicType"); return; }
		}
	}
}

void type_auto_binary_cast(Ast_Type* lhs_type, Ast_Type* rhs_type, Ast_Expr* lhs_expr, Ast_Expr* rhs_expr)
{
	if (lhs_type->tag != Ast_Type_Tag::Basic) return;
	if (rhs_type->tag != Ast_Type_Tag::Basic) return;
	if (lhs_type->as_basic == rhs_type->as_basic) return;
	
	Type_Kind lhs_kind = type_kind(*lhs_type);
	Type_Kind rhs_kind = type_kind(*rhs_type);

	if (lhs_kind == rhs_kind)
	{
		u32 lhs_size = type_basic_size(lhs_type->as_basic);
		u32 rhs_size = type_basic_size(rhs_type->as_basic);
		if (lhs_size < rhs_size) type_auto_cast(lhs_type, *rhs_type, lhs_expr);
		else type_auto_cast(rhs_type, *lhs_type, rhs_expr);
	}
}

option<Ast_Type> check_expr(Check_Context* cc, Expr_Context context, Ast_Expr* expr)
{
	option<Expr_Kind> kind_result = resolve_expr(cc, context, expr);
	if (!kind_result) return {};
	Expr_Kind kind = kind_result.value();
	
	if (kind == Expr_Kind::Const || kind == Expr_Kind::Constfold)
	{
		expr->flags |= AST_EXPR_FLAG_CONST_BIT;
	}

	if (context.constness == Expr_Constness::Const && (expr->flags & AST_EXPR_FLAG_CONST_BIT) == 0)
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
		case Ast_Expr_Tag::Term: return check_term(cc, context, expr->as_term, expr);
		case Ast_Expr_Tag::Unary: return check_unary_expr(cc, context, expr->as_unary_expr, expr);
		case Ast_Expr_Tag::Binary: return check_binary_expr(cc, context, expr->as_binary_expr, expr);
		default: { err_internal("check_expr: invalid Ast_Expr_Tag"); return {}; }
		}
	}
	case Expr_Kind::Constfold:
	{
		if (expr->tag == Ast_Expr_Tag::Folded)
		{
			//@Debug potential multi check errors with multi dimentional arrays etc
			//printf("Folding folded expr\n");
			//err_context(cc, expr->span);
			return type_from_basic(expr->as_folded_expr.basic_type);
		}

		return check_constfold_expr(cc, context, expr);
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
			case Ast_Resolve_Var_Tag::Resolved_Local:
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
			case Ast_Resolve_Var_Tag::Resolved_Global:
			{
				Ast_Decl_Global* global_decl = var->global.global_decl;
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
			if (_enum->tag != Ast_Resolve_Tag::Resolved) return {};

			Ast_Decl_Enum* enum_decl = _enum->resolved.type.enum_decl;
			Ast_Enum_Variant* enum_variant = &enum_decl->variants[_enum->resolved.variant_id];
			Ast_Consteval_Expr* consteval_expr = enum_variant->consteval_expr;
			if (consteval_expr->eval != Consteval::Valid) return {};

			return Expr_Kind::Constfold;
		}
		case Ast_Term_Tag::Cast:
		{
			Ast_Cast* cast = term->as_cast;
			resolve_cast(cc, context, cast);
			if (cast->tag == Ast_Resolve_Cast_Tag::Invalid) return {};

			return resolve_expr(cc, context, cast->expr);
		}
		case Ast_Term_Tag::Sizeof:
		{
			Ast_Sizeof* size_of = term->as_sizeof;
			resolve_sizeof(cc, size_of, true);
			if (size_of->tag != Ast_Resolve_Tag::Resolved) return {};
			
			option<Ast_Type_Struct> struct_type = type_extract_struct_value_type(size_of->type);
			if (struct_type && struct_type.value().struct_decl->size_eval != Consteval::Valid) return {};

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
			if (proc_call->tag != Ast_Resolve_Tag::Resolved) return {};
			return Expr_Kind::Normal;
		}
		case Ast_Term_Tag::Array_Init:
		{
			Ast_Array_Init* array_init = term->as_array_init;
			resolve_array_init(cc, context, array_init, true);
			if (array_init->tag != Ast_Resolve_Tag::Resolved) return {};

			Expr_Kind kind = Expr_Kind::Const;

			Expr_Context input_context = Expr_Context{ {}, context.constness };
			if (array_init->type)
			{
				Ast_Type_Array* array = array_init->type.value().as_array;
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
			if (struct_init->tag != Ast_Resolve_Tag::Resolved) return {};

			Expr_Kind kind = Expr_Kind::Const;

			Ast_Decl_Struct* struct_decl = struct_init->resolved.type.value().struct_decl;
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
	case Ast_Expr_Tag::Unary:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		option<Expr_Kind> rhs_kind = resolve_expr(cc, context, unary_expr->right);
		return rhs_kind;
	}
	case Ast_Expr_Tag::Binary:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		option<Expr_Kind> lhs_kind = resolve_expr(cc, context, binary_expr->left);
		option<Expr_Kind> rhs_kind = resolve_expr(cc, context, binary_expr->right);
		if (!lhs_kind || !rhs_kind) return {};
		return lhs_kind.value() < rhs_kind.value() ? lhs_kind.value() : rhs_kind.value();
	}
	case Ast_Expr_Tag::Folded:
	{
		return Expr_Kind::Constfold;
	}
	default: { err_internal("resolve_expr: invalid Ast_Expr_Tag"); return {}; }
	}
}

//@TODO temp allowing old errors:
#define err_set (void)0;

option<Ast_Type> check_term(Check_Context* cc, Expr_Context context, Ast_Term* term, Ast_Expr* source_expr)
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
		if (struct_init->tag == Ast_Resolve_Tag::Invalid) return {};

		// check input count
		Ast_Decl_Struct* struct_decl = struct_init->resolved.type.value().struct_decl;
		u32 field_count = (u32)struct_decl->fields.size();
		u32 input_count = (u32)struct_init->input_exprs.size();
		if (field_count != input_count)
		{
			//@Err
			err_set;
			err_internal("Unexpected number of fields in struct initializer");
			printf("Expected: %lu Got: %lu \n", field_count, input_count);
			err_context(cc, source_expr->span);
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
		if (array_init->tag == Ast_Resolve_Tag::Invalid) return {};

		if (array_init->type.value().as_array->size_expr->tag != Ast_Expr_Tag::Folded)
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
			err_internal("Unexpected number of fields in array initializer");
			printf("Expected: %lu Got: %lu \n", size_count, input_count);
			err_context(cc, source_expr->span);
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
	if (var->tag == Ast_Resolve_Var_Tag::Invalid) return {};

	switch (var->tag)
	{
	case Ast_Resolve_Var_Tag::Resolved_Local:
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
	case Ast_Resolve_Var_Tag::Resolved_Global:
	{
		option<Ast_Type> type = var->global.global_decl->type;
		if (!type) return {};
		return check_access(cc, type.value(), var->access);
	}
	default: { err_internal("check_var: invalid Ast_Resolve_Var_Tag"); return {}; }
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
		Ast_Access_Var* var_access = access->as_var;

		Type_Kind kind = type_kind(type);
		if (kind == Type_Kind::Pointer && type.pointer_level == 1 && type.tag == Ast_Type_Tag::Struct) kind = Type_Kind::Struct;
		if (kind != Type_Kind::Struct)
		{
			err_set; //@Err
			printf("Field access might only be used on variables of struct or pointer to a struct type\n");
			print_span_context(cc->ast, var_access->unresolved.ident.span);
			printf("Type: "); print_type(type); printf("\n");
			return {};
		}

		Ast_Decl_Struct* struct_decl = type.as_struct.struct_decl;
		option<u32> field_id = find_struct_field(struct_decl, var_access->unresolved.ident);
		if (!field_id)
		{
			err_set; //@Err
			printf("Failed to find struct field during access\n");
			print_span_context(cc->ast, var_access->unresolved.ident.span);
			printf("Type: "); print_type(type); printf("\n");
			return {};
		}
		var_access->resolved.field_id = field_id.value();

		Ast_Type result_type = struct_decl->fields[var_access->resolved.field_id].type;
		return check_access(cc, result_type, access->next);
	}
	case Ast_Access_Tag::Array:
	{
		Ast_Access_Array* array_access = access->as_array;

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
		return check_access(cc, result_type, access->next);
	}
	default: { err_internal("check_access: invalid Ast_Access_Tag"); return {}; }
	}
}

option<Ast_Type> check_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call, Checker_Proc_Call_Flags flags)
{
	resolve_proc_call(cc, proc_call);
	if (proc_call->tag == Ast_Resolve_Tag::Invalid) return {};

	// check input count
	Ast_Decl_Proc* proc_decl = proc_call->resolved.proc_decl;
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

option<Ast_Type> check_unary_expr(Check_Context* cc, Expr_Context context, Ast_Unary_Expr* unary_expr, Ast_Expr* source_expr)
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
		if (rhs_kind == Type_Kind::Float || rhs_kind == Type_Kind::Int || rhs_kind == Type_Kind::Uint) return rhs;
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
		if (rhs_kind == Type_Kind::Int || rhs_kind == Type_Kind::Uint) return rhs;
		err_set; printf("UNARY OP ~ only works on integer\n");
		return {};
	}
	case UnaryOp::ADDRESS_OF:
	{
		//@Todo prevent adress of globals or temporary values like array init and struct init
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

option<Ast_Type> check_binary_expr(Check_Context* cc, Expr_Context context, Ast_Binary_Expr* binary_expr, Ast_Expr* source_expr)
{
	option<Ast_Type> lhs_result = check_expr(cc, context, binary_expr->left);
	option<Ast_Type> rhs_result = check_expr(cc, context, binary_expr->right);
	if (!lhs_result || !rhs_result) return {};

	Ast_Type lhs = lhs_result.value();
	Ast_Type rhs = rhs_result.value();
	
	auto err_context_binary = [&]() {
		err_context(cc, binary_expr->left->span);
		printf("Type: "); print_type(lhs); printf("\n");
		err_context(cc, binary_expr->right->span);
		printf("Type: "); print_type(rhs); printf("\n");
	};

	option<BasicType> lhs_basic_type = type_extract_basic_and_enum_type(lhs);
	option<BasicType> rhs_basic_type = type_extract_basic_and_enum_type(rhs);
	
	if (!lhs_basic_type || !rhs_basic_type)
	{
		err_report(Error::BINARY_EXPR_NON_BASIC);
		err_context_binary();
		return {};
	}

	lhs = type_from_basic(lhs_basic_type.value());
	rhs = type_from_basic(rhs_basic_type.value());
	type_auto_binary_cast(&lhs, &rhs, binary_expr->left, binary_expr->right);
	
	if (lhs.as_basic != rhs.as_basic)
	{
		err_report(Error::BINARY_EXPR_NON_MATCHING_BASIC);
		err_context_binary();
		return {};
	}

	BinaryOp op = binary_expr->op;
	Type_Kind lhs_kind = type_kind(lhs);
	
	if (lhs_kind == Type_Kind::Int)
	{
		source_expr->flags |= AST_EXPR_FLAG_BIN_OP_INT_SIGNED;
	}
	
	switch (op)
	{
	case BinaryOp::LOGIC_AND:
	case BinaryOp::LOGIC_OR:
	{
		if (lhs_kind != Type_Kind::Bool) { err_report(Error::BINARY_LOGIC_ONLY_ON_BOOL); err_context_binary(); return {}; }
		return lhs;
	}
	case BinaryOp::LESS:
	case BinaryOp::GREATER:
	case BinaryOp::LESS_EQUALS:
	case BinaryOp::GREATER_EQUALS:
	{
		if (lhs_kind == Type_Kind::Bool) { err_report(Error::BINARY_CMP_ON_BOOL); err_context_binary(); return {}; }
		if (lhs_kind == Type_Kind::String) { err_report(Error::BINARY_CMP_ON_STRING); err_context_binary(); return {}; }
		return type_from_basic(BasicType::BOOL);
	}
	case BinaryOp::IS_EQUALS:
	case BinaryOp::NOT_EQUALS:
	{
		//@String basic type not implemented
		if (lhs_kind == Type_Kind::String) { err_internal("String comparisons are not implemented yet"); err_context_binary(); return {}; }

		Ast_Type lhs_raw_type = lhs_result.value();
		Ast_Type rhs_raw_type = rhs_result.value();
		if (type_kind(lhs_raw_type) == Type_Kind::Enum && type_kind(rhs_raw_type) == Type_Kind::Enum) 
		{
			if (lhs_raw_type.as_enum.enum_id != rhs_raw_type.as_enum.enum_id)
			{
				lhs = lhs_raw_type;
				rhs = rhs_raw_type;
				err_report(Error::BINARY_CMP_EQUAL_ON_ENUMS); err_context_binary(); return {};
			}
		}

		return type_from_basic(BasicType::BOOL);
	}
	case BinaryOp::PLUS:
	case BinaryOp::MINUS:
	case BinaryOp::TIMES:
	case BinaryOp::DIV:
	case BinaryOp::MOD:
	{
		if (lhs_kind != Type_Kind::Float && lhs_kind != Type_Kind::Int && lhs_kind == Type_Kind::Uint) 
		{ err_report(Error::BINARY_MATH_ONLY_ON_NUMERIC); err_context_binary(); return {}; }
		return lhs;
	}
	case BinaryOp::BITWISE_AND:
	case BinaryOp::BITWISE_OR:
	case BinaryOp::BITWISE_XOR:
	case BinaryOp::BITSHIFT_LEFT:
	case BinaryOp::BITSHIFT_RIGHT:
	{
		if (lhs_kind != Type_Kind::Uint) { err_report(Error::BINARY_BITWISE_ONLY_ON_UINT); err_context_binary(); return {}; }
		return lhs;
	}
	default: { err_internal("check_binary_expr: invalid BinaryOp"); return {}; }
	}
}

#include "general/tree.h"
#include "general/arena.h"

void check_consteval_expr(Check_Context* cc, Consteval_Dependency constant)
{
	if (constant.tag == Consteval_Dependency_Tag::Struct_Size)
	{
		Consteval size_eval = constant.as_struct_size.struct_decl->size_eval;
		if (size_eval == Consteval::Not_Evaluated)
		{
			Tree<Consteval_Dependency> tree(2048, constant);
			if (check_consteval_dependencies_struct_size(cc, &tree.arena, tree.root, constant.as_struct_size.struct_decl) == Consteval::Invalid) return;
			check_evaluate_consteval_tree(cc, tree.root);
		}
	}
	else
	{
		Ast_Consteval_Expr* consteval_expr = consteval_dependency_get_consteval_expr(constant);
		if (consteval_expr->eval == Consteval::Not_Evaluated)
		{
			Tree<Consteval_Dependency> tree(2048, constant);
			if (check_consteval_dependencies(cc, &tree.arena, tree.root, consteval_expr->expr) == Consteval::Invalid) return;
			check_evaluate_consteval_tree(cc, tree.root);
		}
	}
}

Consteval check_consteval_dependencies(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Ast_Expr* expr, option<Expr_Context> context)
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
			if (var->tag == Ast_Resolve_Var_Tag::Invalid) return consteval_dependency_mark_and_return_invalid(cc, parent);
			if (var->tag == Ast_Resolve_Var_Tag::Resolved_Local)
			{
				err_report(Error::CONST_VAR_IS_NOT_GLOBAL);
				err_context(cc, expr->span);
				return consteval_dependency_mark_and_return_invalid(cc, parent);
			}
			
			Ast_Decl_Global* global_decl = var->global.global_decl;
			Consteval eval = global_decl->consteval_expr->eval;
			if (eval == Consteval::Invalid) return Consteval::Invalid;
			if (eval == Consteval::Valid) return Consteval::Not_Evaluated;
			auto node = consteval_dependency_detect_cycle(cc, arena, parent, consteval_dependency_from_global(global_decl, expr->span));
			if (!node) return Consteval::Invalid;

			option<Ast_Access*> access = var->access;
			while (access)
			{
				switch (access.value()->tag)
				{
				case Ast_Access_Tag::Var: access = access.value()->next; break;
				case Ast_Access_Tag::Array:
				{
					if (!consteval_dependency_detect_cycle(cc, arena, parent, consteval_dependency_from_array_access(access.value()->as_array))) return Consteval::Invalid;
					access = access.value()->next;
				} break;
				default: { err_internal("check_consteval_dependencies: invalid Ast_Access_Tag"); break; }
				}
			}

			return check_consteval_dependencies(cc, arena, node.value(), global_decl->consteval_expr->expr);
		}
		case Ast_Term_Tag::Cast:
		{
			Ast_Cast* cast = term->as_cast;
			return check_consteval_dependencies(cc, arena, parent, cast->expr);
		}
		case Ast_Term_Tag::Enum:
		{
			Ast_Enum* _enum = term->as_enum;
			resolve_enum(cc, _enum);
			if (_enum->tag == Ast_Resolve_Tag::Invalid) return consteval_dependency_mark_and_return_invalid(cc, parent);

			Ast_Decl_Enum* enum_decl = _enum->resolved.type.enum_decl;
			Ast_Enum_Variant* enum_variant = &enum_decl->variants[_enum->resolved.variant_id];
			Consteval eval = enum_variant->consteval_expr->eval;
			if (eval == Consteval::Invalid) return Consteval::Invalid;
			if (eval == Consteval::Valid) return Consteval::Not_Evaluated;
			auto node = consteval_dependency_detect_cycle(cc, arena, parent, consteval_dependency_from_enum_variant(enum_variant, enum_decl->basic_type, expr->span));
			if (!node) return Consteval::Invalid;
			
			return check_consteval_dependencies(cc, arena, node.value(), enum_variant->consteval_expr->expr);
		}
		case Ast_Term_Tag::Sizeof:
		{
			Ast_Sizeof* size_of = term->as_sizeof;
			resolve_sizeof(cc, size_of, false);
			if (size_of->tag == Ast_Resolve_Tag::Invalid) return consteval_dependency_mark_and_return_invalid(cc, parent);

			option<Ast_Type_Struct> struct_type = type_extract_struct_value_type(size_of->type);
			if (struct_type) 
			{
				auto node = consteval_dependency_detect_cycle(cc, arena, parent, consteval_dependency_from_struct_size(struct_type.value().struct_decl, expr->span));
				if (!node) return Consteval::Invalid;
				if (check_consteval_dependencies_struct_size(cc, arena, node.value(), struct_type.value().struct_decl) == Consteval::Invalid) return Consteval::Invalid;
			}

			if (check_consteval_dependencies_array_type(cc, arena, parent, &size_of->type) == Consteval::Invalid) return Consteval::Invalid;
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
			return consteval_dependency_mark_and_return_invalid(cc, parent);
		}
		case Ast_Term_Tag::Array_Init:
		{
			Ast_Array_Init* array_init = term->as_array_init;
			if (context) resolve_array_init(cc, context.value(), array_init, false);
			else resolve_array_init(cc, Expr_Context{ {}, Expr_Constness::Const }, array_init, false);
			if (array_init->tag == Ast_Resolve_Tag::Invalid) return consteval_dependency_mark_and_return_invalid(cc, parent);

			option<Expr_Context> element_context = {};
			if (array_init->type)
			{
				element_context = Expr_Context { array_init->type.value().as_array->element_type, Expr_Constness::Const};
			}

			for (Ast_Expr* input_expr : array_init->input_exprs)
			{
				if (check_consteval_dependencies(cc, arena, parent, input_expr, element_context) == Consteval::Invalid) return Consteval::Invalid;
			}

			if (check_consteval_dependencies_array_type(cc, arena, parent, &array_init->type.value()) == Consteval::Invalid) return Consteval::Invalid;
			return Consteval::Not_Evaluated;
		}
		case Ast_Term_Tag::Struct_Init:
		{
			Ast_Struct_Init* struct_init = term->as_struct_init;
			if (context) resolve_struct_init(cc, context.value(), struct_init); //@Input expr count is not checked at resolve, extra input exprs will cause a crash
			else resolve_struct_init(cc, Expr_Context { {}, Expr_Constness::Const }, struct_init);
			if (struct_init->tag == Ast_Resolve_Tag::Invalid) return consteval_dependency_mark_and_return_invalid(cc, parent);
			
			u32 count = 0;
			for (Ast_Expr* input_expr : struct_init->input_exprs)
			{
				Ast_Decl_Struct* struct_decl = struct_init->resolved.type.value().struct_decl;
				Expr_Context field_context = Expr_Context { struct_decl->fields[count].type, Expr_Constness::Const };
				count += 1;
				if (check_consteval_dependencies(cc, arena, parent, input_expr, field_context) == Consteval::Invalid) return Consteval::Invalid;
			}

			return Consteval::Not_Evaluated;
		}
		default: { err_internal("check_const_expr_dependencies: invalid Ast_Term_Tag"); return Consteval::Invalid; }
		}
	}
	case Ast_Expr_Tag::Unary:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		if (check_consteval_dependencies(cc, arena, parent, unary_expr->right) == Consteval::Invalid) return Consteval::Invalid;
		return Consteval::Not_Evaluated;
	}
	case Ast_Expr_Tag::Binary:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		Consteval eval_left = check_consteval_dependencies(cc, arena, parent, binary_expr->left);
		Consteval eval_right = check_consteval_dependencies(cc, arena, parent, binary_expr->right);
		if (eval_left == Consteval::Invalid || eval_right == Consteval::Invalid) return Consteval::Invalid;
		return Consteval::Not_Evaluated;
	}
	case Ast_Expr_Tag::Folded:
	{
		return Consteval::Not_Evaluated;
	}
	default: { err_internal("check_const_expr_dependencies: invalid Ast_Expr_Tag"); return Consteval::Invalid; }
	}
}

Consteval check_consteval_dependencies_array_type(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Ast_Type* type)
{
	option<Ast_Type_Array*> array_type = type_extract_array_type(*type);
	while (array_type)
	{
		tree_node_add_child(arena, parent, consteval_dependency_from_array_size(array_type.value()->size_expr, type));
		if (check_consteval_dependencies(cc, arena, parent, array_type.value()->size_expr) == Consteval::Invalid) return Consteval::Invalid;
		array_type = type_extract_array_type(array_type.value()->element_type);
	}
	return Consteval::Not_Evaluated;
}

Consteval check_consteval_dependencies_struct_size(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Ast_Decl_Struct* struct_decl)
{
	if (struct_decl->size_eval == Consteval::Invalid) consteval_dependency_mark_and_return_invalid(cc, parent);

	for (Ast_Struct_Field& field : struct_decl->fields)
	{
		resolve_type(cc, &field.type, false);
		if (type_is_poison(field.type)) return consteval_dependency_mark_and_return_invalid(cc, parent);

		option<Ast_Type_Struct> struct_type = type_extract_struct_value_type(field.type);
		if (struct_type)
		{
			Ast_Decl_Struct* field_struct = struct_type.value().struct_decl;
			if (field_struct->size_eval == Consteval::Invalid) return consteval_dependency_mark_and_return_invalid(cc, parent);
			auto node = consteval_dependency_detect_cycle(cc, arena, parent, consteval_dependency_from_struct_size(field_struct, field_struct->ident.span));
			if (!node) return Consteval::Invalid;
			if (check_consteval_dependencies_struct_size(cc, arena, node.value(), field_struct) == Consteval::Invalid) return Consteval::Invalid;
		}

		if (check_consteval_dependencies_array_type(cc, arena, parent, &field.type) == Consteval::Invalid) return Consteval::Invalid;
	}

	return Consteval::Not_Evaluated;
}

option<Tree_Node<Consteval_Dependency>*> consteval_dependency_detect_cycle(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Consteval_Dependency constant)
{
	Tree_Node<Consteval_Dependency>* node = tree_node_add_child(arena, parent, constant);
	option<Tree_Node<Consteval_Dependency>*> cycle_node = tree_node_find_cycle(node, constant, match_const_dependency);
	if (cycle_node)
	{
		err_report(Error::CONSTEVAL_DEPENDENCY_CYCLE);
		tree_node_apply_proc_in_reverse_up_to_node(node, cycle_node.value(), cc, consteval_dependency_err_context);
		tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
		return {};
	}
	return node;
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
		Ast_Consteval_Expr* consteval_expr = constant.as_enum_variant.enum_variant->consteval_expr;
		option<Ast_Type> type = check_expr_type(cc, consteval_expr->expr, type_from_basic(constant.as_enum_variant.type), Expr_Constness::Const);
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
	case Consteval_Dependency_Tag::Array_Access_Expr:
	{
		//@Hack requirement i32 as in other place, temp
		option<Ast_Type> type = check_expr_type(cc, constant.as_array_access.access_expr, type_from_basic(BasicType::I32), Expr_Constness::Const);
		if (!type)
		{
			tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
			return Consteval::Invalid;
		}
	} break;
	default: 
	{ 
		err_internal("check_evaluate_consteval_tree: Invalid Consteval_Dependency_Tag");
		tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid); 
		return Consteval::Invalid; 
	}
	}

	return Consteval::Valid;
}

Consteval_Dependency consteval_dependency_from_global(Ast_Decl_Global* global_decl, Span span) { return { .tag = Consteval_Dependency_Tag::Global, .as_global = { global_decl, span } }; }
Consteval_Dependency consteval_dependency_from_enum_variant(Ast_Enum_Variant* enum_variant, BasicType type, Span span) { return { .tag = Consteval_Dependency_Tag::Enum_Variant, .as_enum_variant = { enum_variant, type, span } }; }
Consteval_Dependency consteval_dependency_from_struct_size(Ast_Decl_Struct* struct_decl, Span span) { return { .tag = Consteval_Dependency_Tag::Struct_Size, .as_struct_size = { struct_decl, span } }; }
Consteval_Dependency consteval_dependency_from_array_size(Ast_Expr* size_expr, Ast_Type* type) { return { .tag = Consteval_Dependency_Tag::Array_Size_Expr, .as_array_size = { size_expr, type } }; }
Consteval_Dependency consteval_dependency_from_array_access(Ast_Access_Array* array_access) { return { .tag = Consteval_Dependency_Tag::Array_Access_Expr, .as_array_access = { array_access->index_expr } }; }

Consteval consteval_dependency_mark_and_return_invalid(Check_Context* cc, Tree_Node<Consteval_Dependency>* node)
{
	tree_node_apply_proc_up_to_root(node, cc, consteval_dependency_mark_invalid);
	return Consteval::Invalid;
}

Ast_Consteval_Expr* consteval_dependency_get_consteval_expr(Consteval_Dependency constant)
{
	switch (constant.tag)
	{
	case Consteval_Dependency_Tag::Global: return constant.as_global.global_decl->consteval_expr;
	case Consteval_Dependency_Tag::Enum_Variant: return constant.as_enum_variant.enum_variant->consteval_expr;
	default: { err_internal("consteval_dependency_get_consteval_expr: invalid Const_Dependency_Tag"); return NULL; }
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
	case Consteval_Dependency_Tag::Array_Access_Expr: return false;
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
	case Consteval_Dependency_Tag::Array_Access_Expr: break;
	default: err_internal("consteval_dependency_mark_invalid: invalid Const_Dependency_Tag"); break;
	}
}

void consteval_dependency_err_context(Check_Context* cc, Tree_Node<Consteval_Dependency>* node)
{
	//@Add some in between messages and last one about cycle completion
	Consteval_Dependency constant = node->value;
	switch (constant.tag)
	{
	case Consteval_Dependency_Tag::Global: err_context(cc, constant.as_global.span); break;
	case Consteval_Dependency_Tag::Enum_Variant: err_context(cc, constant.as_enum_variant.span); break;
	case Consteval_Dependency_Tag::Struct_Size: err_context(cc, constant.as_struct_size.span); break;
	case Consteval_Dependency_Tag::Array_Size_Expr: err_context(cc, constant.as_array_size.size_expr->span); break;
	case Consteval_Dependency_Tag::Array_Access_Expr: err_context(cc, constant.as_array_access.access_expr->span); break;
	default: err_internal("consteval_dependency_err_context: invalid Const_Dependency_Tag"); break;
	}
}

option<Ast_Info_Struct> find_struct(Ast* target_ast, Ast_Ident ident) { return target_ast->struct_table.find(ident, hash_ident(ident)); }
option<Ast_Info_Enum> find_enum(Ast* target_ast, Ast_Ident ident)     { return target_ast->enum_table.find(ident, hash_ident(ident)); }
option<Ast_Info_Proc> find_proc(Ast* target_ast, Ast_Ident ident)     { return target_ast->proc_table.find(ident, hash_ident(ident)); }
option<Ast_Info_Global> find_global(Ast* target_ast, Ast_Ident ident) { return target_ast->global_table.find(ident, hash_ident(ident)); }

option<u32> find_enum_variant(Ast_Decl_Enum* enum_decl, Ast_Ident ident)
{
	for (u64 i = 0; i < enum_decl->variants.size(); i += 1)
	{
		if (match_ident(enum_decl->variants[i].ident, ident)) return (u32)i;
	}
	return {};
}

option<u32> find_struct_field(Ast_Decl_Struct* struct_decl, Ast_Ident ident)
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
	option<Ast_Decl_Import*> import_decl = cc->ast->import_table.find(import_ident, hash_ident(import_ident));
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
	case Ast_Type_Tag::Enum: break;
	case Ast_Type_Tag::Struct: break;
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
	case Ast_Type_Tag::Unresolved:
	{
		Ast* target_ast = resolve_import(cc, type->as_unresolved->import);
		if (target_ast == NULL) 
		{
			type->tag = Ast_Type_Tag::Poison;
			return;
		}

		option<Ast_Info_Struct> struct_info = find_struct(target_ast, type->as_unresolved->ident);
		if (struct_info)
		{
			type->tag = Ast_Type_Tag::Struct;
			type->as_struct.struct_id = struct_info.value().struct_id;
			type->as_struct.struct_decl = struct_info.value().struct_decl;
			return;
		}
		
		option<Ast_Info_Enum> enum_info = find_enum(target_ast, type->as_unresolved->ident);
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
	default: { err_internal("resolve_type: invalid Ast_Type_Tag"); break; }
	}
}

void resolve_var(Check_Context* cc, Ast_Var* var)
{
	if (var->tag != Ast_Resolve_Var_Tag::Unresolved) return;

	Ast_Ident ident = var->unresolved.ident;
	Ast* target_ast = cc->ast;

	option<Ast_Decl_Import*> import_decl = cc->ast->import_table.find(ident, hash_ident(ident));
	if (import_decl && var->access && var->access.value()->tag == Ast_Access_Tag::Var)
	{
		Ast_Access_Var* var_access = var->access.value()->as_var;
		Ast_Ident global_ident = var_access->unresolved.ident;
		var->access = var->access.value()->next;

		target_ast = import_decl.value()->import_ast;
		if (target_ast == NULL)
		{
			var->tag = Ast_Resolve_Var_Tag::Invalid;
			return;
		}

		option<Ast_Info_Global> global = find_global(target_ast, global_ident);
		if (global) 
		{
			var->tag = Ast_Resolve_Var_Tag::Resolved_Global;
			var->global.global_id = global.value().global_id;
			var->global.global_decl = global.value().global_decl;
			return;
		}
		else
		{
			err_report(Error::RESOLVE_VAR_GLOBAL_NOT_FOUND);
			err_context(cc, global_ident.span);
			var->tag = Ast_Resolve_Var_Tag::Invalid;
			return;
		}
	}
	else
	{
		option<Ast_Info_Global> global = find_global(target_ast, ident);
		if (global)
		{
			var->tag = Ast_Resolve_Var_Tag::Resolved_Global;
			var->global.global_id = global.value().global_id;
			var->global.global_decl = global.value().global_decl;
			return;
		}
	}

	var->tag = Ast_Resolve_Var_Tag::Resolved_Local;
	var->local.ident = var->unresolved.ident;
}

void resolve_enum(Check_Context* cc, Ast_Enum* _enum)
{
	if (_enum->tag != Ast_Resolve_Tag::Unresolved) return;

	Ast* target_ast = resolve_import(cc, _enum->unresolved.import);
	if (target_ast == NULL) 
	{
		_enum->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	option<Ast_Info_Enum> enum_info = find_enum(target_ast, _enum->unresolved.ident);
	if (!enum_info)
	{
		err_report(Error::RESOLVE_ENUM_NOT_FOUND);
		err_context(cc, _enum->unresolved.ident.span);
		_enum->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	Ast_Decl_Enum* enum_decl = enum_info.value().enum_decl;
	option<u32> variant_id = find_enum_variant(enum_decl, _enum->unresolved.variant);
	if (!variant_id)
	{
		err_report(Error::RESOLVE_ENUM_VARIANT_NOT_FOUND);
		err_context(cc, _enum->unresolved.variant.span);
		_enum->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	_enum->tag = Ast_Resolve_Tag::Resolved;
	_enum->resolved.type = Ast_Type_Enum { enum_info.value().enum_id, enum_info.value().enum_decl };
	_enum->resolved.variant_id = variant_id.value();
}

void resolve_cast(Check_Context* cc, Expr_Context context, Ast_Cast* cast)
{
	option<Ast_Type> type = check_expr_type(cc, cast->expr, {}, context.constness);
	if (!type) 
	{ 
		cast->tag = Ast_Resolve_Cast_Tag::Invalid;
		return; 
	}

	if (type.value().tag != Ast_Type_Tag::Basic)
	{
		err_report(Error::CAST_EXPR_NON_BASIC_TYPE);
		printf("Type: "); print_type(type.value());
		err_context(cc, cast->expr->span);
		cast->tag = Ast_Resolve_Cast_Tag::Invalid;
		return;
	}

	BasicType expr_type = type.value().as_basic;
	if (expr_type == BasicType::BOOL)   { err_report(Error::CAST_EXPR_BOOL_BASIC_TYPE);   err_context(cc, cast->expr->span); cast->tag = Ast_Resolve_Cast_Tag::Invalid; return; }
	if (expr_type == BasicType::STRING) { err_report(Error::CAST_EXPR_STRING_BASIC_TYPE); err_context(cc, cast->expr->span); cast->tag = Ast_Resolve_Cast_Tag::Invalid; return; }
	
	BasicType cast_type = cast->basic_type;
	if (cast_type == BasicType::BOOL)   { err_report(Error::CAST_INTO_BOOL_BASIC_TYPE);   err_context(cc, cast->expr->span); cast->tag = Ast_Resolve_Cast_Tag::Invalid; return; }
	if (cast_type == BasicType::STRING) { err_report(Error::CAST_INTO_STRING_BASIC_TYPE); err_context(cc, cast->expr->span); cast->tag = Ast_Resolve_Cast_Tag::Invalid; return; }

	Type_Kind expr_kind = type_kind(type_from_basic(expr_type));
	Type_Kind cast_kind = type_kind(type_from_basic(cast_type));

	if (expr_type == cast_type)
	{
		switch (expr_kind)
		{
		case Type_Kind::Float:
		{
			err_report(Error::CAST_REDUNDANT_FLOAT_CAST);
			err_context(cc, cast->expr->span);
			cast->tag = Ast_Resolve_Cast_Tag::Invalid;
			return;
		}
		case Type_Kind::Int:
		case Type_Kind::Uint:
		{
			err_report(Error::CAST_REDUNDANT_INTEGER_CAST); 
			err_context(cc, cast->expr->span); 
			cast->tag = Ast_Resolve_Cast_Tag::Invalid;
			return;
		}
		default:
		{
			err_internal("resolve_cast: invalid expr_kind Type_Kind"); 
			err_context(cc, cast->expr->span); 
			cast->tag = Ast_Resolve_Cast_Tag::Invalid;
			return;
		}
		}
	}

	u32 expr_type_size = type_basic_size(expr_type);
	u32 cast_type_size = type_basic_size(cast_type);

	switch (expr_kind)
	{
	case Type_Kind::Float:
	{
		switch (cast_kind)
		{
		case Type_Kind::Float:
		{
			if (cast_type_size > expr_type_size) cast->tag = Ast_Resolve_Cast_Tag::Float_Extend_LLVMFPExt;
			else cast->tag = Ast_Resolve_Cast_Tag::Float_Trunc__LLVMFPTrunc;
		} break;
		case Type_Kind::Int:  cast->tag = Ast_Resolve_Cast_Tag::Float_Int____LLVMFPToSI; break;
		case Type_Kind::Uint: cast->tag = Ast_Resolve_Cast_Tag::Float_Uint___LLVMFPToUI; break;
		}
	} break;
	case Type_Kind::Int:
	{
		switch (cast_kind)
		{
		case Type_Kind::Float: cast->tag = Ast_Resolve_Cast_Tag::Int_Float____LLVMSIToFP; break;
		case Type_Kind::Int:
		{
			if (cast_type_size > expr_type_size) cast->tag = Ast_Resolve_Cast_Tag::Int_Extend___LLVMSExt;
			else cast->tag = Ast_Resolve_Cast_Tag::Int_Trunc____LLVMTrunc;
		} break;
		case Type_Kind::Uint:
		{
			if (cast_type_size == expr_type_size) cast->tag = Ast_Resolve_Cast_Tag::Integer_No_Op;
			else if (cast_type_size > expr_type_size) cast->tag = Ast_Resolve_Cast_Tag::Int_Extend___LLVMSExt;
			else cast->tag = Ast_Resolve_Cast_Tag::Int_Trunc____LLVMTrunc;
		} break;
		}
	} break;
	case Type_Kind::Uint:
	{
		switch (cast_kind)
		{
		case Type_Kind::Float: cast->tag = Ast_Resolve_Cast_Tag::Uint_Float___LLVMUIToFP; break;
		case Type_Kind::Int:
		{
			if (cast_type_size == expr_type_size) cast->tag = Ast_Resolve_Cast_Tag::Integer_No_Op;
			else if (cast_type_size > expr_type_size) cast->tag = Ast_Resolve_Cast_Tag::Uint_Extend__LLVMZExt;
			else cast->tag = Ast_Resolve_Cast_Tag::Int_Trunc____LLVMTrunc;
		} break;
		case Type_Kind::Uint:
		{
			if (cast_type_size > expr_type_size) cast->tag = Ast_Resolve_Cast_Tag::Uint_Extend__LLVMZExt;
			else cast->tag = Ast_Resolve_Cast_Tag::Int_Trunc____LLVMTrunc;
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
		size_of->tag = Ast_Resolve_Tag::Invalid;
		return;
	}
	else size_of->tag = Ast_Resolve_Tag::Resolved;
}

void resolve_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call)
{
	if (proc_call->tag != Ast_Resolve_Tag::Unresolved) return;

	Ast* target_ast = resolve_import(cc, proc_call->unresolved.import);
	if (target_ast == NULL)
	{
		proc_call->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	option<Ast_Info_Proc> proc_info = find_proc(target_ast, proc_call->unresolved.ident);
	if (!proc_info)
	{
		err_report(Error::RESOLVE_PROC_NOT_FOUND);
		err_context(cc, proc_call->unresolved.ident.span);
		proc_call->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	Ast_Decl_Proc* proc_decl = proc_info.value().proc_decl;
	if (proc_decl->return_type)
	{
		if (type_is_poison(proc_decl->return_type.value()))
		{
			proc_call->tag = Ast_Resolve_Tag::Invalid;
			return;
		}
	}

	proc_call->tag = Ast_Resolve_Tag::Resolved;
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
			array_init->tag = Ast_Resolve_Tag::Invalid;
			return;
		}
	}

	if (context.expect_type)
	{
		Ast_Type expect_type = context.expect_type.value();
		if (type_kind(expect_type) != Type_Kind::Array)
		{
			//@err_context
			err_report(Error::RESOLVE_ARRAY_WRONG_CONTEXT);
			print_type(expect_type); printf("\n");
			array_init->tag = Ast_Resolve_Tag::Invalid;
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
				array_init->tag = Ast_Resolve_Tag::Invalid;
				return;
			}
		}
		else array_init->type = expect_type;
	}

	if (!array_init->type)
	{
		//@err_context
		err_report(Error::RESOLVE_ARRAY_NO_CONTEXT);
		array_init->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	array_init->tag = Ast_Resolve_Tag::Resolved;
}

void resolve_struct_init(Check_Context* cc, Expr_Context context, Ast_Struct_Init* struct_init)
{
	if (struct_init->tag != Ast_Resolve_Tag::Unresolved) return;

	Ast* target_ast = resolve_import(cc, struct_init->unresolved.import);
	if (target_ast == NULL)
	{
		struct_init->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	if (struct_init->unresolved.ident)
	{
		option<Ast_Info_Struct> struct_info = find_struct(target_ast, struct_init->unresolved.ident.value());
		if (!struct_info)
		{
			err_report(Error::RESOLVE_STRUCT_NOT_FOUND);
			err_context(cc, struct_init->unresolved.ident.value().span);
			struct_init->tag = Ast_Resolve_Tag::Invalid;
			return;
		}

		struct_init->tag = Ast_Resolve_Tag::Resolved;
		struct_init->resolved.type = Ast_Type_Struct { struct_info.value().struct_id, struct_info.value().struct_decl };
	}
	else
	{
		struct_init->tag = Ast_Resolve_Tag::Resolved;
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
			struct_init->tag = Ast_Resolve_Tag::Invalid;
			return;
		}

		Ast_Type_Struct expected_struct = expect_type.as_struct;
		if (struct_init->resolved.type)
		{
			Ast_Type_Struct struct_type = struct_init->resolved.type.value();
			if (struct_type.struct_id != expected_struct.struct_id)
			{
				//@err_context
				err_report(Error::RESOLVE_STRUCT_TYPE_MISMATCH);
				struct_init->tag = Ast_Resolve_Tag::Invalid;
				return;
			}
		}
		else struct_init->resolved.type = expected_struct;
	}

	if (!struct_init->resolved.type)
	{
		//@err_context
		err_report(Error::RESOLVE_STRUCT_NO_CONTEXT);
		struct_init->tag = Ast_Resolve_Tag::Invalid;
		return;
	}

	struct_init->tag = Ast_Resolve_Tag::Resolved;
}
