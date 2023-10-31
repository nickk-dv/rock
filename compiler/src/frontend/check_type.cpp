#include "check_type.h"

#include "error_handler.h"
#include "check_general.h"

Type_Kind type_kind(Check_Context* cc, Ast_Type type)
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
	default:
	{
		//@Err internal
		//err_set;
		//printf("[COMPILER ERROR] Ast_Type signature wasnt checked, tag cannot be Tag::Custom\n");
		//printf("Hint: submit a bug report if you see this error message\n");
		//debug_print_type(type);
		//printf("\n");
		return Type_Kind::Integer;
	}
	}
}

Ast_Type type_from_basic(BasicType basic_type)
{
	Ast_Type type = {};
	type.tag = Ast_Type_Tag::Basic;
	type.as_basic = basic_type;
	return type;
}

option<Ast_Type> check_type_signature(Check_Context* cc, Ast_Type* type)
{
	switch (type->tag)
	{
	case Ast_Type_Tag::Basic:
	{
		return *type;
	}
	case Ast_Type_Tag::Array:
	{
		//@Todo also must be > 0
		//@Temp using i32 instead of u32 for static arrays to avoid type mismatch
		option<Ast_Type> expr_type = check_expr_type(cc, type->as_array->const_expr.expr, type_from_basic(BasicType::I32), true);
		if (!expr_type) return {};
		
		option<Ast_Type> element_type = check_type_signature(cc, &type->as_array->element_type);
		if (!element_type) return {};
		
		return *type;
	}
	case Ast_Type_Tag::Custom:
	{
		Ast* target_ast = find_import(cc, type->as_custom->import);
		if (target_ast == NULL) return {};

		option<Ast_Struct_Info> struct_meta = find_struct(target_ast, type->as_custom->ident);
		if (struct_meta)
		{
			type->tag = Ast_Type_Tag::Struct;
			type->as_struct.struct_id = struct_meta.value().struct_id;
			type->as_struct.struct_decl = struct_meta.value().struct_decl;
			return *type;
		}

		option<Ast_Enum_Info> enum_meta = find_enum(target_ast, type->as_custom->ident);
		if (enum_meta)
		{
			type->tag = Ast_Type_Tag::Enum;
			type->as_enum.enum_id = enum_meta.value().enum_id;
			type->as_enum.enum_decl = enum_meta.value().enum_decl;
			return *type;
		}

		err_report(Error::TYPE_CUSTOM_NOT_FOUND);
		err_context(cc, type->span);
		return {};
	}
	default:
	{
		//@Err internal
		//err_set;
		//printf("[COMPILER ERROR] Ast_Type signature cannot be checked multiple times\n");
		//printf("Hint: submit a bug report if you see this error message\n");
		//debug_print_type(*type);
		//printf("\n");
		return {};
	}
	}
}

option<Ast_Type> check_expr_type(Check_Context* cc, Ast_Expr* expr, option<Ast_Type> expect_type, bool expect_constant)
{
	Type_Context context = { expect_type, expect_constant };
	option<Ast_Type> type = check_expr(cc, &context, expr);

	if (!type) return {};
	if (!expect_type) return type;

	type_implicit_cast(cc, &type.value(), expect_type.value());

	if (!match_type(cc, type.value(), expect_type.value()))
	{
		err_report(Error::TYPE_MISMATCH);
		err_context(cc, expect_type.value().span); //@Err add ability to add messages
		err_context(cc, type.value().span); //@Span isnt available for generated types, use custom printing for them
		err_context(cc, expr->span);
		return {};
	}

	return type;
}

bool match_type(Check_Context* cc, Ast_Type type_a, Ast_Type type_b)
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
		//@Array match constexpr sizes
		return match_type(cc, array_a->element_type, array_b->element_type);
	}
	default:
	{
		//@Err internal
		//err_set; printf("match_type: Unexpected Ast_Type_Tag. Disambiguate Tag::Custom by using check_type first:\n");
		//debug_print_type(type_a); printf("\n");
		//debug_print_type(type_b); printf("\n");
		return false;
	}
	}
}

bool check_is_const_expr(Ast_Expr* expr)
{
	if (check_is_const_foldable_expr(expr)) return true;

	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term:
	{
		Ast_Term* term = expr->as_term;
		switch (term->tag)
		{
		case Ast_Term_Tag::Array_Init:
		{
			Ast_Array_Init* array_init = term->as_array_init;
			for (Ast_Expr* expr : array_init->input_exprs)
				if (!check_is_const_expr(expr)) return false;
			return true;
		}
		case Ast_Term_Tag::Struct_Init:
		{
			Ast_Struct_Init* struct_init = term->as_struct_init;
			for (Ast_Expr* expr : struct_init->input_exprs)
				if (!check_is_const_expr(expr)) return false;
			return true;
		}
		default: return false;
		}
	}
	case Ast_Expr_Tag::Unary_Expr: return check_is_const_expr(expr->as_unary_expr->right);
	case Ast_Expr_Tag::Binary_Expr: return check_is_const_expr(expr->as_binary_expr->left) && check_is_const_expr(expr->as_binary_expr->right);
	}
}

bool check_is_const_foldable_expr(Ast_Expr* expr)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term:
	{
		Ast_Term* term = expr->as_term;
		switch (term->tag)
		{
		//@Notice not handling enum as constexpr yet 
		//case Ast_Term_Tag::Enum: return true;
		case Ast_Term_Tag::Literal: return term->as_literal.token.type != TokenType::STRING_LITERAL;
		default: return false;
		}
	}
	case Ast_Expr_Tag::Unary_Expr: return check_is_const_foldable_expr(expr->as_unary_expr->right);
	case Ast_Expr_Tag::Binary_Expr: return check_is_const_foldable_expr(expr->as_binary_expr->left) && check_is_const_foldable_expr(expr->as_binary_expr->right);
	}
}

void type_implicit_cast(Check_Context* cc, Ast_Type* type, Ast_Type target_type)
{
	if (type->tag != Ast_Type_Tag::Basic) return;
	if (target_type.tag != Ast_Type_Tag::Basic) return;
	if (type->as_basic == target_type.as_basic) return;
	Type_Kind kind = type_kind(cc, *type);
	Type_Kind target_kind = type_kind(cc, target_type);

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

void type_implicit_binary_cast(Check_Context* cc, Ast_Type* type_a, Ast_Type* type_b)
{
	if (type_a->tag != Ast_Type_Tag::Basic) return;
	if (type_b->tag != Ast_Type_Tag::Basic) return;
	if (type_a->as_basic == type_b->as_basic) return;
	Type_Kind kind_a = type_kind(cc, *type_a);
	Type_Kind kind_b = type_kind(cc, *type_b);

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

option<Ast_Type> check_expr(Check_Context* cc, Type_Context* context, Ast_Expr* expr)
{
	if (context->expect_constant)
	{
		std::vector<Ast_Const_Expr*> dependencies;
		Const_Eval eval = check_const_expr_eval(cc, expr, dependencies);
		if (eval == Const_Eval::Invalid) return {};

		printf("Const_Expr: \n");
		err_context(cc, expr->span);
		printf("Dependencies: \n");
		for (Ast_Const_Expr* const_expr : dependencies)
		err_context(cc, const_expr->expr->span);
	}
	
	if (check_is_const_expr(expr))
	{
		expr->is_const = true;
	}

	if (!check_is_const_foldable_expr(expr))
	{
		if (expr->is_const == false && context->expect_constant)
		{
			err_report(Error::EXPR_EXPECTED_CONSTANT);
			err_context(cc, expr->span);
			return {};
		}

		switch (expr->tag)
		{
		case Ast_Expr_Tag::Term: return check_term(cc, context, expr->as_term);
		case Ast_Expr_Tag::Unary_Expr: return check_unary_expr(cc, context, expr->as_unary_expr);
		case Ast_Expr_Tag::Binary_Expr: return check_binary_expr(cc, context, expr->as_binary_expr);
		}
	}
	else
	{
		option<Literal> lit_result = check_const_expr(expr);
		if (!lit_result) return {};

		Ast_Folded_Expr folded_expr = {};
		Literal lit = lit_result.value();

		if (context)
		{
			switch (lit.kind)
			{
			case Literal_Kind::Bool:
			{
				folded_expr.basic_type = BasicType::BOOL;
				folded_expr.as_bool = lit.as_bool;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::BOOL);
			}
			case Literal_Kind::Float:
			{
				//@Base on context
				folded_expr.basic_type = BasicType::F64;
				folded_expr.as_f64 = lit.as_f64;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::F64);
			}
			case Literal_Kind::Int:
			{
				//@Base on context
				//@Todo range based
				folded_expr.basic_type = BasicType::I32;
				folded_expr.as_i64 = lit.as_i64;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::I32);
			}
			case Literal_Kind::UInt:
			{
				//@Base on context
				//@Todo range based
				folded_expr.basic_type = BasicType::I32;
				folded_expr.as_u64 = lit.as_u64;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::I32);
			}
			}
		}
		else
		{
			switch (lit.kind)
			{
			case Literal_Kind::Bool:
			{
				folded_expr.basic_type = BasicType::BOOL;
				folded_expr.as_bool = lit.as_bool;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::BOOL);
			}
			case Literal_Kind::Float:
			{
				folded_expr.basic_type = BasicType::F64;
				folded_expr.as_f64 = lit.as_f64;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::F64);
			}
			case Literal_Kind::Int:
			{
				//@Todo range based default int type
				folded_expr.basic_type = BasicType::I32;
				folded_expr.as_i64 = lit.as_i64;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::I32);
			}
			case Literal_Kind::UInt:
			{
				//@Todo range based default int type
				//@Might become a u64 if its too big
				folded_expr.basic_type = BasicType::I32;
				folded_expr.as_u64 = lit.as_u64;
				expr->tag = Ast_Expr_Tag::Folded_Expr;
				expr->as_folded_expr = folded_expr;
				return type_from_basic(BasicType::I32);
			}
			}
		}
	}
}

//@TODO temp allowing old errors:
#define err_set (void)0;
#include "debug_printer.h"
static void error_pair(const char* message, const char* labelA, Ast_Ident identA, const char* labelB, Ast_Ident identB)
{
	printf("%s:\n", message);
	printf("%s: ", labelA);
	debug_print_ident(identA, true, true);
	printf("%s: ", labelB);
	debug_print_ident(identB, true, true);
	printf("\n");
}
static void error(const char* message, Ast_Ident ident)
{
	printf("%s:\n", message);
	debug_print_ident(ident, true, true);
	printf("\n");
}

option<Ast_Type> check_term(Check_Context* cc, Type_Context* context, Ast_Term* term)
{
	switch (term->tag)
	{
	case Ast_Term_Tag::Var: return check_var(cc, term->as_var);
	case Ast_Term_Tag::Enum: //@Const fold this should be part of constant evaluation
	{
		Ast_Enum* _enum = term->as_enum;
		check_enum_resolve(cc, _enum);
		if (_enum->tag == Ast_Enum_Tag::Invalid) return {};

		Ast_Type type = {};
		type.tag = Ast_Type_Tag::Enum;
		type.as_enum = _enum->resolved.type;
		return type;
	}
	case Ast_Term_Tag::Sizeof: //@Const fold this should be part of constant evaluation
	{
		//@Notice not doing sizing of types yet, cant know the numeric range
		option<Ast_Type> type = check_type_signature(cc, &term->as_sizeof->type);
		if (type) return type_from_basic(BasicType::U64);
		return {};
	}
	case Ast_Term_Tag::Literal:
	{
		Ast_Literal literal = term->as_literal;
		switch (literal.token.type)
		{
		case TokenType::STRING_LITERAL: //@ Strings are just *i8 cstrings for now
		{
			Ast_Type string_ptr = type_from_basic(BasicType::I8);
			string_ptr.pointer_level += 1;
			return string_ptr;
		}
		default:
		{
			//@Err internal
			//err_set;
			//printf("Check_term: Unsupported literal term, only string literals are allowed to be proccesed:\n");
			//debug_print_token(literal.token, true, true);
			//printf("\n");
			return {};
		}
		}
	}
	case Ast_Term_Tag::Proc_Call:
	{
		return check_proc_call(cc, term->as_proc_call, Checker_Proc_Call_Flags::In_Expr);
	}
	case Ast_Term_Tag::Struct_Init:
	{
		Ast_Struct_Init* struct_init = term->as_struct_init;
		check_struct_init_resolve(cc, struct_init);
		if (struct_init->tag == Ast_Struct_Init_Tag::Invalid) return {};

		if (context->expect_type)
		{
			Ast_Type expect_type = context->expect_type.value();
			if (type_kind(cc, expect_type) != Type_Kind::Struct)
			{
				err_set; 
				printf("Cannot use struct initializer in non struct type context\n");
				printf("Context: "); debug_print_type(expect_type); printf("\n");
				return {};
			}

			Ast_Struct_Type expected_struct = expect_type.as_struct;
			if (struct_init->resolved.type)
			{
				Ast_Struct_Type struct_type = struct_init->resolved.type.value();
				if (struct_type.struct_id != expected_struct.struct_id)
				{
					err_set;
					printf("Struct initializer struct type doesnt match the expected type:\n");
					debug_print_struct_init(struct_init, 0); printf("\n");
					return {};
				}
			}
			else struct_init->resolved.type = expected_struct;
		}

		if (!struct_init->resolved.type)
		{
			err_set;
			printf("Cannot infer the struct initializer type without a context\n");
			printf("Hint: specify type on varible: var : Type = .{ ... }, or on initializer var := Type.{ ... }\n");
			debug_print_struct_init(struct_init, 0); printf("\n");
			return {};
		}

		// check input count
		Ast_Struct_Decl* struct_decl = struct_init->resolved.type.value().struct_decl;
		u32 field_count = (u32)struct_decl->fields.size();
		u32 input_count = (u32)struct_init->input_exprs.size();
		if (field_count != input_count)
		{
			err_set;
			printf("Unexpected number of fields in struct initializer:\n");
			printf("Expected: %lu Got: %lu \n", field_count, input_count);
			debug_print_struct_init(struct_init, 0); printf("\n");
		}

		// check input exprs
		for (u32 i = 0; i < input_count; i += 1)
		{
			if (i < field_count)
			{
				Ast_Type field_type = struct_decl->fields[i].type;
				check_expr_type(cc, struct_init->input_exprs[i], field_type, context->expect_constant);
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

		option<Ast_Type> type = {};
		if (array_init->type)
		{
			type = check_type_signature(cc, &array_init->type.value());
			if (!type) return {};
			if (type_kind(cc, type.value()) != Type_Kind::Array)
			{
				err_set; printf("Array initializer must have array type signature\n");
				return {};
			}
		}

		if (context->expect_type)
		{
			Ast_Type expect_type = context->expect_type.value();
			if (type_kind(cc, expect_type) != Type_Kind::Array)
			{
				err_set; printf("Cannot use array initializer in non array type context\n");
				printf("Context: "); debug_print_type(expect_type); printf("\n");
				return {};
			}

			if (!type)
			{
				type = expect_type;
			}
			else
			{
				if (!match_type(cc, type.value(), expect_type))
				{
					err_set;
					printf("Array initializer type doesnt match the expected type:\n");
					return {};
				}
			}
		}

		if (!type)
		{
			err_set;
			printf("Cannot infer the array initializer type without a context\n");
			printf("Hint: specify type on varible: var : [2]i32 = { 2, 3 } or var := [2]i32{ 2, 3 }\n");
			return {};
		}

		//@Check input count compared to array size
		// check input count
		u32 input_count = (u32)array_init->input_exprs.size();
		u32 expected_count = input_count; //@move 1 line up

		// check input exprs
		for (u32 i = 0; i < input_count; i += 1)
		{
			if (i < expected_count)
			{
				Ast_Type element_type = type.value().as_array->element_type;
				check_expr_type(cc, array_init->input_exprs[i], element_type, context->expect_constant);
			}
		}

		array_init->type = type;
		return type;
	}
	}
}

option<Ast_Type> check_var(Check_Context* cc, Ast_Var* var)
{
	check_var_resolve(cc, var);
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

		Type_Kind kind = type_kind(cc, type);
		if (kind == Type_Kind::Pointer && type.pointer_level == 1 && type.tag == Ast_Type_Tag::Struct) kind = Type_Kind::Struct;
		if (kind != Type_Kind::Struct)
		{
			err_set;
			error("Field access might only be used on variables of struct or pointer to a struct type", var_access->ident);
			return {};
		}

		Ast_Struct_Decl* struct_decl = type.as_struct.struct_decl;
		option<u32> field_id = find_struct_field(struct_decl, var_access->ident);
		if (!field_id)
		{
			err_set;
			error("Failed to find struct field during access", var_access->ident);
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
		Type_Kind kind = type_kind(cc, type);
		if (kind == Type_Kind::Pointer && type.pointer_level == 1 && type.tag == Ast_Type_Tag::Array) kind = Type_Kind::Array;
		if (kind != Type_Kind::Array)
		{
			err_set;
			printf("Array access might only be used on variables of array type:\n");
			debug_print_access(access);
			printf("\n\n");
			return {};
		}

		//@Notice allowing u64 for dynamic array and slices
		//@Temp using i32 instead of u64 to avoid type mismatch
		check_expr_type(cc, array_access->index_expr, type_from_basic(BasicType::I32), false);

		Ast_Type result_type = type.as_array->element_type;
		return check_access(cc, result_type, array_access->next);
	}
	}
}

option<Ast_Type> check_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call, Checker_Proc_Call_Flags flags)
{
	check_proc_call_resolve(cc, proc_call);
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
			err_set;
			printf("Unexpected number of arguments in variadic procedure call:\n");
			printf("Expected at least: %lu Got: %lu \n", param_count, input_count);
			debug_print_ident(proc_call->resolved.proc_decl->ident, true, true); printf("\n");
		}
	}
	else
	{
		if (param_count != input_count)
		{
			err_set;
			printf("Unexpected number of arguments in procedure call:\n");
			printf("Expected: %lu Got: %lu \n", param_count, input_count);
			debug_print_ident(proc_call->resolved.proc_decl->ident, true, true); printf("\n");
		}
	}

	// check input exprs
	for (u32 i = 0; i < input_count; i += 1)
	{
		if (i < param_count)
		{
			Ast_Type param_type = proc_decl->input_params[i].type;
			check_expr_type(cc, proc_call->input_exprs[i], param_type, false);
		}
		else if (is_variadic)
		{
			check_expr_type(cc, proc_call->input_exprs[i], {}, false);
		}
	}

	// check access & return
	if (flags == Checker_Proc_Call_Flags::In_Expr)
	{
		if (!proc_decl->return_type)
		{
			err_set;
			printf("Procedure call inside expression must have a return type:\n");
			debug_print_proc_call(proc_call, 0);
			printf("\n");
			return {};
		}

		return check_access(cc, proc_decl->return_type.value(), proc_call->access);
	}
	else
	{
		if (proc_call->access)
		{
			err_set;
			printf("Procedure call statement cannot have access chains:\n");
			debug_print_proc_call(proc_call, 0);
			printf("\n");
		}

		if (proc_decl->return_type)
		{
			err_set;
			printf("Procedure call result cannot be discarded:\n");
			debug_print_proc_call(proc_call, 0);
			printf("\n");
		}

		return {};
	}
}

option<Ast_Type> check_unary_expr(Check_Context* cc, Type_Context* context, Ast_Unary_Expr* unary_expr)
{
	option<Ast_Type> rhs_result = check_expr(cc, context, unary_expr->right);
	if (!rhs_result) return {};

	UnaryOp op = unary_expr->op;
	Ast_Type rhs = rhs_result.value();
	Type_Kind rhs_kind = type_kind(cc, rhs);

	switch (op)
	{
	case UnaryOp::MINUS:
	{
		if (rhs_kind == Type_Kind::Float || rhs_kind == Type_Kind::Integer) return rhs;
		err_set; printf("UNARY OP - only works on float or integer\n");
		debug_print_unary_expr(unary_expr, 0); return {};
	}
	case UnaryOp::LOGIC_NOT:
	{
		if (rhs_kind == Type_Kind::Bool) return rhs;
		err_set; printf("UNARY OP ! only works on bool\n");
		debug_print_unary_expr(unary_expr, 0); return {};
	}
	case UnaryOp::BITWISE_NOT:
	{
		if (rhs_kind == Type_Kind::Integer) return rhs;
		err_set; printf("UNARY OP ~ only works on integer\n");
		debug_print_unary_expr(unary_expr, 0); return {};
	}
	case UnaryOp::ADDRESS_OF:
	{
		//@Todo prevent adress of temporary values
		rhs.pointer_level += 1;
		return rhs;
	}
	case UnaryOp::DEREFERENCE:
	{
		err_set; printf("UNARY OP << unsupported\n"); debug_print_unary_expr(unary_expr, 0); return {};
	}
	}
}

option<Ast_Type> check_binary_expr(Check_Context* cc, Type_Context* context, Ast_Binary_Expr* binary_expr)
{
	option<Ast_Type> lhs_result = check_expr(cc, context, binary_expr->left);
	if (!lhs_result) return {};
	option<Ast_Type> rhs_result = check_expr(cc, context, binary_expr->right);
	if (!rhs_result) return {};

	BinaryOp op = binary_expr->op;
	Ast_Type lhs = lhs_result.value();
	Type_Kind lhs_kind = type_kind(cc, lhs);
	Ast_Type rhs = rhs_result.value();
	Type_Kind rhs_kind = type_kind(cc, rhs);
	bool same_kind = lhs_kind == rhs_kind;

	if (!same_kind)
	{
		err_set;
		printf("Binary expr cannot be done on different type kinds\n");
		debug_print_binary_expr(binary_expr, 0); return {};
	}

	type_implicit_binary_cast(cc, &lhs, &rhs);
	//@Todo int basic types arent accounted for during this

	switch (op)
	{
	case BinaryOp::LOGIC_AND:
	case BinaryOp::LOGIC_OR:
	{
		if (lhs_kind == Type_Kind::Bool) return rhs;
		err_set; printf("BINARY Logic Ops (&& ||) only work on bools\n");
		debug_print_binary_expr(binary_expr, 0); return {};
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
		debug_print_binary_expr(binary_expr, 0); return {};
	}
	case BinaryOp::PLUS:
	case BinaryOp::MINUS:
	case BinaryOp::TIMES:
	case BinaryOp::DIV:
	{
		if (lhs_kind == Type_Kind::Float || lhs_kind == Type_Kind::Integer) return lhs;
		err_set; printf("BINARY Math Ops (+ - * /) only work on floats or integers\n");
		debug_print_binary_expr(binary_expr, 0); return {};
	}
	case BinaryOp::MOD:
	{
		if (lhs_kind == Type_Kind::Integer) return lhs;
		err_set; printf("BINARY Op %% only works on integers\n");
		debug_print_binary_expr(binary_expr, 0); return {};
	}
	case BinaryOp::BITWISE_AND:
	case BinaryOp::BITWISE_OR:
	case BinaryOp::BITWISE_XOR:
	case BinaryOp::BITSHIFT_LEFT:
	case BinaryOp::BITSHIFT_RIGHT:
	{
		if (lhs_kind == Type_Kind::Integer) return lhs;
		err_set; printf("BINARY Bitwise Ops (& | ^ << >>) only work on integers\n");
		debug_print_binary_expr(binary_expr, 0); return {};
	}
	}

}

option<Literal> check_const_expr(Ast_Expr* expr)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term: return check_const_term(expr->as_term);
	case Ast_Expr_Tag::Unary_Expr: return check_const_unary_expr(expr->as_unary_expr);
	case Ast_Expr_Tag::Binary_Expr: return check_const_binary_expr(expr->as_binary_expr);
	}
}

option<Literal> check_const_term(Ast_Term* term)
{
	switch (term->tag)
	{
	case Ast_Term_Tag::Literal:
	{
		Token token = term->as_literal.token;
		Literal lit = {};

		if (token.type == TokenType::BOOL_LITERAL)
		{
			lit.kind = Literal_Kind::Bool;
			lit.as_bool = token.bool_value;
		}
		else if (token.type == TokenType::FLOAT_LITERAL)
		{
			lit.kind = Literal_Kind::Float;
			lit.as_f64 = token.float64_value;
		}
		else
		{
			lit.kind = Literal_Kind::UInt;
			lit.as_u64 = token.integer_value;
		}

		return lit;
	}
	default: return {};
	}
}

option<Literal> check_const_unary_expr(Ast_Unary_Expr* unary_expr)
{
	option<Literal> rhs_result = check_const_expr(unary_expr->right);
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
	}
}

option<Literal> check_const_binary_expr(Ast_Binary_Expr* binary_expr)
{
	option<Literal> lhs_result = check_const_expr(binary_expr->left);
	if (!lhs_result) return {};
	option<Literal> rhs_result = check_const_expr(binary_expr->right);
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
	}
}

option<Ast_Type> check_const_expr(Check_Context* cc, Ast_Const_Expr* const_expr)
{
	std::vector<Ast_Const_Expr*> dependencies;
	Const_Eval eval = check_const_expr_eval(cc, const_expr->expr, dependencies);
	if (eval == Const_Eval::Invalid) return {};

	printf("Const_Expr: \n");
	err_context(cc, const_expr->expr->span);
	printf("Dependencies: \n");
	for (Ast_Const_Expr* const_expr : dependencies) err_context(cc, const_expr->expr->span);
}

Const_Eval check_const_expr_eval(Check_Context* cc, Ast_Expr* expr, std::vector<Ast_Const_Expr*>& dependencies)
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
			check_var_resolve(cc, var);
			if (var->tag == Ast_Var_Tag::Invalid) return Const_Eval::Invalid;
			if (var->tag == Ast_Var_Tag::Local)
			{
				err_report(Error::CONST_VAR_IS_NOT_GLOBAL);
				err_context(cc, expr->span);
				return Const_Eval::Invalid;
			}

			Ast_Global_Decl* global_decl = var->global.global_decl;
			Const_Eval global_eval = global_decl->const_expr.eval;
			if (global_eval == Const_Eval::Invalid) return Const_Eval::Invalid;
			if (global_eval == Const_Eval::Not_Evaluated) dependencies.emplace_back(&global_decl->const_expr);
			return Const_Eval::Not_Evaluated;
		}
		case Ast_Term_Tag::Enum:
		{
			Ast_Enum* _enum = term->as_enum;
			check_enum_resolve(cc, _enum);
			if (_enum->tag == Ast_Enum_Tag::Invalid) return Const_Eval::Invalid;
			return Const_Eval::Not_Evaluated;
		}
		case Ast_Term_Tag::Sizeof:
		{
			//@How to check if sizeof type signature is valid?
			//also consider with array size expressions
			return Const_Eval::Not_Evaluated;
		}
		case Ast_Term_Tag::Literal:
		{
			return Const_Eval::Not_Evaluated;
		}
		case Ast_Term_Tag::Proc_Call:
		{
			err_report(Error::CONST_PROC_IS_NOT_CONST);
			err_context(cc, expr->span);
			return Const_Eval::Invalid;
		}
		case Ast_Term_Tag::Struct_Init:
		{
			Ast_Struct_Init* struct_init = term->as_struct_init;
			for (Ast_Expr* input_expr : struct_init->input_exprs)
			if (check_const_expr_eval(cc, input_expr, dependencies) == Const_Eval::Invalid) return Const_Eval::Invalid;
			return Const_Eval::Not_Evaluated;
		}
		case Ast_Term_Tag::Array_Init:
		{
			//@Consider array type signature
			Ast_Array_Init* array_init = term->as_array_init;
			for (Ast_Expr* input_expr : array_init->input_exprs)
			if (check_const_expr_eval(cc, input_expr, dependencies) == Const_Eval::Invalid) return Const_Eval::Invalid;
			return Const_Eval::Not_Evaluated;
		}
		}
	}
	case Ast_Expr_Tag::Unary_Expr:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		if (check_const_expr_eval(cc, unary_expr->right, dependencies) == Const_Eval::Invalid) return Const_Eval::Invalid;
		return Const_Eval::Not_Evaluated;
	}
	case Ast_Expr_Tag::Binary_Expr:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		if (check_const_expr_eval(cc, binary_expr->left, dependencies) == Const_Eval::Invalid) return Const_Eval::Invalid;
		if (check_const_expr_eval(cc, binary_expr->right, dependencies) == Const_Eval::Invalid) return Const_Eval::Invalid;
		return Const_Eval::Not_Evaluated;
	}
	}
}

void check_var_resolve(Check_Context* cc, Ast_Var* var)
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
		option<Ast_Global_Info> global = find_global(target_ast, global_ident);
		if (global) 
		{
			var->tag = Ast_Var_Tag::Global;
			var->global.global_decl = global.value().global_decl;
			var->global.global_id = global.value().global_id;
			return;
		}
		else
		{
			err_report(Error::VAR_GLOBAL_IS_NOT_FOUND);
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
			var->global.global_decl = global.value().global_decl;
			var->global.global_id = global.value().global_id;
			return;
		}
	}

	var->tag = Ast_Var_Tag::Local;
}

void check_enum_resolve(Check_Context* cc, Ast_Enum* _enum)
{
	if (_enum->tag != Ast_Enum_Tag::Unresolved) return;

	Ast* target_ast = find_import(cc, _enum->unresolved.import);
	if (target_ast == NULL) 
	{
		_enum->tag = Ast_Enum_Tag::Invalid; 
		return;
	}

	option<Ast_Enum_Info> enum_info = find_enum(target_ast, _enum->unresolved.ident);
	if (!enum_info)
	{
		err_report(Error::ENUM_UNDECLARED);
		err_context(cc, _enum->unresolved.ident.span);
		_enum->tag = Ast_Enum_Tag::Invalid;
		return;
	}

	Ast_Enum_Decl* enum_decl = enum_info.value().enum_decl;
	option<u32> variant_id = find_enum_variant(enum_decl, _enum->unresolved.variant);
	if (!variant_id)
	{
		err_report(Error::ENUM_VARIANT_UNDECLARED);
		err_context(cc, _enum->unresolved.variant.span);
		_enum->tag = Ast_Enum_Tag::Invalid;
		return;
	}

	_enum->tag = Ast_Enum_Tag::Resolved;
	_enum->resolved.type = Ast_Enum_Type { enum_info.value().enum_id, enum_info.value().enum_decl };
	_enum->resolved.variant_id = variant_id.value();
}

void check_proc_call_resolve(Check_Context* cc, Ast_Proc_Call* proc_call)
{
	if (proc_call->tag != Ast_Proc_Call_Tag::Unresolved) return;

	Ast* target_ast = find_import(cc, proc_call->unresolved.import);
	if (target_ast == NULL)
	{
		proc_call->tag = Ast_Proc_Call_Tag::Invalid;
		return;
	}

	option<Ast_Proc_Info> proc_info = find_proc(target_ast, proc_call->unresolved.ident);
	if (!proc_info)
	{
		err_report(Error::PROC_UNDECLARED);
		err_context(cc, proc_call->unresolved.ident.span);
		proc_call->tag = Ast_Proc_Call_Tag::Invalid;
		return;
	}

	proc_call->tag = Ast_Proc_Call_Tag::Resolved;
	proc_call->resolved.proc_id = proc_info.value().proc_id;
	proc_call->resolved.proc_decl = proc_info.value().proc_decl;
}

void check_struct_init_resolve(Check_Context* cc, Ast_Struct_Init* struct_init)
{
	if (struct_init->tag != Ast_Struct_Init_Tag::Unresolved) return;

	Ast* target_ast = find_import(cc, struct_init->unresolved.import);
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
			err_report(Error::STRUCT_UNDECLARED);
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
}
