#include "check_constfold.h"

enum class Literal_Kind
{
	Bool,
	Float,
	Int,
	Uint
};

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
	u32 variant_id = 0;

	Literal(bool val) { kind = Literal_Kind::Bool; this->as_bool = val; }
	Literal(f64 val) { kind = Literal_Kind::Float; this->as_f64 = val; }
	Literal(i64 val) { kind = Literal_Kind::Int; this->as_i64 = val; }
	Literal(u64 val) { kind = Literal_Kind::Uint; this->as_u64 = val; }

	void try_uint_convert()
	{
		if (kind == Literal_Kind::Int && this->as_i64 >= 0)
		{
			kind = Literal_Kind::Uint;
			this->as_u64 = static_cast<u64>(this->as_i64);
		}
	}

	void try_int_convert()
	{
		//@ -INT64_MAX - 1 can still be represented, off by 1 not accounted for here
		//used for unary -
		if (kind == Literal_Kind::Uint && this->as_u64 <= INT64_MAX)
		{
			kind = Literal_Kind::Int;
			this->as_i64 = static_cast<i64>(this->as_i64);
		}
	}
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
		case UnaryOp::MINUS:       return constfold_unary_minus(cc, rhs);
		case UnaryOp::LOGIC_NOT:   return constfold_unary_logic_not(cc, rhs);
		case UnaryOp::BITWISE_NOT: return constfold_unary_bitwise_not(cc, rhs);
		case UnaryOp::ADDRESS_OF:  return constfold_unary_address_of(cc, rhs);
		case UnaryOp::DEREFERENCE: return constfold_unary_dereference(cc, rhs);
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
		//@Convert or math types similar to binary expr type checker
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

	//@Todo enum needs to be stored under folded expr?
	//@Todo also return the enum type as ast_type
	//account for expected type, if also expected enum return type as enum, else return the basic

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
	case BasicType::BOOL: return Literal(folded.as_bool);
	case BasicType::F32:
	case BasicType::F64: return Literal(folded.as_f64);
	case BasicType::I8:
	case BasicType::I16:
	case BasicType::I32:
	case BasicType::I64: return Literal(folded.as_i64);
	case BasicType::U8:
	case BasicType::U16:
	case BasicType::U32:
	case BasicType::U64: return Literal(folded.as_u64);
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
	return Literal(static_cast<u64>(type_size(size_of->type)));
}

option<Literal> constfold_term_literal(Ast_Literal* literal)
{
	Token token = literal->token;
	switch (token.type)
	{
	case TokenType::BOOL_LITERAL:    return Literal(token.bool_value);
	case TokenType::FLOAT_LITERAL:   return Literal(token.float64_value);
	case TokenType::INTEGER_LITERAL: return Literal(token.integer_value);
	default: { err_internal("constfold_term_literal: invalid TokenType"); return {}; }
	}
}

option<Literal> constfold_unary_minus(Check_Context* cc, Literal rhs)
{
	switch (rhs.kind)
	{
	case Literal_Kind::Float: return Literal(-rhs.as_f64);
	case Literal_Kind::Int:   return Literal(-rhs.as_i64);
	case Literal_Kind::Uint:  
	{
		rhs.try_int_convert();
		if (rhs.kind != Literal_Kind::Int) { err_report(Error::FOLD_UNARY_MINUS_OVERFLOW); return {}; }
		return Literal(-rhs.as_i64);
	}
	default: { err_internal("constfold_unary_minus expected numeric type"); return {}; }
	}
}

option<Literal> constfold_unary_logic_not(Check_Context* cc, Literal rhs)
{
	if (rhs.kind != Literal_Kind::Bool) { err_internal("constfold_unary_logic_not expected bool"); return {}; }
	return Literal(!rhs.as_bool);
}

option<Literal> constfold_unary_bitwise_not(Check_Context* cc, Literal rhs)
{
	rhs.try_uint_convert();
	if (rhs.kind != Literal_Kind::Uint) { err_internal("constfold_unary_bitwise_not expected uint"); return {}; }
	return Literal(~rhs.as_u64);
}

option<Literal> constfold_unary_address_of(Check_Context* cc, Literal rhs)
{
	err_internal("constfold_unary_address_of on temporary");
	return {};
}

option<Literal> constfold_unary_dereference(Check_Context* cc, Literal rhs)
{
	err_internal("constfold_unary_dereference on temporary");
	return {};
}

option<Literal> constfold_binary_logic_and(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (kind != Literal_Kind::Bool) { err_internal("constfold_binary_logic_and"); return {}; }
	return Literal(lhs.as_bool && rhs.as_bool);
}

option<Literal> constfold_binary_logic_or(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (kind != Literal_Kind::Bool) { err_internal("constfold_binary_logic_and"); return {}; }
	return Literal(lhs.as_bool || rhs.as_bool);
}

option<Literal> constfold_binary_less(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	switch (kind) 
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 < rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 < rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 < rhs.as_u64);
	default: { err_internal("constfold_binary_less expected numeric"); return {}; } 
	}
}

option<Literal> constfold_binary_greater(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	switch (kind)
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 > rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 > rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 > rhs.as_u64);
	default: { err_internal("constfold_binary_greater expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_less_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	switch (kind)
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 <= rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 <= rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 <= rhs.as_u64);
	default: { err_internal("constfold_binary_less_equals expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_greater_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	switch (kind)
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 >= rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 >= rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 >= rhs.as_u64);
	default: { err_internal("constfold_binary_greater_equals expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_is_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (lhs.enum_type && rhs.enum_type)
	{
		if (lhs.enum_type.value().enum_id != rhs.enum_type.value().enum_id)
		{
			err_report(Error::BINARY_CMP_EQUAL_ON_ENUMS);
			return {};
		}
	}
	
	switch (kind)
	{
	case Literal_Kind::Bool:  return Literal(lhs.as_bool == rhs.as_bool);
	case Literal_Kind::Float: return Literal(lhs.as_f64 == rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 == rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 == rhs.as_u64);
	default: { err_internal("constfold_binary_is_equals invalid Literal_Kind"); return {}; }
	}
}

option<Literal> constfold_binary_not_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (lhs.enum_type && rhs.enum_type)
	{
		if (lhs.enum_type.value().enum_id != rhs.enum_type.value().enum_id)
		{
			err_report(Error::BINARY_CMP_EQUAL_ON_ENUMS);
			return {};
		}
	}

	switch (kind)
	{
	case Literal_Kind::Bool:  return Literal(lhs.as_bool != rhs.as_bool);
	case Literal_Kind::Float: return Literal(lhs.as_f64 != rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 != rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 != rhs.as_u64);
	default: { err_internal("constfold_binary_not_equals invalid Literal_Kind"); return {}; }
	}
}

option<Literal> constfold_binary_plus(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	//@Not range checked or converted
	switch (kind)
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 + rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 + rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 + rhs.as_u64);
	default: { err_internal("constfold_binary_plus expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_minus(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	//@Not range checked or converted
	switch (kind)
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 - rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 - rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 - rhs.as_u64);
	default: { err_internal("constfold_binary_minus expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_times(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	//@Not range checked or converted
	switch (kind)
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 * rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 * rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 * rhs.as_u64);
	default: { err_internal("constfold_binary_times expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_div(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	//@Not range checked or converted
	switch (kind)
	{
	case Literal_Kind::Float: return Literal(lhs.as_f64 / rhs.as_f64);
	case Literal_Kind::Int:   return Literal(lhs.as_i64 / rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 / rhs.as_u64);
	default: { err_internal("constfold_binary_div expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_mod(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	switch (kind)
	{
	case Literal_Kind::Float: if (rhs.as_f64 == 0.0) { err_internal("constfold_binary_mod float modulo of 0.0"); return {}; }
	case Literal_Kind::Int:   if (rhs.as_i64 == 0.0) { err_internal("constfold_binary_mod int modulo of 0"); return {}; }
	case Literal_Kind::Uint:  if (rhs.as_u64 == 0.0) { err_internal("constfold_binary_mod uint modulo of 0"); return {}; }
	default: break;
	}

	switch (kind)
	{
	case Literal_Kind::Float: return Literal(fmod(lhs.as_f64, rhs.as_f64)); //@Hope that it matches llvm FRem behavior, tests needed
	case Literal_Kind::Int:   return Literal(lhs.as_i64 % rhs.as_i64);
	case Literal_Kind::Uint:  return Literal(lhs.as_u64 % rhs.as_u64);
	default: { err_internal("constfold_binary_mod expected numeric"); return {}; }
	}
}

option<Literal> constfold_binary_bitwise_and(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (rhs.kind != Literal_Kind::Uint) { err_internal("constfold_binary_bitwise_and expected uint"); return {}; }
	return Literal(lhs.as_u64 & rhs.as_u64);
}

option<Literal> constfold_binary_bitwise_or(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (rhs.kind != Literal_Kind::Uint) { err_internal("constfold_binary_bitwise_or expected uint"); return {}; }
	return Literal(lhs.as_u64 | rhs.as_u64);
}

option<Literal> constfold_binary_bitwise_xor(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (rhs.kind != Literal_Kind::Uint) { err_internal("constfold_binary_bitwise_xor expected uint"); return {}; }
	return Literal(lhs.as_u64 ^ rhs.as_u64);
}

option<Literal> constfold_binary_bitshift_left(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (rhs.kind != Literal_Kind::Uint) { err_internal("constfold_binary_bitshift_left expected uint"); return {}; }
	return Literal(lhs.as_u64 << rhs.as_u64);
}

option<Literal> constfold_binary_bitshift_right(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind)
{
	if (rhs.kind != Literal_Kind::Uint) { err_internal("constfold_binary_bitshift_right expected uint"); return {}; }
	return Literal(lhs.as_u64 >> rhs.as_u64);
}
