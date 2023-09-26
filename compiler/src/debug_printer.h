#ifndef DEBUG_PRINTER_H
#define DEBUG_PRINTER_H

#include "ast.h"

void debug_print_ast(Ast* ast);
void debug_print_token(Token token, bool endl, bool location = false);

void debug_print_unary_op(UnaryOp op);
void debug_print_binary_op(BinaryOp op);
void debug_print_assign_op(AssignOp op);
void debug_print_basic_type(BasicType type);
void debug_print_branch(u32& depth);
void debug_print_spacing(u32 depth);
void debug_print_struct_decl(Ast_Struct_Decl* struct_decl);
void debug_print_enum_decl(Ast_Enum_Decl* enum_decl);
void debug_print_proc_decl(Ast_Proc_Decl* proc_decl);
void debug_print_type(Ast_Type* type);
void debug_print_ident_chain(Ast_Ident_Chain* ident_chain);
void debug_print_term(Ast_Term* term, u32 depth);
void debug_print_expr(Ast_Expr* expr, u32 depth);
void debug_print_unary_expr(Ast_Unary_Expr* unary_expr, u32 depth);
void debug_print_binary_expr(Ast_Binary_Expr* binary_expr, u32 depth);
void debug_print_block(Ast_Block* block, u32 depth);
void debug_print_statement(Ast_Statement* statement, u32 depth);
void debug_print_if(Ast_If* _if, u32 depth);
void debug_print_else(Ast_Else* _else, u32 depth);
void debug_print_for(Ast_For* _for, u32 depth);
void debug_print_break(Ast_Break* _break, u32 depth);
void debug_print_return(Ast_Return* _return, u32 depth);
void debug_print_continue(Ast_Continue* _continue, u32 depth);
void debug_print_proc_call(Ast_Proc_Call* proc_call, u32 depth);
void debug_print_var_decl(Ast_Var_Decl* var_decl, u32 depth);
void debug_print_var_assign(Ast_Var_Assign* var_assign, u32 depth);

#endif
