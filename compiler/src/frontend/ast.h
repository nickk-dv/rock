#ifndef AST_H
#define AST_H

#include "token.h"
#include "general/hashmap.h"
#include "llvm-c/Types.h"

struct Ast_Program;
struct Ast_Struct_IR_Info;
struct Ast_Enum_IR_Info;
struct Ast_Proc_IR_Info;
struct Ast_Global_IR_Info;

struct Ast;
struct Ast_Struct_Info;
struct Ast_Enum_Info;
struct Ast_Proc_Info;
struct Ast_Global_Info;

struct Ast_Ident;
struct Ast_Literal;
struct Ast_Type;
struct Ast_Array_Type;
struct Ast_Struct_Type;
struct Ast_Enum_Type;
struct Ast_Unresolved_Type;

struct Ast_Import_Decl;
struct Ast_Use_Decl;
struct Ast_Struct_Decl;
struct Ast_Struct_Field;
struct Ast_Enum_Decl;
struct Ast_Enum_Variant;
struct Ast_Proc_Decl;
struct Ast_Proc_Param;
struct Ast_Global_Decl;

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

struct Ast_Expr;
struct Ast_Unary_Expr;
struct Ast_Binary_Expr;
struct Ast_Folded_Expr;
enum class Const_Eval;
struct Ast_Const_Expr;
struct Ast_Term;
struct Ast_Var;
struct Ast_Access;
struct Ast_Var_Access;
struct Ast_Array_Access;
struct Ast_Enum;
struct Ast_Sizeof;
struct Ast_Proc_Call;
struct Ast_Array_Init;
struct Ast_Struct_Init;

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
	std::vector<Ast_Global_IR_Info> globals;
};

struct Ast_Struct_IR_Info
{
	Ast_Struct_Decl* struct_decl;
	bool is_sized = false;
	u32 struct_size;
	u32 max_align;
	LLVMTypeRef struct_type;
	LLVMValueRef default_value;
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

struct Ast_Global_IR_Info
{
	Ast_Global_Decl* global_decl;
	LLVMValueRef global_value;
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
	std::vector<Ast_Global_Decl*> globals;
	
	HashMap<Ast_Ident, Ast_Import_Decl*, u32, match_ident> import_table;
	HashMap<Ast_Ident, Ast_Struct_Info, u32, match_ident> struct_table;
	HashMap<Ast_Ident, Ast_Enum_Info, u32, match_ident> enum_table;
	HashMap<Ast_Ident, Ast_Proc_Info, u32, match_ident> proc_table;
	HashMap<Ast_Ident, Ast_Global_Info, u32, match_ident> global_table;
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

struct Ast_Global_Info
{
	u32 global_id;
	Ast_Global_Decl* global_decl;
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

struct Ast_Struct_Type
{
	u32 struct_id;
	Ast_Struct_Decl* struct_decl;
};

struct Ast_Enum_Type
{
	u32 enum_id;
	Ast_Enum_Decl* enum_decl;
};

enum class Ast_Type_Tag
{
	Basic, 
	Array, 
	Struct, 
	Enum, 
	Unresolved, 
	Poison,
};

struct Ast_Type
{
	Span span;
	Ast_Type_Tag tag;
	u32 pointer_level = 0;

	union
	{
		BasicType as_basic;
		Ast_Array_Type* as_array;
		Ast_Struct_Type as_struct;
		Ast_Enum_Type as_enum;
		Ast_Unresolved_Type* as_unresolved;
	};
};

struct Ast_Array_Type
{
	Ast_Type element_type;
	Ast_Const_Expr* const_expr;
};

struct Ast_Unresolved_Type
{
	option<Ast_Ident> import;
	Ast_Ident ident;
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
	std::vector<Ast_Struct_Field> fields;
	Const_Eval size_eval;
	u64 size;
};

struct Ast_Struct_Field
{
	Ast_Ident ident;
	Ast_Type type;
	option<Ast_Const_Expr*> const_expr;
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
	Ast_Const_Expr* const_expr;
	//ir builder
	LLVMValueRef constant;
};

struct Ast_Proc_Decl
{
	Ast_Ident ident;
	std::vector<Ast_Proc_Param> input_params;
	option<Ast_Type> return_type;
	Ast_Block* block;
	bool is_main;
	bool is_external;
	bool is_variadic;
};

struct Ast_Proc_Param
{
	Ast_Ident ident;
	Ast_Type type;
};

struct Ast_Global_Decl
{
	Ast_Ident ident;
	Ast_Const_Expr* const_expr;
	option<Ast_Type> type;
};

struct Ast_Block
{
	std::vector<Ast_Statement*> statements;
};

enum class Ast_Statement_Tag
{
	If, 
	For, 
	Block, 
	Defer, 
	Break, 
	Return,
	Switch, 
	Continue, 
	Var_Decl, 
	Var_Assign, 
	Proc_Call,
};

struct Ast_Statement
{
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
	Span span;
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
	Span span;
	Ast_Else_Tag tag;

	union
	{
		Ast_If* as_if;
		Ast_Block* as_block;
	};
};

struct Ast_For
{
	Span span;
	option<Ast_Var_Decl*> var_decl;
	option<Ast_Expr*> condition_expr;
	option<Ast_Var_Assign*> var_assign;
	Ast_Block* block;
};

struct Ast_Defer
{
	Span span;
	Ast_Block* block;
};

struct Ast_Break
{
	Span span;
};

struct Ast_Return
{
	Span span;
	option<Ast_Expr*> expr;
};

struct Ast_Switch
{
	Span span;
	Ast_Expr* expr;
	std::vector<Ast_Switch_Case> cases;
};

struct Ast_Switch_Case
{
	Ast_Const_Expr* const_expr;
	option<Ast_Block*> block;
	//ir builder
	LLVMBasicBlockRef basic_block;
};

struct Ast_Continue
{
	Span span;
};

struct Ast_Var_Decl
{
	Span span;
	Ast_Ident ident;
	option<Ast_Type> type;
	option<Ast_Expr*> expr;
};

struct Ast_Var_Assign
{
	Span span;
	Ast_Var* var;
	AssignOp op;
	Ast_Expr* expr;
};

struct Ast_Folded_Expr
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
	Term, 
	Unary_Expr, 
	Binary_Expr, 
	Folded_Expr,
};

struct Ast_Expr
{
	Span span;
	Ast_Expr_Tag tag;
	bool is_const;

	union
	{
		Ast_Term* as_term;
		Ast_Unary_Expr* as_unary_expr;
		Ast_Binary_Expr* as_binary_expr;
		Ast_Folded_Expr as_folded_expr;
	};
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

enum class Const_Eval
{
	Not_Evaluated = 0,
	Invalid = 1,
	Valid = 2,
};

struct Ast_Const_Expr
{
	Ast_Expr* expr;
	Const_Eval eval;
};

enum class Ast_Term_Tag
{
	Var,
	Enum,
	Sizeof,
	Literal,
	Proc_Call,
	Array_Init,
	Struct_Init,
};

struct Ast_Term
{
	Ast_Term_Tag tag;

	union
	{
		Ast_Var* as_var;
		Ast_Enum* as_enum;
		Ast_Sizeof* as_sizeof;
		Ast_Literal* as_literal;
		Ast_Proc_Call* as_proc_call;
		Ast_Array_Init* as_array_init;
		Ast_Struct_Init* as_struct_init;
	};
};

enum class Ast_Var_Tag
{
	Unresolved, 
	Local,
	Global,
	Invalid,
};

struct Ast_Var
{
	Ast_Var_Tag tag;
	option<Ast_Access*> access;

	union
	{
		struct Unresolved 
		{ 
			Ast_Ident ident; 
		} unresolved;

		struct Local 
		{ 
			Ast_Ident ident; 
		} local;

		struct Global 
		{ 
			u32 global_id;
			Ast_Global_Decl* global_decl; 
		} global;
	};
};

enum class Ast_Access_Tag
{
	Var, 
	Array,
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

enum class Ast_Enum_Tag
{
	Unresolved, 
	Resolved, 
	Invalid,
};

struct Ast_Enum
{
	Ast_Enum_Tag tag;
	
	union
	{
		struct Unresolved
		{
			option<Ast_Ident> import;
			Ast_Ident ident;
			Ast_Ident variant;
		} unresolved;

		struct Resolved
		{
			Ast_Enum_Type type;
			u32 variant_id;
		} resolved;
	};
};

struct Ast_Sizeof
{
	Ast_Type type;
};

enum class Ast_Proc_Call_Tag
{
	Unresolved,
	Resolved,
	Invalid,
};

struct Ast_Proc_Call
{
	Span span;
	Ast_Proc_Call_Tag tag;
	option<Ast_Access*> access;
	std::vector<Ast_Expr*> input_exprs;

	union
	{
		struct Unresolved
		{
			option<Ast_Ident> import;
			Ast_Ident ident;
		} unresolved;

		struct Resolved
		{
			Ast_Proc_Decl* proc_decl;
			u32 proc_id;
		} resolved;
	};
};

struct Ast_Array_Init
{
	option<Ast_Type> type;
	std::vector<Ast_Expr*> input_exprs;
};

enum class Ast_Struct_Init_Tag
{
	Unresolved,
	Resolved,
	Invalid,
};

struct Ast_Struct_Init
{
	Ast_Struct_Init_Tag tag;
	std::vector<Ast_Expr*> input_exprs;

	union
	{
		struct Unresolved
		{
			option<Ast_Ident> import;
			option<Ast_Ident> ident;
		} unresolved;

		struct Resolved
		{
			option<Ast_Struct_Type> type;
		} resolved;
	};
};

#endif
