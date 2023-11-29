export module checker;

import ast;
import err_handler;

export bool check_ast(Ast* ast);

template<typename Pass_Proc>
void module_pass(Ast_Module* module, Pass_Proc pass);
void module_pass_add_decl_symbols(Ast_Module* module);
void module_pass_import_symbols(Ast_Module* module);
void module_pass_resolve_impl(Ast_Module* module);
void module_pass_remove_duplicates(Ast_Module* module);

module : private;

bool check_ast(Ast* ast) {
	module_pass(ast->root, module_pass_add_decl_symbols);
	module_pass(ast->root, module_pass_import_symbols);
	module_pass(ast->root, module_pass_resolve_impl);
	module_pass(ast->root, module_pass_remove_duplicates);
	return !err_get_status();
}

template<typename Pass_Proc>
void module_pass(Ast_Module* module, Pass_Proc pass) {
	pass(module);
	for (Ast_Module* submodule : module->submodules) {
		pass(submodule);
	}
}

option<Ast_Ident> ident_from_decl(Ast_Decl* decl) {
	switch (decl->tag()) {
	case Ast_Decl::Tag::Proc: return decl->as_proc->ident;
	case Ast_Decl::Tag::Enum: return decl->as_enum->ident;
	case Ast_Decl::Tag::Struct: return decl->as_struct->ident;
	case Ast_Decl::Tag::Global: return decl->as_global->ident;
	default: return {};
	}
}

void module_scope_try_add_symbol(Ast_Module* module, Ast_Decl* decl)
{
	option<Ast_Ident> ident = ident_from_decl(decl);
	if (!ident) return;
	
	option<Ast_Decl*> existing_decl = module->scope->decl_map.find(ident.value());
	if (!existing_decl) {
		module->scope->decl_map.add(ident.value(), decl);
		return;
	}

	err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
	err_context(module->source, ident.value().span);
	err_context("previous declaration:");
	err_context(module->source, ident_from_decl(existing_decl.value()).value().span);
}

void module_pass_add_decl_symbols(Ast_Module* module) {
	module->scope->decl_map.init(256);
	for (Ast_Decl* decl : module->decls) {
		module_scope_try_add_symbol(module, decl);
	}
}

void module_pass_import_symbols(Ast_Module* module) {
	//resolve import decls
	//import symbols -> add to module scope
}

void module_pass_resolve_impl(Ast_Module* module) {
	//resolve impl types
	//check duplicate impl methods -> add to impl scope
}

void module_pass_remove_duplicates(Ast_Module* module) {
	//check duplicates:
	//-enum variants
	//-struct fields
	//-proc & impl proc params
}
