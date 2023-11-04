#ifndef CHECK_H
#define CHECK_H

#include "check_context.h"

bool check_program(Ast_Program* program);

static void check_decl_uniqueness(Check_Context* cc);
static void check_decls(Check_Context* cc);
static void check_main_proc(Check_Context* cc);
static void check_perform_struct_sizing(Check_Context* cc);
static bool check_struct_self_storage(Check_Context* cc, Ast_Struct_Decl* in_struct, u32 struct_id, std::vector<u32>& visited_ids, std::vector<Ast_Ident>& field_chain);
static void check_struct_size(Ast_Struct_IR_Info* struct_info);
static u32 check_get_basic_type_size(BasicType basic_type);
static u32 check_get_basic_type_align(BasicType basic_type);
static u32 check_get_type_size(Ast_Type type);
static u32 check_get_type_align(Ast_Type type);
static void check_ast(Check_Context* cc);
static void check_proc_block(Check_Context* cc, Ast_Proc_Decl* proc_decl);

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
