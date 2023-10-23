#ifndef AST_H
#define AST_H

#include "common.h"
#include "token.h"
#include "llvm-c/Types.h"

struct Ast_Program;
struct Ast_Struct_IR_Info;
struct Ast_Enum_IR_Info;
struct Ast_Proc_IR_Info;

struct Ast;
struct Ast_Struct_Info;
struct Ast_Enum_Info;
struct Ast_Proc_Info;

struct Ast_Ident;
struct Ast_Literal;
struct Ast_Type;
struct Ast_Array_Type;
struct Ast_Custom_Type;
struct Ast_Struct_Type;
struct Ast_Enum_Type;
struct Ast_Ident_Type_Pair;

struct Ast_Import_Decl;
struct Ast_Use_Decl;
struct Ast_Struct_Decl;
struct Ast_Enum_Decl;
struct Ast_Enum_Variant;
struct Ast_Proc_Decl;

struct Ast_Block;
struct Ast_Statement;
struct Ast_If;
struct Ast_Else;
struct Ast_For;
struct Ast_Defer;
struct Ast_Break;
struct Ast_Return;
struct Ast_Switch;
struct Ast_Switch_Case;
struct Ast_Continue;
struct Ast_Var_Decl;
struct Ast_Var_Assign;
struct Ast_Proc_Call;

struct Ast_Expr;
struct Ast_Term;
struct Ast_Var;
struct Ast_Access;
struct Ast_Var_Access;
struct Ast_Array_Access;
struct Ast_Enum;
struct Ast_Sizeof;
struct Ast_Struct_Init;
struct Ast_Unary_Expr;
struct Ast_Binary_Expr;
struct Ast_Const_Expr;

Ast_Ident token_to_ident(const Token& token);
u32 hash_ident(Ast_Ident& ident);
bool match_ident(Ast_Ident& a, Ast_Ident& b);
bool match_string(std::string& a, std::string& b);

struct Ast_Program
{
	std::vector<Ast*> modules;
	HashMap<std::string, Ast*, u32, match_string> module_map;
	std::vector<Ast_Struct_IR_Info> structs;
	std::vector<Ast_Enum_IR_Info> enums;
	std::vector<Ast_Proc_IR_Info> procs;
};

struct Ast_Struct_IR_Info
{
	Ast_Struct_Decl* struct_decl;
	LLVMTypeRef struct_type;
};

struct Ast_Enum_IR_Info
{
	Ast_Enum_Decl* enum_decl;
	LLVMTypeRef enum_type;
};

struct Ast_Proc_IR_Info
{
	Ast_Proc_Decl* proc_decl;
	LLVMTypeRef proc_type;
	LLVMValueRef proc_value;
};

struct Ast
{
	StringView source;
	std::string filepath;

	std::vector<Ast_Import_Decl*> imports;
	std::vector<Ast_Use_Decl*> uses;
	std::vector<Ast_Struct_Decl*> structs;
	std::vector<Ast_Enum_Decl*> enums;
	std::vector<Ast_Proc_Decl*> procs;
	
	//checker
	HashMap<Ast_Ident, Ast_Import_Decl*, u32, match_ident> import_table;
	HashMap<Ast_Ident, Ast_Struct_Info, u32, match_ident> struct_table;
	HashMap<Ast_Ident, Ast_Enum_Info, u32, match_ident> enum_table;
	HashMap<Ast_Ident, Ast_Proc_Info, u32, match_ident> proc_table;
};

struct Ast_Struct_Info
{
	u32 struct_id;
	Ast_Struct_Decl* struct_decl;
};

struct Ast_Enum_Info
{
	u32 enum_id;
	Ast_Enum_Decl* enum_decl;
};

struct Ast_Proc_Info
{
	u32 proc_id;
	Ast_Proc_Decl* proc_decl;
};

struct Ast_Ident
{
	Span span;
	StringView str;
};

struct Ast_Literal
{
	Token token;
};

struct Ast_Struct_Type  //@Memory can remove decl and use program id lookup in checker
{
	u32 struct_id;
	Ast_Struct_Decl* struct_decl;
};

struct Ast_Enum_Type //@Memory can remove decl and use program id lookup in checker
{
	u32 enum_id;
	Ast_Enum_Decl* enum_decl;
};

enum class Ast_Type_Tag
{
	Basic, Array, Custom, Struct, Enum
};

struct Ast_Type
{
	Ast_Type_Tag tag;

	union
	{
		BasicType as_basic;
		Ast_Array_Type* as_array;
		Ast_Custom_Type* as_custom;
		//checker
		Ast_Struct_Type as_struct;
		Ast_Enum_Type as_enum;
	};

	u32 pointer_level = 0;
};

struct Ast_Custom_Type
{
	option<Ast_Ident> import;
	Ast_Ident ident;
};

struct Ast_Array_Type
{
	Ast_Type element_type;
	bool is_dynamic;
	u64 fixed_size;
};

struct Ast_Ident_Type_Pair
{
	Ast_Ident ident;
	Ast_Type type;
};

struct Ast_Ident_Literal_Pair
{
	Ast_Ident ident;
	Ast_Literal literal;
	bool is_negative;
	//ir builder
	LLVMValueRef constant;
};

struct Ast_Import_Decl
{
	Ast_Ident alias;
	Ast_Literal file_path;
	//checker
	Ast* import_ast;
};

struct Ast_Use_Decl
{
	Ast_Ident alias;
	Ast_Ident import;
	Ast_Ident symbol;
};

struct Ast_Struct_Decl
{
	Ast_Ident ident;
	std::vector<Ast_Ident_Type_Pair> fields;
};

struct Ast_Enum_Decl
{
	Ast_Ident ident;
	BasicType basic_type;
	std::vector<Ast_Enum_Variant> variants;
};

struct Ast_Enum_Variant
{
	Ast_Ident ident;
	Ast_Expr* const_expr;
	//ir builder
	LLVMValueRef constant;
};

struct Ast_Proc_Decl
{
	Ast_Ident ident;
	std::vector<Ast_Ident_Type_Pair> input_params;
	option<Ast_Type> return_type;
	Ast_Block* block;
	bool is_external;
	bool is_main; //@Todo use flags or enum kinds if types cant overlap
	bool is_variadic;
};

struct Ast_Block
{
	std::vector<Ast_Statement*> statements;
};

enum class Ast_Statement_Tag
{
	If, For, Block, Defer, Break, Return,
	Switch, Continue, Var_Decl, Var_Assign, Proc_Call
};

struct Ast_Statement
{
	Span span;
	Ast_Statement_Tag tag;

	union
	{
		Ast_If* as_if;
		Ast_For* as_for;
		Ast_Block* as_block;
		Ast_Defer* as_defer;
		Ast_Break* as_break;
		Ast_Return* as_return;
		Ast_Switch* as_switch;
		Ast_Continue* as_continue;
		Ast_Var_Decl* as_var_decl;
		Ast_Var_Assign* as_var_assign;
		Ast_Proc_Call* as_proc_call;
	};
};

struct Ast_If
{
	Token token;
	Ast_Expr* condition_expr;
	Ast_Block* block;
	option<Ast_Else*> _else;
};

enum class Ast_Else_Tag
{
	If, Block
};

struct Ast_Else
{
	Token token;
	Ast_Else_Tag tag;

	union
	{
		Ast_If* as_if;
		Ast_Block* as_block;
	};
};

struct Ast_For
{
	Token token;
	option<Ast_Var_Decl*> var_decl;
	option<Ast_Expr*> condition_expr;
	option<Ast_Var_Assign*> var_assign;
	Ast_Block* block;
};

struct Ast_Defer
{
	Token token;
	Ast_Block* block;
};

struct Ast_Break
{
	Token token;
};

struct Ast_Return
{
	Token token;
	option<Ast_Expr*> expr;
};

struct Ast_Switch
{
	Token token;
	Ast_Expr* expr;
	std::vector<Ast_Switch_Case> cases;
};

struct Ast_Switch_Case
{
	Ast_Expr* const_expr;
	option<Ast_Block*> block;
	//ir builder
	LLVMBasicBlockRef basic_block;
};

struct Ast_Continue
{
	Token token;
};

struct Ast_Var_Decl
{
	Ast_Ident ident;
	option<Ast_Type> type;
	option<Ast_Expr*> expr;
};

struct Ast_Var_Assign
{
	Ast_Var* var;
	AssignOp op;
	Ast_Expr* expr;
};

struct Ast_Proc_Call
{
	option<Ast_Ident> import;
	Ast_Ident ident;
	std::vector<Ast_Expr*> input_exprs;
	option<Ast_Access*> access;
	//checker
	u32 proc_id;
};

struct Ast_Const_Expr
{
	BasicType basic_type;

	union
	{
		bool as_bool;
		f64 as_f64;
		i64 as_i64;
		u64 as_u64;
	};
};

enum class Ast_Expr_Tag
{
	Term, Unary_Expr, Binary_Expr, Const_Expr
};

struct Ast_Expr
{
	Span span;
	Ast_Expr_Tag tag;

	union
	{
		Ast_Term* as_term;
		Ast_Unary_Expr* as_unary_expr;
		Ast_Binary_Expr* as_binary_expr;
		Ast_Const_Expr as_const_expr; //@Notice span doesnt change after const fold
	};
};

enum class Ast_Term_Tag
{
	Var, Enum, Sizeof, Literal,
	Proc_Call, Struct_Init
};

struct Ast_Term
{
	Ast_Term_Tag tag;

	union
	{
		Ast_Var* as_var;
		Ast_Enum* as_enum;
		Ast_Sizeof* as_sizeof;
		Ast_Literal as_literal; //@Pointer todo to squish size
		Ast_Proc_Call* as_proc_call;
		Ast_Struct_Init* as_struct_init;
	};
};

struct Ast_Var
{
	Ast_Ident ident;
	option<Ast_Access*> access;
};

enum class Ast_Access_Tag
{
	Var, Array
};

struct Ast_Access
{
	Ast_Access_Tag tag;

	union
	{
		Ast_Var_Access* as_var;
		Ast_Array_Access* as_array;
	};
};

struct Ast_Var_Access
{
	Ast_Ident ident;
	option<Ast_Access*> next;
	//checker
	u32 field_id;
};

struct Ast_Array_Access
{
	Ast_Expr* index_expr;
	option<Ast_Access*> next;
};

struct Ast_Enum
{
	option<Ast_Ident> import;
	Ast_Ident ident;
	Ast_Ident variant;
	//checker
	u32 enum_id;
	u32 variant_id;
};

struct Ast_Sizeof
{
	Ast_Type type;
};

struct Ast_Struct_Init
{
	option<Ast_Ident> import;
	option<Ast_Ident> ident;
	std::vector<Ast_Expr*> input_exprs;
	//checker
	u32 struct_id;
};

struct Ast_Unary_Expr
{
	UnaryOp op;
	Ast_Expr* right;
};

struct Ast_Binary_Expr
{
	BinaryOp op;
	Ast_Expr* left;
	Ast_Expr* right;
};

#endif
