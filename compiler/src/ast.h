#ifndef AST_H
#define AST_H

#include "common.h"
#include "token.h"
#include "llvm-c/Types.h"

struct Ast_Program;
struct Ast_Struct_Meta;
struct Ast_Enum_Meta;
struct Ast_Proc_Meta;

struct Ast;
struct Ast_Struct_Decl_Meta;
struct Ast_Enum_Decl_Meta;
struct Ast_Proc_Decl_Meta;

struct Ast_Ident;
struct Ast_Literal;
struct Ast_Type;
struct Ast_Array_Type;
struct Ast_Custom_Type;
struct Ast_Struct_Type;
struct Ast_Enum_Type;
struct Ast_Ident_Type_Pair;
struct Ast_Ident_Literal_Pair;

struct Ast_Import_Decl;
struct Ast_Use_Decl;
struct Ast_Struct_Decl;
struct Ast_Enum_Decl;
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
struct Ast_Struct_Init;
struct Ast_Unary_Expr;
struct Ast_Binary_Expr;

Ast_Ident token_to_ident(const Token& token);
u32 hash_ident(Ast_Ident& ident);
bool match_ident(Ast_Ident& a, Ast_Ident& b);

struct Ast_Program
{
	std::vector<Ast_Struct_Meta> structs;
	std::vector<Ast_Enum_Meta> enums;
	std::vector<Ast_Proc_Meta> procedures;
};

struct Ast_Struct_Meta
{
	Ast_Struct_Decl* struct_decl;
	LLVMTypeRef struct_type;
};

struct Ast_Enum_Meta
{
	Ast_Enum_Decl* enum_decl;
	LLVMTypeRef enum_type;
};

struct Ast_Proc_Meta
{
	Ast_Proc_Decl* proc_decl;
	LLVMTypeRef proc_type;
	LLVMValueRef proc_value;
};

struct Ast
{
	std::vector<Ast_Import_Decl*> imports;
	std::vector<Ast_Use_Decl*> uses;
	std::vector<Ast_Struct_Decl*> structs;
	std::vector<Ast_Enum_Decl*> enums;
	std::vector<Ast_Proc_Decl*> procs;
	//checker
	HashMap<Ast_Ident, Ast_Import_Decl*, u32, match_ident> import_table;
	HashMap<Ast_Ident, Ast_Struct_Decl_Meta, u32, match_ident> struct_table;
	HashMap<Ast_Ident, Ast_Enum_Decl_Meta, u32, match_ident> enum_table;
	HashMap<Ast_Ident, Ast_Proc_Decl_Meta, u32, match_ident> proc_table;
};

struct Ast_Struct_Decl_Meta
{
	u32 struct_id;
	Ast_Struct_Decl* struct_decl;
};

struct Ast_Enum_Decl_Meta
{
	u32 enum_id;
	Ast_Enum_Decl* enum_decl;
};

struct Ast_Proc_Decl_Meta
{
	u32 proc_id;
	Ast_Proc_Decl* proc_decl;
};

struct Ast_Ident
{
	u32 l0 = 0;
	u32 c0 = 0;
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

struct Ast_Type
{
	enum class Tag
	{
		Basic, Array, Custom, Struct, Enum
	} tag;

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
	std::optional<Ast_Ident> import;
	Ast_Ident type;
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
	Ast_Ident type;
	std::vector<Ast_Ident_Type_Pair> fields;
};

struct Ast_Enum_Decl
{
	Ast_Ident type;
	std::optional<BasicType> basic_type;
	std::vector<Ast_Ident_Literal_Pair> variants;
};

struct Ast_Proc_Decl
{
	Ast_Ident ident;
	std::vector<Ast_Ident_Type_Pair> input_params;
	std::optional<Ast_Type> return_type;
	Ast_Block* block;
	bool is_external;
	bool is_main; //@Todo use flags or enum kinds if types cant overlap
};

struct Ast_Block
{
	std::vector<Ast_Statement*> statements;
};

struct Ast_Statement
{
	enum class Tag
	{
		If, For, Block, Defer, Break, Return,
		Switch, Continue, Var_Decl, Var_Assign, Proc_Call
	} tag;

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
	std::optional<Ast_Else*> _else;
};

struct Ast_Else
{
	Token token;

	enum class Tag
	{
		If, Block
	} tag;

	union
	{
		Ast_If* as_if;
		Ast_Block* as_block;
	};
};

struct Ast_For
{
	Token token;
	std::optional<Ast_Var_Decl*> var_decl;
	std::optional<Ast_Expr*> condition_expr;
	std::optional<Ast_Var_Assign*> var_assign;
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
	std::optional<Ast_Expr*> expr;
};

struct Ast_Switch
{
	Token token;
	Ast_Term* term;
	std::vector<Ast_Switch_Case> cases;
	//checker
	Ast_Type type;
};

struct Ast_Switch_Case
{
	Ast_Term* term;
	std::optional<Ast_Block*> block;
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
	std::optional<Ast_Type> type;
	std::optional<Ast_Expr*> expr;
};

struct Ast_Var_Assign
{
	Ast_Var* var;
	AssignOp op;
	Ast_Expr* expr;
};

struct Ast_Proc_Call
{
	std::optional<Ast_Ident> import;
	Ast_Ident ident;
	std::vector<Ast_Expr*> input_exprs;
	std::optional<Ast_Access*> access;
	//checker
	u32 proc_id;
};

struct Ast_Expr
{
	enum class Tag
	{
		Term, Unary_Expr, Binary_Expr
	} tag;

	union
	{
		Ast_Term* as_term;
		Ast_Unary_Expr* as_unary_expr;
		Ast_Binary_Expr* as_binary_expr;
	};
};

struct Ast_Term
{
	enum class Tag
	{
		Var, Enum, Literal, Proc_Call, Struct_Init
	} tag;

	union
	{
		Ast_Var* as_var;
		Ast_Enum* as_enum;
		Ast_Literal as_literal;
		Ast_Proc_Call* as_proc_call;
		Ast_Struct_Init* as_struct_init;
	};
};

struct Ast_Var
{
	Ast_Ident ident;
	std::optional<Ast_Access*> access;
};

struct Ast_Access
{
	enum class Tag
	{
		Var, Array
	} tag;

	union
	{
		Ast_Var_Access* as_var;
		Ast_Array_Access* as_array;
	};
};

struct Ast_Var_Access
{
	Ast_Ident ident;
	std::optional<Ast_Access*> next;
	//checker
	u32 field_id;
};

struct Ast_Array_Access
{
	Ast_Expr* index_expr;
	std::optional<Ast_Access*> next;
};

struct Ast_Enum
{
	std::optional<Ast_Ident> import;
	Ast_Ident type;
	Ast_Ident variant;
	//checker
	u32 enum_id;
	u32 variant_id;
};

struct Ast_Struct_Init
{
	std::optional<Ast_Ident> import;
	std::optional<Ast_Ident> type;
	std::vector<Ast_Expr*> input_exprs;
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
