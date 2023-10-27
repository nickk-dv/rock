#ifndef CHECK_GENERAL_H
#define CHECK_GENERAL_H

#include "check_context.h"

Ast* find_import(Check_Context* cc, option<Ast_Ident> import);
option<Ast_Struct_Info> find_struct(Ast* target_ast, Ast_Ident ident);
option<Ast_Enum_Info> find_enum(Ast* target_ast, Ast_Ident ident);
option<Ast_Proc_Info> find_proc(Ast* target_ast, Ast_Ident ident);
option<Ast_Global_Info> find_global(Ast* target_ast, Ast_Ident ident);
option<u32> find_enum_variant(Ast_Enum_Decl* enum_decl, Ast_Ident ident); //check_type
option<u32> find_struct_field(Ast_Struct_Decl* struct_decl, Ast_Ident ident);
void error_pair(const char* message, const char* labelA, Ast_Ident identA, const char* labelB, Ast_Ident identB);
void error(const char* message, Ast_Ident ident);

#endif
