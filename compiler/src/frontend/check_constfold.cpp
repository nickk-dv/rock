#include "check_constfold.h"

struct Literal
{
	Literal_Kind kind;

	union
	{
		bool as_bool;
		f64 as_f64;
		i64 as_i64;
		u64 as_u64;
	};

	option<Ast_Type_Enum> enum_type;
	u32 variant_id;
};

enum class Literal_Kind
{
	Bool,
	Float,
	Int,
	Uint
};

option<Ast_Type> check_constfold_expr(Check_Context* cc, Expr_Context context, Ast_Expr* expr)
{
	option<Literal> lit = constfold_literal_from_expr(cc, expr);
	if (!lit) return {};
	return constfold_range_check(cc, context, expr, lit.value());
}

option<Literal> constfold_literal_from_expr(Check_Context* cc, Ast_Expr* expr)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term:
	{
		Ast_Term* term = expr->as_term;
		switch (term->tag)
		{
		case Ast_Term_Tag::Var:     return constfold_term_var(cc, term->as_var);
		case Ast_Term_Tag::Enum:    return constfold_term_enum(term->as_enum);
		case Ast_Term_Tag::Cast:    return constfold_term_cast(cc, term->as_cast);
		case Ast_Term_Tag::Sizeof:  return constfold_term_sizeof(term->as_sizeof);
		case Ast_Term_Tag::Literal: return constfold_term_literal(term->as_literal);
		default: { err_internal("constfold_literal_from_expr: invalid Ast_Term_Tag"); return {}; }
		}
	}
	case Ast_Expr_Tag::Unary:
	{
		Ast_Unary_Expr* unary_expr = expr->as_unary_expr;
		option<Literal> rhs_result = constfold_literal_from_expr(cc, unary_expr->right);
		if (!rhs_result) return {};

		UnaryOp op = unary_expr->op;
		Literal rhs = rhs_result.value();
		Literal_Kind rhs_kind = rhs.kind;

		switch (op)
		{
		case UnaryOp::MINUS:       return constfold_unary_minus(cc, rhs, rhs_kind);
		case UnaryOp::LOGIC_NOT:   return constfold_unary_logic_not(cc, rhs, rhs_kind);
		case UnaryOp::BITWISE_NOT: return constfold_unary_bitwise_not(cc, rhs, rhs_kind);
		case UnaryOp::ADDRESS_OF:  return constfold_unary_address_of(cc, rhs, rhs_kind);
		case UnaryOp::DEREFERENCE: return constfold_unary_dereference(cc, rhs, rhs_kind);
		default: { err_internal("constfold_literal_from_expr: invalid UnaryOp"); return {}; }
		}
	}
	case Ast_Expr_Tag::Binary:
	{
		Ast_Binary_Expr* binary_expr = expr->as_binary_expr;
		option<Literal> lhs_result = constfold_literal_from_expr(cc, binary_expr->left);
		option<Literal> rhs_result = constfold_literal_from_expr(cc, binary_expr->right);
		if (!lhs_result) return {};
		if (!rhs_result) return {};

		BinaryOp op = binary_expr->op;
		Literal lhs = lhs_result.value();
		Literal rhs = rhs_result.value();
		Literal_Kind lhs_kind = lhs.kind;
		Literal_Kind rhs_kind = rhs.kind;

		switch (op)
		{
		case BinaryOp::LOGIC_AND:      return constfold_binary_logic_and(cc, lhs, rhs, lhs_kind);
		case BinaryOp::LOGIC_OR:       return constfold_binary_logic_or(cc, lhs, rhs, lhs_kind);
		case BinaryOp::LESS:           return constfold_binary_less(cc, lhs, rhs, lhs_kind);
		case BinaryOp::GREATER:        return constfold_binary_greater(cc, lhs, rhs, lhs_kind);
		case BinaryOp::LESS_EQUALS:    return constfold_binary_less_equals(cc, lhs, rhs, lhs_kind);
		case BinaryOp::GREATER_EQUALS: return constfold_binary_greater_equals(cc, lhs, rhs, lhs_kind);
		case BinaryOp::IS_EQUALS:      return constfold_binary_is_equals(cc, lhs, rhs, lhs_kind);
		case BinaryOp::NOT_EQUALS:     return constfold_binary_not_equals(cc, lhs, rhs, lhs_kind);
		case BinaryOp::PLUS:           return constfold_binary_plus(cc, lhs, rhs, lhs_kind);
		case BinaryOp::MINUS:          return constfold_binary_minus(cc, lhs, rhs, lhs_kind);
		case BinaryOp::TIMES:          return constfold_binary_times(cc, lhs, rhs, lhs_kind);
		case BinaryOp::DIV:            return constfold_binary_div(cc, lhs, rhs, lhs_kind);
		case BinaryOp::MOD:            return constfold_binary_mod(cc, lhs, rhs, lhs_kind);
		case BinaryOp::BITWISE_AND:    return constfold_binary_bitwise_and(cc, lhs, rhs, lhs_kind);
		case BinaryOp::BITWISE_OR:     return constfold_binary_bitwise_or(cc, lhs, rhs, lhs_kind);
		case BinaryOp::BITWISE_XOR:    return constfold_binary_bitwise_xor(cc, lhs, rhs, lhs_kind);
		case BinaryOp::BITSHIFT_LEFT:  return constfold_binary_bitshift_left(cc, lhs, rhs, lhs_kind);
		case BinaryOp::BITSHIFT_RIGHT: return constfold_binary_bitshift_right(cc, lhs, rhs, lhs_kind);
		default: { err_internal("constfold_literal_from_expr: invalid BinaryOp"); return {}; }
		}
	}
	case Ast_Expr_Tag::Folded:
	{
		return constfold_literal_from_folded_expr(expr->as_folded_expr);
	}
	default: { err_internal("constfold_literal_from_expr: invalid Ast_Expr_Tag"); return {}; }
	}
}

option<Ast_Type> constfold_range_check(Check_Context* cc, Expr_Context context, Ast_Expr* expr, Literal lit)
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
		bool default_type = true;

		if (context.expect_type)
		{
			Ast_Type type = context.expect_type.value();
			if (type.tag == Ast_Type_Tag::Basic)
			{
				BasicType basic_type = type.as_basic;
				default_type = false;

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
				default: default_type = true; break;
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
				default: default_type = true; break;
				}
			}
		}

		if (default_type)
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
	case Literal_Kind::Uint:
	{
		bool default_type = true;

		if (context.expect_type)
		{
			Ast_Type type = context.expect_type.value();
			if (type.tag == Ast_Type_Tag::Basic)
			{
				BasicType basic_type = type.as_basic;
				default_type = false;

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
				default: default_type = true; break;
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
				default: default_type = true; break;
				}
			}
		}

		if (default_type)
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
	default: { err_internal("constfold_range_check: invalid Literal_Kind"); return {}; }
	}

	expr->tag = Ast_Expr_Tag::Folded;
	expr->as_folded_expr = folded;
	return type_from_basic(folded.basic_type);
}

option<Literal> constfold_literal_from_folded_expr(Ast_Folded_Expr folded)
{
	switch (folded.basic_type)
	{
	case BasicType::BOOL: return Literal { .kind = Literal_Kind::Bool, .as_bool = folded.as_bool };
	case BasicType::F32:
	case BasicType::F64:  return Literal { .kind = Literal_Kind::Float, .as_f64 = folded.as_f64 };
	case BasicType::I8:
	case BasicType::I16:
	case BasicType::I32:
	case BasicType::I64:  return Literal { .kind = Literal_Kind::Int, .as_i64 = folded.as_i64 };
	case BasicType::U8:
	case BasicType::U16:
	case BasicType::U32:
	case BasicType::U64:  return Literal { .kind = Literal_Kind::Uint, .as_u64 = folded.as_u64 };
	default: { err_internal("constfold_literal_from_folded_expr invalid BasicType"); return {};  }
	}
}

option<Literal> constfold_term_var(Check_Context* cc, Ast_Var* var)
{
	//@Todo access unsupported
	Ast_Consteval_Expr* consteval_expr = var->global.global_decl->consteval_expr;
	return constfold_literal_from_expr(cc, consteval_expr->expr);
}

option<Literal> constfold_term_enum(Ast_Enum* _enum)
{
	Ast_Decl_Enum* enum_decl = _enum->resolved.type.enum_decl;
	Ast_Enum_Variant* enum_variant = &enum_decl->variants[_enum->resolved.variant_id];

	option<Literal> lit = constfold_literal_from_folded_expr(enum_variant->consteval_expr->expr->as_folded_expr);
	if (!lit) return {};
	lit.value().enum_type = _enum->resolved.type;
	lit.value().variant_id = _enum->resolved.variant_id;
	return lit;
}

option<Literal> constfold_term_cast(Check_Context* cc, Ast_Cast* cast)
{
	option<Literal> lit_result = constfold_literal_from_expr(cc, cast->expr);
	if (!lit_result) return {};
	Literal lit = lit_result.value();
	Ast_Expr* expr = cast->expr;

	//@int cast can be important for bitwise operations in constants
	//@check that literal kind matches cast tag
	switch (cast->tag)
	{
	case Ast_Resolve_Cast_Tag::Integer_No_Op:            { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
	case Ast_Resolve_Cast_Tag::Int_Trunc____LLVMTrunc:   { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
	case Ast_Resolve_Cast_Tag::Uint_Extend__LLVMZExt:    { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
	case Ast_Resolve_Cast_Tag::Int_Extend___LLVMSExt:    { err_report(Error::CAST_FOLD_REDUNDANT_INT_CAST); err_context(cc, expr->span); } break;
	case Ast_Resolve_Cast_Tag::Float_Uint___LLVMFPToUI:  { lit.as_u64 = static_cast<u64>(lit.as_f64); lit.kind = Literal_Kind::Uint;  } break; //@Basic type impacts the result, negative float wraps, disallow? range check?
	case Ast_Resolve_Cast_Tag::Float_Int____LLVMFPToSI:  { lit.as_i64 = static_cast<i64>(lit.as_f64); lit.kind = Literal_Kind::Int;   } break; //@Basic type impacts the result, range check?
	case Ast_Resolve_Cast_Tag::Uint_Float___LLVMUIToFP:  { lit.as_f64 = static_cast<f64>(lit.as_u64); lit.kind = Literal_Kind::Float; } break;
	case Ast_Resolve_Cast_Tag::Int_Float____LLVMSIToFP:  { lit.as_f64 = static_cast<f64>(lit.as_i64); lit.kind = Literal_Kind::Float; } break;
	case Ast_Resolve_Cast_Tag::Float_Trunc__LLVMFPTrunc: { err_report(Error::CAST_FOLD_REDUNDANT_FLOAT_CAST); err_context(cc, expr->span); } break;
	case Ast_Resolve_Cast_Tag::Float_Extend_LLVMFPExt:   { err_report(Error::CAST_FOLD_REDUNDANT_FLOAT_CAST); err_context(cc, expr->span); } break;
	default: { err_internal("constfold_term_cast: invalid Ast_Resolve_Cast_Tag"); return {}; }
	}

	return lit;
}

option<Literal> constfold_term_sizeof(Ast_Sizeof* size_of)
{
	return Literal { .kind = Literal_Kind::Uint, .as_u64 = type_size(size_of->type) };
}

option<Literal> constfold_term_literal(Ast_Literal* literal)
{
	Token token = literal->token;
	switch (token.type)
	{
	case TokenType::BOOL_LITERAL:    return Literal { .kind = Literal_Kind::Bool, .as_bool = token.bool_value };
	case TokenType::FLOAT_LITERAL:   return Literal { .kind = Literal_Kind::Float, .as_f64 = token.float64_value };
	case TokenType::INTEGER_LITERAL: return Literal { .kind = Literal_Kind::Uint, .as_u64 = token.integer_value };
	default: { err_internal("constfold_term_literal: invalid TokenType"); return {}; }
	}
}

option<Literal> constfold_unary_minus(Check_Context* cc, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_unary_logic_not(Check_Context* cc, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_unary_bitwise_not(Check_Context* cc, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_unary_address_of(Check_Context* cc, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_unary_dereference(Check_Context* cc, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_logic_and(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_logic_or(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_less(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_greater(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_less_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_greater_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_is_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_not_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_plus(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_minus(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_times(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_div(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_mod(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_bitwise_and(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_bitwise_or(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_bitwise_xor(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_bitshift_left(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}

option<Literal> constfold_binary_bitshift_right(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	return {};
}
