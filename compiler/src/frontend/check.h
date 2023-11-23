#ifndef CHECK_H
#define CHECK_H

#include "check_context.h"

bool check_program(Ast_Program* program);

static void check_main_entry_point(Ast_Program* program);
static void check_decls_symbols(Check_Context* cc);
static void check_decls_consteval(Check_Context* cc);
static void check_decls_finalize(Check_Context* cc);
static void check_proc_blocks(Check_Context* cc);

static Terminator check_cfg_block(Check_Context* cc, Ast_Stmt_Block* block, bool is_loop, bool is_defer);
static void check_cfg_if(Check_Context* cc, Ast_Stmt_If* _if, bool is_loop, bool is_defer);
static void check_cfg_switch(Check_Context* cc, Ast_Stmt_Switch* _switch, bool is_loop, bool is_defer);

static void check_stmt_block(Check_Context* cc, Ast_Stmt_Block* block, Checker_Block_Flags flags);
static void check_stmt_if(Check_Context* cc, Ast_Stmt_If* _if);
static void check_stmt_for(Check_Context* cc, Ast_Stmt_For* _for);
static void check_stmt_return(Check_Context* cc, Ast_Stmt_Return* _return);
static void check_stmt_switch(Check_Context* cc, Ast_Stmt_Switch* _switch);
static void check_stmt_var_decl(Check_Context* cc, Ast_Stmt_Var_Decl* var_decl);
static void check_stmt_var_assign(Check_Context* cc, Ast_Stmt_Var_Assign* var_assign);

static Ast* check_module_access(Check_Context* cc, option<Ast_Module_Access*> option_module_access);
static Ast* check_module_list(Check_Context* cc, std::vector<Ast_Ident>& modules);
static void check_decl_import(Check_Context* cc, Ast_Decl_Import* import_decl);

#endif
