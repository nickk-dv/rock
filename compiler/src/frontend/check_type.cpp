#include "check_type.h"

#include "check_general.h"
#include "debug_printer.h"

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
		err_set;
		printf("[COMPILER ERROR] Ast_Type signature wasnt checked, tag cannot be Tag::Custom\n");
		printf("Hint: submit a bug report if you see this error message\n");
		debug_print_type(type);
		printf("\n");
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
		option<Ast_Type> expr_type = check_expr_type(cc, type->as_array->const_expr, type_from_basic(BasicType::I32), true);
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

		err_set;
		printf("Failed to find the custom type: ");
		debug_print_custom_type(type->as_custom);
		printf("\n");
		debug_print_ident(type->as_custom->ident);
		printf("\n");
		return {};
	}
	default:
	{
		err_set;
		printf("[COMPILER ERROR] Ast_Type signature cannot be checked multiple times\n");
		printf("Hint: submit a bug report if you see this error message\n");
		debug_print_type(*type);
		printf("\n");
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
		err_set;
		printf("Type mismatch:\n");
		printf("Expected: "); debug_print_type(expect_type.value()); printf("\n");
		printf("Got:      "); debug_print_type(type.value()); printf("\n");
		//@Temp printing here
		printf("In Expr: ");
		for (u32 i = expr->span.start; i <= expr->span.end; i += 1)
		{
			printf("%c", cc->ast->source.data[i]);
		}
		printf("\n\n");
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
		err_set; printf("match_type: Unexpected Ast_Type_Tag. Disambiguate Tag::Custom by using check_type first:\n");
		debug_print_type(type_a); printf("\n");
		debug_print_type(type_b); printf("\n");
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
	if (check_is_const_expr(expr))
	{
		expr->is_const = true;
	}

	if (!check_is_const_foldable_expr(expr))
	{
		if (expr->is_const == false && context->expect_constant)
		{
			err_set;
			printf("Expected constant expression. Got: \n");
			debug_print_expr(expr, 0);
			printf("\n\n");
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
		if (!lit_result)
		{
			err_set; //propagate err inside check const expr instead of here
			printf("Invalid const expr");
			debug_print_expr(expr, 0);
			printf("\n\n");
			return {};
		}

		Ast_Const_Expr const_expr = {};
		Literal lit = lit_result.value();

		if (context)
		{
			switch (lit.kind)
			{
			case Literal_Kind::Bool:
			{
				const_expr.basic_type = BasicType::BOOL;
				const_expr.as_bool = lit.as_bool;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
				return type_from_basic(BasicType::BOOL);
			}
			case Literal_Kind::Float:
			{
				//@Base on context
				const_expr.basic_type = BasicType::F64;
				const_expr.as_f64 = lit.as_f64;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
				return type_from_basic(BasicType::F64);
			}
			case Literal_Kind::Int:
			{
				//@Base on context
				//@Todo range based
				const_expr.basic_type = BasicType::I32;
				const_expr.as_i64 = lit.as_i64;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
				return type_from_basic(BasicType::I32);
			}
			case Literal_Kind::UInt:
			{
				//@Base on context
				//@Todo range based
				const_expr.basic_type = BasicType::I32;
				const_expr.as_u64 = lit.as_u64;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
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
				const_expr.basic_type = BasicType::BOOL;
				const_expr.as_bool = lit.as_bool;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
				return type_from_basic(BasicType::BOOL);
			}
			case Literal_Kind::Float:
			{
				const_expr.basic_type = BasicType::F64;
				const_expr.as_f64 = lit.as_f64;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
				return type_from_basic(BasicType::F64);
			}
			case Literal_Kind::Int:
			{
				//@Todo range based default int type
				const_expr.basic_type = BasicType::I32;
				const_expr.as_i64 = lit.as_i64;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
				return type_from_basic(BasicType::I32);
			}
			case Literal_Kind::UInt:
			{
				//@Todo range based default int type
				//@Might become a u64 if its too big
				const_expr.basic_type = BasicType::I32;
				const_expr.as_u64 = lit.as_u64;
				expr->tag = Ast_Expr_Tag::Const_Expr;
				expr->as_const_expr = const_expr;
				return type_from_basic(BasicType::I32);
			}
			}
		}
	}
}

option<Ast_Type> check_term(Check_Context* cc, Type_Context* context, Ast_Term* term)
{
	switch (term->tag)
	{
	case Ast_Term_Tag::Var: return check_var(cc, term->as_var);
	case Ast_Term_Tag::Enum: //@Const fold this should be part of constant evaluation
	{
		Ast_Enum* _enum = term->as_enum;
		Ast* target_ast = find_import(cc, _enum->import);
		if (target_ast == NULL) return {};

		option<Ast_Enum_Info> enum_meta = find_enum(target_ast, _enum->ident);
		if (!enum_meta) { err_set; error("Accessing undeclared enum", _enum->ident); return {}; }
		Ast_Enum_Decl* enum_decl = enum_meta.value().enum_decl;
		_enum->enum_id = enum_meta.value().enum_id;

		option<u32> variant_id = find_enum_variant(enum_decl, _enum->variant);
		if (!variant_id) { err_set; error("Accessing undeclared enum variant", _enum->variant); }
		else _enum->variant_id = variant_id.value();

		Ast_Type type = {};
		type.tag = Ast_Type_Tag::Enum;
		type.as_enum.enum_id = _enum->enum_id;
		type.as_enum.enum_decl = enum_meta.value().enum_decl;
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
			err_set;
			printf("Check_term: Unsupported literal term, only string literals are allowed to be proccesed:\n");
			debug_print_token(literal.token, true, true);
			printf("\n");
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
		Ast* target_ast = find_import(cc, struct_init->import);
		if (target_ast == NULL) return {};

		// find struct
		Ast_Struct_Decl* struct_decl = NULL;
		u32 struct_id = 0;

		if (struct_init->ident)
		{
			Ast_Ident ident = struct_init->ident.value();
			option<Ast_Struct_Info> struct_meta = find_struct(target_ast, ident);
			if (!struct_meta)
			{
				err_set;
				error("Struct type identifier wasnt found", ident);
				return {};
			}
			struct_decl = struct_meta.value().struct_decl;
			struct_id = struct_meta.value().struct_id;
		}

		if (context->expect_type)
		{
			Ast_Type expect_type = context->expect_type.value();
			if (type_kind(cc, expect_type) != Type_Kind::Struct)
			{
				err_set; printf("Cannot use struct initializer in non struct type context\n");
				printf("Context: "); debug_print_type(expect_type); printf("\n");
				return {};
			}

			Ast_Struct_Type expected_struct = expect_type.as_struct;
			if (struct_decl == NULL)
			{
				struct_decl = expected_struct.struct_decl;
				struct_id = expected_struct.struct_id;
			}
			else
			{
				if (struct_id != expected_struct.struct_id)
				{
					err_set;
					printf("Struct initializer struct type doesnt match the expected type:\n");
					debug_print_struct_init(struct_init, 0); printf("\n");
					return {};
				}
			}
		}

		if (struct_decl == NULL)
		{
			err_set;
			printf("Cannot infer the struct initializer type without a context\n");
			printf("Hint: specify type on varible: var : Type = .{ ... }, or on initializer var := Type.{ ... }\n");
			debug_print_struct_init(struct_init, 0); printf("\n");
			return {};
		}

		// check input count
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

		struct_init->struct_id = struct_id;

		Ast_Type type = {};
		type.tag = Ast_Type_Tag::Struct;
		type.as_struct.struct_id = struct_id;
		type.as_struct.struct_decl = struct_decl;

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
	Ast_Ident ident = var->ident;
	Ast* target_ast = cc->ast;

	option<Ast_Import_Decl*> import_decl = cc->ast->import_table.find(ident, hash_ident(ident));
	if (import_decl && var->access && var->access.value()->tag == Ast_Access_Tag::Var)
	{
		target_ast = import_decl.value()->import_ast;
		Ast_Var_Access* var_access = var->access.value()->as_var;
		var->access = var_access->next;
		Ast_Ident global_ident = var_access->ident;

		option<Ast_Global_Info> global = find_global(target_ast, global_ident);
		if (global)
		{
			var->global_id = global.value().global_id;
			option<Ast_Type> type = global.value().global_decl->type;
			if (type) return check_access(cc, type.value(), var->access);
			else return {};
		}
		else
		{
			err_set;
			error_pair("Failed to find global variable in module", "Module: ", ident, "Global identifier: ", global_ident);
		}
	}
	else
	{
		option<Ast_Global_Info> global = find_global(target_ast, ident);
		if (global)
		{
			var->global_id = global.value().global_id;
			option<Ast_Type> type = global.value().global_decl->type;
			if (type) return check_access(cc, type.value(), var->access);
			else return {};
		}
		else
		{
			option<Ast_Type> type = check_context_block_find_var_type(cc, var->ident);
			if (type) return check_access(cc, type.value(), var->access);

			err_set;
			error("Check var: var is not found or has not valid type", var->ident);
			return {};
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
	Ast* target_ast = find_import(cc, proc_call->import);
	if (target_ast == NULL) return {};

	// find proc
	Ast_Ident ident = proc_call->ident;
	option<Ast_Proc_Info> proc_meta = find_proc(target_ast, ident);
	if (!proc_meta)
	{
		err_set;
		error("Calling undeclared procedure", ident);
		return {};
	}
	Ast_Proc_Decl* proc_decl = proc_meta.value().proc_decl;
	proc_call->proc_id = proc_meta.value().proc_id;

	// check input count
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
			debug_print_ident(ident, true, true); printf("\n");
		}
	}
	else
	{
		if (param_count != input_count)
		{
			err_set;
			printf("Unexpected number of arguments in procedure call:\n");
			printf("Expected: %lu Got: %lu \n", param_count, input_count);
			debug_print_ident(ident, true, true); printf("\n");
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

// perform operations on highest sized types and only on call site determine
// and type check / infer the type based on the resulting value
// the only possible errors are:
// 1. int range overflow
// 2. invalid op semantics
// 3. float NaN
// 4. division / mod by 0

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
