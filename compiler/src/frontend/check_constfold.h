#ifndef CHECK_CONSTFOLD_H
#define CHECK_CONSTFOLD_H

#include "check_type.h"

struct Literal;
enum class Literal_Kind;

option<Ast_Type> check_constfold_expr(Check_Context* cc, Expr_Context context, Ast_Expr* expr);

static option<Literal> constfold_literal_from_expr(Check_Context* cc, Ast_Expr* expr);
static option<Ast_Type> constfold_range_check(Check_Context* cc, Expr_Context context, Ast_Expr* expr, Literal lit);
static option<Literal> constfold_literal_from_folded_expr(Ast_Folded_Expr folded);
static option<Literal> constfold_term_var(Check_Context* cc, Ast_Var* var);
static option<Literal> constfold_term_enum(Ast_Enum* _enum);
static option<Literal> constfold_term_cast(Check_Context* cc, Ast_Cast* cast);
static option<Literal> constfold_term_sizeof(Ast_Sizeof* size_of);
static option<Literal> constfold_term_literal(Ast_Literal* literal);
static option<Literal> constfold_unary_minus(Check_Context* cc, Literal rhs);
static option<Literal> constfold_unary_logic_not(Check_Context* cc, Literal rhs);
static option<Literal> constfold_unary_bitwise_not(Check_Context* cc, Literal rhs);
static option<Literal> constfold_unary_address_of(Check_Context* cc, Literal rhs);
static option<Literal> constfold_unary_dereference(Check_Context* cc, Literal rhs);
static option<Literal> constfold_binary_logic_and(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_logic_or(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_less(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_greater(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_less_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_greater_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_is_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_not_equals(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_plus(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_minus(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_times(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_div(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_mod(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_bitwise_and(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_bitwise_or(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_bitwise_xor(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_bitshift_left(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);
static option<Literal> constfold_binary_bitshift_right(Check_Context* cc, Literal lhs, Literal rhs, Literal_Kind kind);

#endif
