#ifndef CHECKER_H
#define CHECKER_H

#include "common.h"
#include "ast.h"
#include "error_handler.h"

typedef std::unordered_map<std::string, Ast*> Module_Map;
enum class Terminator;
enum class Type_Kind;
struct Type_Info;
struct Var_Info;
struct Block_Stack;

void check_decl_uniqueness(Error_Handler* err, Ast* ast, Ast_Program* program, Module_Map& modules);
void check_decls(Error_Handler* err, Ast* ast);
void check_main_proc(Error_Handler* err, Ast* ast);
void check_program(Error_Handler* err, Ast_Program* program);
void check_ast(Error_Handler* err, Ast* ast);

static void check_struct_decl(Error_Handler* err, Ast* ast, Ast_Struct_Decl* struct_decl);
static void check_enum_decl(Error_Handler* err, Ast_Enum_Decl* enum_decl);
static void check_proc_decl(Error_Handler* err, Ast* ast, Ast_Proc_Decl* proc_decl);
static Ast* try_import(Error_Handler* err, Ast* ast, std::optional<Ast_Ident> import);
static std::optional<Ast_Struct_Decl_Meta> find_struct(Ast* target_ast, Ast_Ident ident);
static std::optional<Ast_Enum_Decl_Meta> find_enum(Ast* target_ast, Ast_Ident ident);
static std::optional<Ast_Proc_Decl_Meta> find_proc(Ast* target_ast, Ast_Ident ident);
static std::optional<u32> find_enum_variant(Ast_Enum_Decl* enum_decl, Ast_Ident ident);
static std::optional<u32> find_struct_field(Ast_Struct_Decl* struct_decl, Ast_Ident ident);
static Terminator check_block_cfg(Error_Handler* err, Ast_Block* block, bool is_loop, bool is_defer);
static void check_if_cfg(Error_Handler* err, Ast_If* _if, bool is_loop, bool is_defer);
static void check_block(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Block* block, bool add_block = true);
static void check_if(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_If* _if);
static void check_for(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_For* _for);
static void check_return(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Return* _return);
static void check_var_decl(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Var_Decl* var_decl);
static void check_var_assign(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Var_Assign* var_assign);
static std::optional<Type_Info> check_type(Error_Handler* err, Ast* ast, Ast_Type* type);
static std::optional<Type_Info> check_access(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Access* access, Ast_Type type);
static std::optional<Type_Info> check_expr(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Expr* expr);
static std::optional<Type_Info> check_term(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Term* term);
static std::optional<Type_Info> check_var(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Var* var);
static std::optional<Type_Info> check_enum(Error_Handler* err, Ast* ast, Ast_Enum* _enum);
static std::optional<Type_Info> check_literal(Error_Handler* err, Ast_Literal* literal);
static std::optional<Type_Info> check_proc_call(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Proc_Call* proc_call, bool is_statement);
static std::optional<Type_Info> check_unary_expr(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Unary_Expr* unary_expr);
static std::optional<Type_Info> check_binary_expr(Error_Handler* err, Ast* ast, Block_Stack* bc, Ast_Binary_Expr* binary_expr);
static Type_Kind type_kind(Error_Handler* err, Ast_Type type);
static Type_Kind type_info_kind(Error_Handler* err, Type_Info type_info);
static Type_Info type_info_from_basic(BasicType basic_type);
static void type_implicit_cast(Error_Handler* err, Ast_Type* type, Ast_Type target_type);
static void type_implicit_binary_cast(Error_Handler* err, Ast_Type* type_a, Ast_Type* type_b);

static bool match_type_info(Error_Handler* err, Type_Info type_info_a, Type_Info type_info_b);
static bool match_type(Error_Handler* err, Ast_Type type_a, Ast_Type type_b);
static void block_stack_reset(Block_Stack* bc);
static void block_stack_add_block(Block_Stack* bc);
static void block_stack_remove_block(Block_Stack* bc);
static void block_stack_add_var(Block_Stack* bc, Ast_Ident ident, Type_Info type_info);
static bool block_stack_contains_var(Block_Stack* bc, Ast_Ident ident);
static std::optional<Type_Info> block_stack_find_var_type(Block_Stack* bc, Ast_Ident ident);
static void error_pair(const char* message, const char* labelA, Ast_Ident identA, const char* labelB, Ast_Ident identB);
static void error(const char* message, Ast_Ident ident);

enum class Terminator
{
	None,
	Break,
	Return,
	Continue,
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

struct Type_Info
{
	bool is_var_owned; //@Issue accidental propagation of this might be weird, only need for '&' adress op
	Ast_Type type;
};

struct Var_Info
{
	Ast_Ident ident;
	Type_Info type_info;
};

struct Block_Stack
{
	Ast_Proc_Decl* proc_context;
	u32 block_count = 0;
	std::vector<u32> var_count_stack;
	std::vector<Var_Info> var_stack;
};

#endif
