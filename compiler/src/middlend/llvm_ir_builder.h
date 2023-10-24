#ifndef LLVM_IR_BUILDER_H
#define LLVM_IR_BUILDER_H

#include "llvm_ir_builder_context.h"

LLVMModuleRef build_module(Ast_Program* program);

static IR_Terminator build_block(IR_Builder_Context* bc, Ast_Block* block, IR_Block_Flags flags);
static void build_defer(IR_Builder_Context* bc, IR_Terminator terminator);
static void build_if(IR_Builder_Context* bc, Ast_If* _if, Basic_Block cont_block);
static void build_for(IR_Builder_Context* bc, Ast_For* _for);
static void build_switch(IR_Builder_Context* bc, Ast_Switch* _switch);
static void build_var_decl(IR_Builder_Context* bc, Ast_Var_Decl* var_decl);
static void build_var_assign(IR_Builder_Context* bc, Ast_Var_Assign* var_assign);
static Value build_proc_call(IR_Builder_Context* bc, Ast_Proc_Call* proc_call, IR_Proc_Call_Flags flags);
static Value build_expr(IR_Builder_Context* bc, Ast_Expr* expr, bool unary_address = false);
static Value build_term(IR_Builder_Context* bc, Ast_Term* term, bool unary_address = false);
static IR_Access_Info build_var(IR_Builder_Context* bc, Ast_Var* var);
static Value build_unary_expr(IR_Builder_Context* bc, Ast_Unary_Expr* unary_expr);
static Value build_binary_expr(IR_Builder_Context* bc, Ast_Binary_Expr* binary_expr);
static Value build_const_expr(IR_Builder_Context* bc, Ast_Const_Expr const_expr);
static void build_implicit_cast(IR_Builder_Context* bc, Value* value, Type type, Type target_type);
static void build_implicit_binary_cast(IR_Builder_Context* bc, Value* value_lhs, Value* value_rhs, Type type_lhs, Type type_rhs);
static char* ident_to_cstr(Ast_Ident& ident);
static Type type_from_basic_type(BasicType basic_type);
static Type type_from_ast_type(IR_Builder_Context* bc, Ast_Type type);

#endif
