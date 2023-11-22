#ifndef AST_H
#define AST_H

#include "token.h"
#include "general/hashmap.h"
#include "llvm-c/Types.h"

struct Ast;
struct Ast_Program;
struct Ast_Ident;
struct Ast_Literal;

struct Ast_Info_Proc;
struct Ast_Info_Enum;
struct Ast_Info_Struct;
struct Ast_Info_Global;
struct Ast_Info_IR_Proc;
struct Ast_Info_IR_Enum;
struct Ast_Info_IR_Struct;
struct Ast_Info_IR_Global;

struct Ast_Type;
struct Ast_Type_Enum;
struct Ast_Type_Array;
struct Ast_Type_Struct;
struct Ast_Type_Procedure;
struct Ast_Type_Unresolved;

struct Ast_Decl_Impl;
struct Ast_Decl_Use;
struct Ast_Decl_Proc;
struct Ast_Proc_Param;
struct Ast_Decl_Enum;
struct Ast_Enum_Variant;
struct Ast_Decl_Struct;
struct Ast_Struct_Field;
struct Ast_Decl_Global;
struct Ast_Decl_Import;

struct Ast_Stmt;
struct Ast_Stmt_If;
struct Ast_Else;
struct Ast_Stmt_For;
struct Ast_Stmt_Block;
struct Ast_Stmt_Defer;
struct Ast_Stmt_Break;
struct Ast_Stmt_Return;
struct Ast_Stmt_Switch;
struct Ast_Switch_Case;
struct Ast_Stmt_Continue;
struct Ast_Stmt_Var_Decl;
struct Ast_Stmt_Var_Assign;

struct Ast_Expr;
struct Ast_Unary_Expr;
struct Ast_Binary_Expr;
struct Ast_Folded_Expr;
enum class Consteval;
struct Ast_Consteval_Expr;
struct Ast_Term;
struct Ast_Var;
struct Ast_Access;
struct Ast_Access_Var;
struct Ast_Access_Array;
struct Ast_Enum;
struct Ast_Cast;
struct Ast_Sizeof;
struct Ast_Proc_Call;
struct Ast_Array_Init;
struct Ast_Struct_Init;

//@new syntax
struct Ast_Module_Access;
struct Ast_Decl_Import_New;

Ast_Ident token_to_ident(const Token& token);
u32 hash_ident(Ast_Ident& ident);
bool match_ident(Ast_Ident& a, Ast_Ident& b);
bool match_string(std::string& a, std::string& b);

struct Ast
{
	StringView source;
	std::string filepath;
	std::vector<Span> line_spans;

	std::vector<Ast_Decl_Impl*> impls;
	std::vector<Ast_Decl_Use*> uses;
	std::vector<Ast_Decl_Proc*> procs;
	std::vector<Ast_Decl_Enum*> enums;
	std::vector<Ast_Decl_Struct*> structs;
	std::vector<Ast_Decl_Global*> globals;
	std::vector<Ast_Decl_Import*> imports;

	HashMap<Ast_Ident, Ast_Info_Proc, u32, match_ident> proc_table;
	HashMap<Ast_Ident, Ast_Info_Enum, u32, match_ident> enum_table;
	HashMap<Ast_Ident, Ast_Info_Struct, u32, match_ident> struct_table;
	HashMap<Ast_Ident, Ast_Info_Global, u32, match_ident> global_table;
	HashMap<Ast_Ident, Ast_Decl_Import*, u32, match_ident> import_table;
};

struct Ast_Program
{
	std::vector<Ast*> modules;
	HashMap<std::string, Ast*, u32, match_string> module_map;
	HashMap<Ast_Ident, Ast_Decl_Proc*, u32, match_ident> external_proc_table;
	
	std::vector<Ast_Info_IR_Proc> procs;
	std::vector<Ast_Info_IR_Enum> enums;
	std::vector<Ast_Info_IR_Struct> structs;
	std::vector<Ast_Info_IR_Global> globals;
};

struct Ast_Info_Proc      { Ast_Decl_Proc* proc_decl; u32 proc_id; };
struct Ast_Info_Enum      { Ast_Decl_Enum* enum_decl; u32 enum_id; };
struct Ast_Info_Struct    { Ast_Decl_Struct* struct_decl; u32 struct_id; };
struct Ast_Info_Global    { Ast_Decl_Global* global_decl; u32 global_id; };
struct Ast_Info_IR_Proc   { Ast_Decl_Proc* proc_decl; LLVMTypeRef proc_type; LLVMValueRef proc_value; };
struct Ast_Info_IR_Enum   { Ast_Decl_Enum* enum_decl; LLVMTypeRef enum_type; };
struct Ast_Info_IR_Struct { Ast_Decl_Struct* struct_decl; LLVMTypeRef struct_type; LLVMValueRef default_value; };
struct Ast_Info_IR_Global { Ast_Decl_Global* global_decl; LLVMValueRef global_ptr; LLVMValueRef const_value; };

struct Ast_Ident
{
	Span span;
	StringView str;
};

struct Ast_Literal
{
	Token token;
};

struct Ast_Type_Enum
{
	u32 enum_id;
	Ast_Decl_Enum* enum_decl;
};

struct Ast_Type_Struct
{
	u32 struct_id;
	Ast_Decl_Struct* struct_decl;
};

enum class Ast_Type_Tag
{
	Basic, Enum, Struct,
	Array, Procedure,
	Unresolved, Poison,
};

struct Ast_Type
{
	Span span;
	Ast_Type_Tag tag;
	u32 pointer_level = 0;

	union
	{
		BasicType as_basic;
		Ast_Type_Enum as_enum;
		Ast_Type_Struct as_struct;
		Ast_Type_Array* as_array;
		Ast_Type_Procedure* as_procedure;
		Ast_Type_Unresolved* as_unresolved;
	};
};

struct Ast_Type_Array
{
	Ast_Type element_type;
	Ast_Expr* size_expr;
};

struct Ast_Type_Procedure
{
	std::vector<Ast_Type> input_types;
	option<Ast_Type> return_type;
};

struct Ast_Type_Unresolved
{
	option<Ast_Ident> import;
	Ast_Ident ident;
};

struct Ast_Decl_Impl
{
	Ast_Type type;
	std::vector<Ast_Decl_Proc*> member_procedures;
};

struct Ast_Decl_Use
{
	Ast_Ident alias;
	Ast_Ident import;
	Ast_Ident symbol;
};

struct Ast_Decl_Proc
{
	Ast_Ident ident;
	std::vector<Ast_Proc_Param> input_params;
	option<Ast_Type> return_type;
	Ast_Stmt_Block* block;
	bool is_member;
	bool is_main;
	bool is_external;
	bool is_variadic;
};

struct Ast_Proc_Param
{
	Ast_Ident ident;
	Ast_Type type;
	bool self;
};

struct Ast_Decl_Enum
{
	Ast_Ident ident;
	BasicType basic_type;
	std::vector<Ast_Enum_Variant> variants;
};

struct Ast_Enum_Variant
{
	Ast_Ident ident;
	Ast_Consteval_Expr* consteval_expr;
	//ir builder
	LLVMValueRef constant;
};

struct Ast_Decl_Struct
{
	Ast_Ident ident;
	std::vector<Ast_Struct_Field> fields;
	Consteval size_eval;
	u32 struct_size;
	u32 max_align;
};

struct Ast_Struct_Field
{
	Ast_Ident ident;
	Ast_Type type;
	option<Ast_Expr*> default_expr;
};

struct Ast_Decl_Global
{
	Ast_Ident ident;
	Ast_Consteval_Expr* consteval_expr;
	option<Ast_Type> type;
};

struct Ast_Decl_Import
{
	Ast_Ident alias;
	Ast_Literal file_path;
	//checker
	Ast* import_ast;
};

enum class Ast_Stmt_Tag
{
	If, For, Block,
	Defer, Break, Return,
	Switch, Continue, Var_Decl,
	Var_Assign, Proc_Call,
};

struct Ast_Stmt
{
	Ast_Stmt_Tag tag;

	union
	{
		Ast_Stmt_If* as_if;
		Ast_Stmt_For* as_for;
		Ast_Stmt_Block* as_block;
		Ast_Stmt_Defer* as_defer;
		Ast_Stmt_Break* as_break;
		Ast_Stmt_Return* as_return;
		Ast_Stmt_Switch* as_switch;
		Ast_Stmt_Continue* as_continue;
		Ast_Stmt_Var_Decl* as_var_decl;
		Ast_Stmt_Var_Assign* as_var_assign;
		Ast_Proc_Call* as_proc_call;
	};
};

struct Ast_Stmt_If
{
	Span span;
	Ast_Expr* condition_expr;
	Ast_Stmt_Block* block;
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
		Ast_Stmt_If* as_if;
		Ast_Stmt_Block* as_block;
	};
};

struct Ast_Stmt_For
{
	Span span;
	option<Ast_Stmt_Var_Decl*> var_decl;
	option<Ast_Expr*> condition_expr;
	option<Ast_Stmt_Var_Assign*> var_assign;
	Ast_Stmt_Block* block;
};

struct Ast_Stmt_Block
{
	std::vector<Ast_Stmt*> statements;
};

struct Ast_Stmt_Defer
{
	Span span;
	Ast_Stmt_Block* block;
};

struct Ast_Stmt_Break
{
	Span span;
};

struct Ast_Stmt_Return
{
	Span span;
	option<Ast_Expr*> expr;
};

struct Ast_Stmt_Switch
{
	Span span;
	Ast_Expr* expr;
	std::vector<Ast_Switch_Case> cases;
};

struct Ast_Switch_Case
{
	Ast_Expr* case_expr;
	option<Ast_Stmt_Block*> block;
	//ir builder
	LLVMBasicBlockRef basic_block;
};

struct Ast_Stmt_Continue
{
	Span span;
};

struct Ast_Stmt_Var_Decl
{
	Span span;
	Ast_Ident ident;
	option<Ast_Type> type;
	option<Ast_Expr*> expr;
};

struct Ast_Stmt_Var_Assign
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

enum Ast_Expr_Flags
{
	AST_EXPR_FLAG_CONST_BIT               = 1 << 0,
	AST_EXPR_FLAG_AUTO_CAST_F32_F64_BIT   = 1 << 1,
	AST_EXPR_FLAG_AUTO_CAST_F64_F32_BIT   = 1 << 2,
	AST_EXPR_FLAG_AUTO_CAST_INT_SEXT_BIT  = 1 << 3,
	AST_EXPR_FLAG_AUTO_CAST_UINT_ZEXT_BIT = 1 << 4,
	AST_EXPR_FLAG_AUTO_CAST_TO_INT_16_BIT = 1 << 5,
	AST_EXPR_FLAG_AUTO_CAST_TO_INT_32_BIT = 1 << 6,
	AST_EXPR_FLAG_AUTO_CAST_TO_INT_64_BIT = 1 << 7,
	AST_EXPR_FLAG_BIN_OP_INT_SIGNED       = 1 << 8,
};

enum class Ast_Expr_Tag
{
	Term,
	Unary,
	Binary,
	Folded,
};

struct Ast_Expr
{
	Span span;
	Ast_Expr_Tag tag;
	u16 flags;

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

enum class Consteval
{
	Not_Evaluated = 0,
	Invalid = 1,
	Valid = 2,
};

struct Ast_Consteval_Expr
{
	Ast_Expr* expr;
	Consteval eval;
};

enum class Ast_Term_Tag
{
	Var, Enum, Cast, Sizeof, Literal,
	Proc_Call, Array_Init, Struct_Init,
};

struct Ast_Term
{
	Ast_Term_Tag tag;

	union
	{
		Ast_Var* as_var;
		Ast_Enum* as_enum;
		Ast_Cast* as_cast;
		Ast_Sizeof* as_sizeof;
		Ast_Literal* as_literal;
		Ast_Proc_Call* as_proc_call;
		Ast_Array_Init* as_array_init;
		Ast_Struct_Init* as_struct_init;
	};
};

enum class Ast_Resolve_Tag
{
	Unresolved,
	Resolved,
	Invalid,
};

enum class Ast_Resolve_Var_Tag
{
	Unresolved, 
	Resolved_Local,
	Resolved_Global,
	Invalid,
};

struct Ast_Var
{
	Ast_Resolve_Var_Tag tag;
	option<Ast_Access*> access;

	union
	{
		struct Unresolved      { Ast_Ident ident; } unresolved;
		struct Resolved_Local  { Ast_Ident ident; } local;
		struct Resolved_Global { u32 global_id; Ast_Decl_Global* global_decl; } global;
	};
};

enum class Ast_Access_Tag
{
	Var, Array,
};

struct Ast_Access
{
	Ast_Access_Tag tag;
	option<Ast_Access*> next;

	union
	{
		Ast_Access_Var* as_var;
		Ast_Access_Array* as_array;
	};
};

struct Ast_Access_Var
{
	Ast_Resolve_Tag tag;
	
	union
	{
		struct Unresolved { Ast_Ident ident; } unresolved;
		struct Resolved   { u32 field_id; } resolved;
	};
};

struct Ast_Access_Array
{
	Ast_Expr* index_expr;
};

struct Ast_Enum
{
	Ast_Resolve_Tag tag;
	
	union
	{
		struct Unresolved { option<Ast_Ident> import; Ast_Ident ident; Ast_Ident variant; } unresolved;
		struct Resolved   { Ast_Type_Enum type; u32 variant_id; } resolved;
	};
};

enum class Ast_Resolve_Cast_Tag
{
	Unresolved,
	Invalid,
	Integer_No_Op,
	Int_Trunc____LLVMTrunc,
	Uint_Extend__LLVMZExt,
	Int_Extend___LLVMSExt,
	Float_Uint___LLVMFPToUI,
	Float_Int____LLVMFPToSI,
	Uint_Float___LLVMUIToFP,
	Int_Float____LLVMSIToFP,
	Float_Trunc__LLVMFPTrunc,
	Float_Extend_LLVMFPExt,
};

struct Ast_Cast
{
	Ast_Resolve_Cast_Tag tag;
	BasicType basic_type;
	Ast_Expr* expr;
};

struct Ast_Sizeof
{
	Ast_Resolve_Tag tag;
	Ast_Type type;
};

struct Ast_Proc_Call
{
	Span span;
	Ast_Resolve_Tag tag;
	option<Ast_Access*> access;
	std::vector<Ast_Expr*> input_exprs;

	union
	{
		struct Unresolved { option<Ast_Ident> import; Ast_Ident ident; } unresolved;
		struct Resolved   { Ast_Decl_Proc* proc_decl; u32 proc_id; } resolved;
	};
};

struct Ast_Array_Init
{
	Ast_Resolve_Tag tag;
	option<Ast_Type> type;
	std::vector<Ast_Expr*> input_exprs;
};

struct Ast_Struct_Init
{
	Ast_Resolve_Tag tag;
	std::vector<Ast_Expr*> input_exprs;
	
	union
	{
		struct Unresolved { option<Ast_Ident> import; option<Ast_Ident> ident; } unresolved;
		struct Resolved   { option<Ast_Type_Struct> type; } resolved;
	};
};

//@new syntax
struct Ast_Module_Access
{
	std::vector<Ast_Ident> modules;
};

enum class Ast_Resolve_Import_Tag
{
	Unresolved,
	Resolved_Module,
	Resolved_Symbol,
	Resolved_Wildcard,
	Resolved_Symbol_List,
	Invalid,
};

struct Ast_Decl_Import_New
{
	Ast_Resolve_Import_Tag tag;
	option<Ast_Module_Access*> module_access;

	union
	{
		struct Unresolved           { Ast_Ident ident; } unresolved;
		struct Resolved_Module      { Ast* target_ast; } resolved_module;
		struct Resolved_Symbol      { Ast_Ident symbol; } resolved_symbol;
		struct Resolved_Symbol_List { std::vector<Ast_Ident> symbols; } resolved_symbol_list;
	};
};

#endif
