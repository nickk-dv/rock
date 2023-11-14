#ifndef CHECK_H
#define CHECK_H

#include "check_context.h"

bool check_program(Ast_Program* program);

static void check_main_entry_point(Ast_Program* program);
static void check_decls_symbols(Check_Context* cc);
static void check_decls_consteval(Check_Context* cc);
static void check_decls_finalize(Check_Context* cc);
static void check_proc_blocks(Check_Context* cc);
static bool basic_type_is_integer(BasicType type);

static Terminator check_cfg_block(Check_Context* cc, Ast_Block* block, bool is_loop, bool is_defer);
static void check_cfg_if(Check_Context* cc, Ast_If* _if, bool is_loop, bool is_defer);
static void check_cfg_switch(Check_Context* cc, Ast_Switch* _switch, bool is_loop, bool is_defer);

static void check_statement_block(Check_Context* cc, Ast_Block* block, Checker_Block_Flags flags);
static void check_statement_if(Check_Context* cc, Ast_If* _if);
static void check_statement_for(Check_Context* cc, Ast_For* _for);
static void check_statement_return(Check_Context* cc, Ast_Return* _return);
static void check_statement_switch(Check_Context* cc, Ast_Switch* _switch);
static void check_statement_var_decl(Check_Context* cc, Ast_Var_Decl* var_decl);
static void check_statement_var_assign(Check_Context* cc, Ast_Var_Assign* var_assign);

#endif
