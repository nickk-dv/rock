#ifndef AST_H
#define AST_H

#include "token.h"
#include "general/hashmap.h"
#include "llvm-c/Types.h"

struct Ast;
struct Ast_Program;
struct Ast_Ident;
struct Ast_Literal;
struct Ast_Module_Tree;

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

struct Ast_Decl;
struct Ast_Decl_Proc;
struct Ast_Decl_Impl;
struct Ast_Decl_Enum;
struct Ast_Decl_Struct;
struct Ast_Decl_Global;
struct Ast_Decl_Import;
struct Ast_Proc_Param;
struct Ast_Enum_Variant;
struct Ast_Struct_Field;
struct Ast_Import_Target;

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
struct Ast_Stmt_Proc_Call;

struct Ast_Expr;
struct Ast_Unary_Expr;
struct Ast_Binary_Expr;
struct Ast_Folded_Expr;
enum class Consteval;
struct Ast_Consteval_Expr;
struct Ast_Term;
struct Ast_Enum;
struct Ast_Cast;
struct Ast_Sizeof;
struct Ast_Array_Init;
struct Ast_Struct_Init;

//@new syntax
struct Ast_Module_Access;
struct Ast_Something;
struct Ast_Access_Chain;
struct Ast_Expr_List;

Ast_Ident token_to_ident(const Token& token);
u32 hash_ident(Ast_Ident& ident);
bool match_ident(Ast_Ident& a, Ast_Ident& b);
bool match_string(std::string& a, std::string& b);

struct Ast
{
	StringView source;
	std::string filepath;
	std::vector<Span> line_spans;

	std::vector<Ast_Decl*> decls;
	std::vector<Ast_Decl_Impl*> impls;
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

struct Ast_Ident
{
	Span span;
	StringView str;
};

struct Ast_Module_Tree
{
	Ast_Ident ident;
	std::string name;
	option<Ast*> leaf_ast;
	std::vector<Ast_Module_Tree> submodules;
};

struct Ast_Program
{
	Ast_Module_Tree root;
	std::vector<Ast*> modules;
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

struct Ast_Type
{
	u32 pointer_level = 0;

public:
	enum class Tag
	{
		Basic,
		Enum,
		Struct,
		Array,
		Procedure,
		Unresolved,
		Poison,
	};

private:
	Tag tg;

public:
	union
	{
		BasicType as_basic;
		Ast_Type_Enum as_enum;
		Ast_Type_Struct as_struct;
		Ast_Type_Array* as_array;
		Ast_Type_Procedure* as_procedure;
		Ast_Type_Unresolved* as_unresolved;
	};

	inline Tag tag() { return tg; }
	inline void set_basic(BasicType type_basic)                      { tg = Tag::Basic;      as_basic = type_basic; }
	inline void set_enum(Ast_Type_Enum type_enum)                    { tg = Tag::Enum;       as_enum = type_enum; }
	inline void set_struct(Ast_Type_Struct type_struct)              { tg = Tag::Struct;     as_struct = type_struct; }
	inline void set_array(Ast_Type_Array* type_array)                { tg = Tag::Array;      as_array = type_array; }
	inline void set_procedure(Ast_Type_Procedure* type_procedure)    { tg = Tag::Procedure;  as_procedure = type_procedure; }
	inline void set_unresolved(Ast_Type_Unresolved* type_unresolved) { tg = Tag::Unresolved; as_unresolved = type_unresolved; }
	inline void set_poison()                                         { tg = Tag::Poison; }
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
	option<Ast_Module_Access*> module_access;
	Ast_Ident ident;
};

struct Ast_Decl
{
public:
	enum class Tag
	{
		Proc,
		Impl,
		Enum,
		Struct,
		Global,
		Import,
	};

private:
	Tag tg;

public:
	union
	{
		Ast_Decl_Proc* as_proc;
		Ast_Decl_Impl* as_impl;
		Ast_Decl_Enum* as_enum;
		Ast_Decl_Struct* as_struct;
		Ast_Decl_Global* as_global;
		Ast_Decl_Import* as_import;
	};

	inline Tag tag() { return tg; }
	inline void set_proc(Ast_Decl_Proc* decl_proc)       { tg = Tag::Proc;   as_proc = decl_proc; }
	inline void set_impl(Ast_Decl_Impl* decl_impl)       { tg = Tag::Impl;   as_impl = decl_impl; }
	inline void set_enum(Ast_Decl_Enum* decl_enum)       { tg = Tag::Enum;   as_enum = decl_enum; }
	inline void set_struct(Ast_Decl_Struct* decl_struct) { tg = Tag::Struct; as_struct = decl_struct; }
	inline void set_global(Ast_Decl_Global* decl_global) { tg = Tag::Global; as_global = decl_global; }
	inline void set_import(Ast_Decl_Import* decl_import) { tg = Tag::Import; as_import = decl_import; }
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
	bool self; //@handle the self in better ways
};

struct Ast_Decl_Impl
{
	Ast_Type type;
	std::vector<Ast_Decl_Proc*> member_procedures;
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
	std::vector<Ast_Ident> modules;
	option<Ast_Import_Target*> target;
};

struct Ast_Import_Target
{
public:
	enum class Tag
	{
		Wildcard,
		Symbol_List,
		Symbol_Or_Module,
	};

private:
	Tag tg;

public:
	union
	{
		struct Symbol_List { std::vector<Ast_Ident> symbols; } as_symbol_list;
		struct Symbol_Or_Module { Ast_Ident ident; } as_symbol_or_module;
	};
	
	inline Tag tag() { return tg; }
	inline void set_wildcard()                        { tg = Tag::Wildcard; }
	inline void set_symbol_list()                     { tg = Tag::Symbol_List; }
	inline void set_symbol_or_module(Ast_Ident ident) { tg = Tag::Symbol_Or_Module; as_symbol_or_module.ident = ident; }
};

struct Ast_Stmt
{
public:
	enum class Tag
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

private:
	Tag tg;

public:
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
		Ast_Stmt_Proc_Call* as_proc_call;
	};

	inline Tag tag() { return tg; }
	inline void set_if(Ast_Stmt_If* _if)                        { tg = Tag::If;         as_if = _if; }
	inline void set_for(Ast_Stmt_For* _for)                     { tg = Tag::For;        as_for = _for; }
	inline void set_block(Ast_Stmt_Block* block)                { tg = Tag::Block;      as_block = block; }
	inline void set_defer(Ast_Stmt_Defer* defer)                { tg = Tag::Defer;      as_defer = defer; }
	inline void set_break(Ast_Stmt_Break* _break)               { tg = Tag::Break;      as_break = _break; }
	inline void set_return(Ast_Stmt_Return* _return)            { tg = Tag::Return;     as_return = _return; }
	inline void set_switch(Ast_Stmt_Switch* _switch)            { tg = Tag::Switch;     as_switch = _switch; }
	inline void set_continue(Ast_Stmt_Continue* _continue)      { tg = Tag::Continue;   as_continue = _continue; }
	inline void set_var_decl(Ast_Stmt_Var_Decl* var_decl)       { tg = Tag::Var_Decl;   as_var_decl = var_decl; }
	inline void set_var_assign(Ast_Stmt_Var_Assign* var_assign) { tg = Tag::Var_Assign; as_var_assign = var_assign; }
	inline void set_proc_call(Ast_Stmt_Proc_Call* proc_call)    { tg = Tag::Proc_Call;  as_proc_call = proc_call; }
};

struct Ast_Stmt_If
{
	Span span;
	Ast_Expr* condition_expr;
	Ast_Stmt_Block* block;
	option<Ast_Else*> _else;
};

struct Ast_Else
{
	Span span;

public:
	enum class Tag
	{
		If,
		Block,
	};

private:
	Tag tg;

public:
	union
	{
		Ast_Stmt_If* as_if;
		Ast_Stmt_Block* as_block;
	};

	inline Tag tag() { return tg; }
	inline void set_if(Ast_Stmt_If* _if)         { tg = Tag::If;    as_if = _if; }
	inline void set_block(Ast_Stmt_Block* block) { tg = Tag::Block; as_block = block; }
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
	Ast_Something* something;
	AssignOp op;
	Ast_Expr* expr;
};

struct Ast_Stmt_Proc_Call
{
	Ast_Something* something;
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

struct Ast_Expr
{
	Span span;
	u16 flags;

public:
	enum class Tag
	{
		Term,
		Unary,
		Binary,
		Folded,
	};

private:
	Tag tg;

public:
	union
	{
		Ast_Term* as_term;
		Ast_Unary_Expr* as_unary_expr;
		Ast_Binary_Expr* as_binary_expr;
		Ast_Folded_Expr as_folded_expr;
	};

	inline Tag tag() { return tg; }
	inline void set_term(Ast_Term* term)                 { tg = Tag::Term;    as_term = term; }
	inline void set_unary(Ast_Unary_Expr* unary_expr)    { tg = Tag::Unary;   as_unary_expr = unary_expr; }
	inline void set_binary(Ast_Binary_Expr* binary_expr) { tg = Tag::Binary;  as_binary_expr = binary_expr; }
	inline void set_folded(Ast_Folded_Expr folded_expr)  { tg = Tag::Folded;  as_folded_expr = folded_expr; }
	inline void ptr_tag_copy(Ast_Expr* other)            { tg = other->tag(); as_term = other->as_term; }
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

struct Ast_Term
{
public:
	enum class Tag
	{
		Enum,
		Cast,
		Sizeof,
		Literal,
		Array_Init,
		Struct_Init,
		Something,
	};

private:
	Tag tg;

public:
	union
	{
		Ast_Enum* as_enum;
		Ast_Cast* as_cast;
		Ast_Sizeof* as_sizeof;
		Ast_Literal* as_literal;
		Ast_Something* as_something;
		Ast_Array_Init* as_array_init;
		Ast_Struct_Init* as_struct_init;
	};

	inline Tag tag() { return tg; }
	inline void set_enum(Ast_Enum* _enum)                     { tg = Tag::Enum;        as_enum = _enum; }
	inline void set_cast(Ast_Cast* cast)                      { tg = Tag::Cast;        as_cast = cast; }
	inline void set_sizeof(Ast_Sizeof* size_of)               { tg = Tag::Sizeof;      as_sizeof = size_of; }
	inline void set_literal(Ast_Literal* literal)             { tg = Tag::Literal;     as_literal = literal; }
	inline void set_something(Ast_Something* something)       { tg = Tag::Something;   as_something = something; }
	inline void set_array_init(Ast_Array_Init* array_init)    { tg = Tag::Array_Init;  as_array_init = array_init; }
	inline void set_struct_init(Ast_Struct_Init* struct_init) { tg = Tag::Struct_Init; as_struct_init = struct_init; }
};

enum class Ast_Resolve_Tag
{
	Unresolved,
	Resolved,
	Invalid,
};

struct Ast_Enum
{
	Ast_Resolve_Tag tag;
	
	union
	{
		struct Unresolved 
		{
			Ast_Ident variant_ident;
		} unresolved;

		struct Resolved 
		{ 
			Ast_Type_Enum type; 
			u32 variant_id; 
		} resolved;
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

struct Ast_Array_Init
{
	Ast_Resolve_Tag tag;
	option<Ast_Type> type;
	Ast_Expr_List* input;
};

struct Ast_Struct_Init
{
	Ast_Resolve_Tag tag;
	Ast_Expr_List* input;
	
	union
	{
		struct Unresolved 
		{ 
			option<Ast_Module_Access*> module_access;
			option<Ast_Ident> struct_ident;
		} unresolved;

		struct Resolved 
		{ 
			option<Ast_Type_Struct> type; 
		} resolved;
	};
};

//@new syntax
struct Ast_Module_Access
{
	std::vector<Ast_Ident> modules;
};

struct Ast_Something
{
	option<Ast_Module_Access*> module_access;
	Ast_Access_Chain* chain;
};

struct Ast_Access_Chain
{
	option<Ast_Access_Chain*> next;

public:
	enum class Tag
	{
		Ident,
		Array,
		Call,
	};

private:
	Tag tg;

public:
	union
	{
		Ast_Ident as_ident;
		struct Array { Ast_Expr* index_expr; } as_array;
		struct Call  { Ast_Ident ident; Ast_Expr_List* input; } as_call;
	};

	inline Tag tag() { return tg; }
	inline void set_ident(Ast_Ident ident)                      { tg = Tag::Ident; as_ident = ident; }
	inline void set_array(Ast_Expr* index_expr)                 { tg = Tag::Array; as_array.index_expr = index_expr; }
	inline void set_call(Ast_Ident ident, Ast_Expr_List* input) { tg = Tag::Call;  as_call.ident = ident; as_call.input = input; }
};

struct Ast_Expr_List
{
	std::vector<Ast_Expr*> exprs;
};

#endif
