#ifndef CHECKER_H
#define CHECKER_H

#include "ast.h"
#include "checker_context.h"

void check_decl_uniqueness(Checker_Context* cc, Module_Map& modules);
void check_decls(Checker_Context* cc);
void check_main_proc(Checker_Context* cc);
void check_program(Checker_Context* cc);
void check_ast(Checker_Context* cc);

static void check_struct_decl(Checker_Context* cc, Ast_Struct_Decl* struct_decl);
static void check_enum_decl(Checker_Context* cc, Ast_Enum_Decl* enum_decl);
static void check_proc_decl(Checker_Context* cc, Ast_Proc_Decl* proc_decl);
static Ast* try_import(Checker_Context* cc, option<Ast_Ident> import);
static option<Ast_Struct_Decl_Meta> find_struct(Ast* target_ast, Ast_Ident ident);
static option<Ast_Enum_Decl_Meta> find_enum(Ast* target_ast, Ast_Ident ident);
static option<Ast_Proc_Decl_Meta> find_proc(Ast* target_ast, Ast_Ident ident);
static option<u32> find_enum_variant(Ast_Enum_Decl* enum_decl, Ast_Ident ident);
static option<u32> find_struct_field(Ast_Struct_Decl* struct_decl, Ast_Ident ident);

static Terminator check_block_cfg(Checker_Context* cc, Ast_Block* block, bool is_loop, bool is_defer);
static void check_if_cfg(Checker_Context* cc, Ast_If* _if, bool is_loop, bool is_defer);
static void check_switch_cfg(Checker_Context* cc, Ast_Switch* _switch, bool is_loop, bool is_defer);
static void check_block(Checker_Context* cc, Ast_Block* block, Checker_Block_Flags flags);
static void check_if(Checker_Context* cc, Ast_If* _if);
static void check_for(Checker_Context* cc, Ast_For* _for);
static void check_return(Checker_Context* cc, Ast_Return* _return);
static void check_switch(Checker_Context* cc, Ast_Switch* _switch);
static void check_var_decl(Checker_Context* cc, Ast_Var_Decl* var_decl);
static void check_var_assign(Checker_Context* cc, Ast_Var_Assign* var_assign);

static Type_Kind type_kind(Checker_Context* cc, Ast_Type type);
static Ast_Type type_from_basic(BasicType basic_type);
static bool match_type(Checker_Context* cc, Ast_Type type_a, Ast_Type type_b);
static void type_implicit_cast(Checker_Context* cc, Ast_Type* type, Ast_Type target_type);
static void type_implicit_binary_cast(Checker_Context* cc, Ast_Type* type_a, Ast_Type* type_b);
static option<Ast_Type> check_type_signature(Checker_Context* cc, Ast_Type* type);
static option<Ast_Type> check_expr(Checker_Context* cc, option<Type_Context*> context, Ast_Expr* expr);
static option<Ast_Type> check_term(Checker_Context* cc, option<Type_Context*> context, Ast_Term* term);
static option<Ast_Type> check_var(Checker_Context* cc, Ast_Var* var);
static option<Ast_Type> check_access(Checker_Context* cc, Ast_Type type, option<Ast_Access*> optional_access);
static option<Ast_Type> check_proc_call(Checker_Context* cc, Ast_Proc_Call* proc_call, Checker_Proc_Call_Flags flags);
static option<Ast_Type> check_unary_expr(Checker_Context* cc, option<Type_Context*> context, Ast_Unary_Expr* unary_expr);
static option<Ast_Type> check_binary_expr(Checker_Context* cc, option<Type_Context*> context, Ast_Binary_Expr* binary_expr);

static void error_pair(const char* message, const char* labelA, Ast_Ident identA, const char* labelB, Ast_Ident identB);
static void error(const char* message, Ast_Ident ident);

#endif
