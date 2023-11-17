#ifndef CHECK_TYPE_H
#define CHECK_TYPE_H

#include "check_context.h"
#include "general/tree.h"

struct Literal;
struct Expr_Context;
struct Consteval_Dependency;
enum class Type_Kind;
enum class Literal_Kind;
enum class Expr_Kind;
enum class Expr_Constness;
enum class Consteval_Dependency_Tag;

struct Expr_Context
{
	option<Ast_Type> expect_type;
	Expr_Constness constness;
};

bool type_is_poison(Ast_Type type);
bool type_match(Ast_Type type_a, Ast_Type type_b);
Type_Kind type_kind(Ast_Type type);
Ast_Type type_from_poison();
Ast_Type type_from_basic(BasicType basic_type);
option<Ast_Type_Struct> type_extract_struct_value_type(Ast_Type type);
option<Ast_Type_Array*> type_extract_array_value_type(Ast_Type type);
static void compute_struct_size(Ast_Decl_Struct* struct_decl);
static u32 type_basic_size(BasicType basic_type);
static u32 type_basic_align(BasicType basic_type);
static u32 type_size(Ast_Type type);
static u32 type_align(Ast_Type type);
static void type_auto_cast(Ast_Type* type, Ast_Type expect_type, Ast_Expr* expr);
static void type_auto_binary_cast(Ast_Type* type_a, Ast_Type* type_b, Ast_Expr* lhs_expr, Ast_Expr* rhs_expr);

option<Ast_Type> check_expr_type(Check_Context* cc, Ast_Expr* expr, option<Ast_Type> type, Expr_Constness constness);
option<Ast_Type> check_var(Check_Context* cc, Ast_Var* var);
option<Ast_Type> check_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call, Checker_Proc_Call_Flags flags);

static option<Ast_Type> check_expr(Check_Context* cc, Expr_Context context, Ast_Expr* expr);
static option<Expr_Kind> resolve_expr(Check_Context* cc, Expr_Context context, Ast_Expr* expr);
static option<Ast_Type> check_term(Check_Context* cc, Expr_Context context, Ast_Term* term, Ast_Expr* source_expr);
static option<Ast_Type> check_access(Check_Context* cc, Ast_Type type, option<Ast_Access*> optional_access);
static option<Ast_Type> check_unary_expr(Check_Context* cc, Expr_Context context, Ast_Unary_Expr* unary_expr);
static option<Ast_Type> check_binary_expr(Check_Context* cc, Expr_Context context, Ast_Binary_Expr* binary_expr);
static option<Literal> check_folded_expr(Check_Context* cc, Ast_Expr* expr);
static option<Literal> literal_convert_int_to_uint(Literal lit);
static option<Literal> literal_convert_uint_to_int(Literal lit);
static option<Ast_Type> check_apply_expr_fold(Check_Context* cc, Expr_Context context, Ast_Expr* expr, Literal lit);

void check_consteval_expr(Check_Context* cc, Consteval_Dependency constant);
static Consteval check_consteval_dependencies(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Ast_Expr* expr, option<Expr_Context> context = {});
static Consteval check_consteval_dependencies_array_type(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Ast_Type* type);
static Consteval check_consteval_dependencies_struct_size(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Ast_Decl_Struct* struct_decl);
static option<Tree_Node<Consteval_Dependency>*> consteval_dependency_detect_cycle(Check_Context* cc, Arena* arena, Tree_Node<Consteval_Dependency>* parent, Consteval_Dependency constant);
static Consteval check_evaluate_consteval_tree(Check_Context* cc, Tree_Node<Consteval_Dependency>* node);

Consteval_Dependency consteval_dependency_from_global(Ast_Decl_Global* global_decl, Span span);
Consteval_Dependency consteval_dependency_from_enum_variant(Ast_Enum_Variant* enum_variant, Span span);
Consteval_Dependency consteval_dependency_from_struct_size(Ast_Decl_Struct* struct_decl, Span span);
static Consteval_Dependency consteval_dependency_from_array_size(Ast_Expr* size_expr, Ast_Type* type);
static Consteval_Dependency consteval_dependency_from_array_access(Ast_Access_Array* array_access);
static Consteval consteval_dependency_mark_and_return_invalid(Check_Context* cc, Tree_Node<Consteval_Dependency>* node);
static Ast_Consteval_Expr* consteval_dependency_get_consteval_expr(Consteval_Dependency constant);
static bool match_const_dependency(Consteval_Dependency a, Consteval_Dependency b);
static void consteval_dependency_mark_invalid(Check_Context* cc, Tree_Node<Consteval_Dependency>* node);
static void consteval_dependency_err_context(Check_Context* cc, Tree_Node<Consteval_Dependency>* node);

option<Ast_Info_Struct> find_struct(Ast* target_ast, Ast_Ident ident);
option<Ast_Info_Enum> find_enum(Ast* target_ast, Ast_Ident ident);
option<Ast_Info_Proc> find_proc(Ast* target_ast, Ast_Ident ident);
option<Ast_Info_Global> find_global(Ast* target_ast, Ast_Ident ident);
option<u32> find_enum_variant(Ast_Decl_Enum* enum_decl, Ast_Ident ident);
option<u32> find_struct_field(Ast_Decl_Struct* struct_decl, Ast_Ident ident);

Ast* resolve_import(Check_Context* cc, option<Ast_Ident> import);
void resolve_type(Check_Context* cc, Ast_Type* type, bool check_array_size_expr);
static void resolve_var(Check_Context* cc, Ast_Var* var);
static void resolve_enum(Check_Context* cc, Ast_Enum* _enum);
static void resolve_cast(Check_Context* cc, Expr_Context context, Ast_Cast* cast);
static void resolve_sizeof(Check_Context* cc, Ast_Sizeof* size_of, bool check_array_size_expr);
static void resolve_proc_call(Check_Context* cc, Ast_Proc_Call* proc_call);
static void resolve_array_init(Check_Context* cc, Expr_Context context, Ast_Array_Init* array_init, bool check_array_size_expr);
static void resolve_struct_init(Check_Context* cc, Expr_Context context, Ast_Struct_Init* struct_init);

struct Dependency_Global       { Ast_Decl_Global* global_decl; Span span; };
struct Dependency_Enum_Variant { Ast_Enum_Variant* enum_variant; Span span; };
struct Dependency_Struct_Size  { Ast_Decl_Struct* struct_decl; Span span; };
struct Dependency_Array_Size   { Ast_Expr* size_expr; Ast_Type* type; };
struct Dependency_Array_Access { Ast_Expr* access_expr; };

enum class Consteval_Dependency_Tag
{
	Global,
	Enum_Variant,
	Struct_Size,
	Array_Size_Expr,
	Array_Access_Expr,
};

struct Consteval_Dependency
{
	Consteval_Dependency_Tag tag;

	union
	{
		Dependency_Global as_global;
		Dependency_Enum_Variant as_enum_variant;
		Dependency_Struct_Size as_struct_size;
		Dependency_Array_Size as_array_size;
		Dependency_Array_Access as_array_access;
	};
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
};

enum class Expr_Constness
{
	Normal,
	Const,
};

enum class Expr_Kind
{
	Normal,
	Const,
	Constfold,
};

enum class Type_Kind
{
	Bool,
	Float,
	Int,
	Uint,
	String,
	Pointer,
	Enum,
	Array,
	Struct,
};

enum class Literal_Kind
{
	Bool,
	Float,
	Int,
	UInt
};

#endif
