#include "check_general.h"

#include "error_handler.h"

Ast* find_import(Check_Context* cc, option<Ast_Ident> import)
{
	if (!import) return cc->ast;
	Ast_Ident import_ident = import.value();
	option<Ast_Import_Decl*> import_decl = cc->ast->import_table.find(import_ident, hash_ident(import_ident));
	if (!import_decl)
	{
		err_report(Error::IMPORT_MODULE_NOT_FOUND);
		err_context(cc, import_ident.span);
		return NULL;
	}
	return import_decl.value()->import_ast;
}

option<Ast_Struct_Info> find_struct(Ast* target_ast, Ast_Ident ident) 
{ 
	return target_ast->struct_table.find(ident, hash_ident(ident)); 
}

option<Ast_Enum_Info> find_enum(Ast* target_ast, Ast_Ident ident) 
{ 
	return target_ast->enum_table.find(ident, hash_ident(ident)); 
}

option<Ast_Proc_Info> find_proc(Ast* target_ast, Ast_Ident ident) 
{ 
	return target_ast->proc_table.find(ident, hash_ident(ident)); 
}

option<Ast_Global_Info> find_global(Ast* target_ast, Ast_Ident ident) 
{ 
	return target_ast->global_table.find(ident, hash_ident(ident)); 
}

option<u32> find_enum_variant(Ast_Enum_Decl* enum_decl, Ast_Ident ident)
{
	for (u64 i = 0; i < enum_decl->variants.size(); i += 1)
	{
		if (match_ident(enum_decl->variants[i].ident, ident)) return (u32)i;
	}
	return {};
}

option<u32> find_struct_field(Ast_Struct_Decl* struct_decl, Ast_Ident ident)
{
	for (u64 i = 0; i < struct_decl->fields.size(); i += 1)
	{
		if (match_ident(struct_decl->fields[i].ident, ident)) return (u32)i;
	}
	return {};
}
