#ifndef CHECK_TYPE_H
#define CHECK_TYPE_H

#include "check_context.h"

struct Literal;
struct Type_Context;
enum class Type_Kind;
enum class Literal_Kind;

Type_Kind type_kind(Check_Context* cc, Ast_Type type);
Ast_Type type_from_basic(BasicType basic_type);
option<Ast_Type> check_type_signature(Check_Context* cc, Ast_Type* type);
option<Ast_Type> check_expr_type(Check_Context* cc, Ast_Expr* expr, option<Ast_Type> expect_type, bool expect_constant);
option<Ast_Type> check_var(Check_Context* cc, Ast_Var* var);
option<Ast_Type> check_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call, Checker_Proc_Call_Flags flags);
bool match_type(Check_Context* cc, Ast_Type type_a, Ast_Type type_b);

static bool check_is_const_expr(Ast_Expr* expr);
static bool check_is_const_foldable_expr(Ast_Expr* expr);
static void type_implicit_cast(Check_Context* cc, Ast_Type* type, Ast_Type target_type);
static void type_implicit_binary_cast(Check_Context* cc, Ast_Type* type_a, Ast_Type* type_b);
static option<Ast_Type> check_expr(Check_Context* cc, Type_Context* context, Ast_Expr* expr);
static option<Ast_Type> check_term(Check_Context* cc, Type_Context* context, Ast_Term* term);
static option<Ast_Type> check_access(Check_Context* cc, Ast_Type type, option<Ast_Access*> optional_access);
static option<Ast_Type> check_unary_expr(Check_Context* cc, Type_Context* context, Ast_Unary_Expr* unary_expr);
static option<Ast_Type> check_binary_expr(Check_Context* cc, Type_Context* context, Ast_Binary_Expr* binary_expr);
static option<Literal> check_const_expr(Ast_Expr* expr);
static option<Literal> check_const_term(Ast_Term* term);
static option<Literal> check_const_unary_expr(Ast_Unary_Expr* unary_expr);
static option<Literal> check_const_binary_expr(Ast_Binary_Expr* binary_expr);

static option<Ast_Type> check_const_expr(Check_Context* cc, Ast_Const_Expr* const_expr);
static Const_Eval check_const_expr_eval(Check_Context* cc, Ast_Expr* expr, std::vector<Ast_Const_Expr*>& dependencies);
static bool check_var_signature(Check_Context* cc, Ast_Var* var);
static void check_enum_resolve(Check_Context* cc, Ast_Enum* _enum);

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
};

struct Type_Context
{
	option<Ast_Type> expect_type;
	bool expect_constant;
};

enum class Type_Kind
{
	Bool,
	Float,
	Integer,
	String,
	Pointer,
	Array,
	Struct,
	Enum,
};

enum class Literal_Kind
{
	Bool,
	Float,
	Int,
	UInt
};

#endif
