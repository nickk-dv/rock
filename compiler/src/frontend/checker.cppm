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

void module_scope_try_add_symbol(Ast_Module* module, Ast_Decl* decl) {
	option<Ast_Ident> ident = ident_from_decl(decl);
	if (!ident) return;
	option<Ast_Decl*> existing = module->scope->decl_map.find(ident.value());
	if (existing) {
		err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
		err_context(module->source, ident.value().span);
		err_context("previous declaration:");
		err_context(module->source, ident_from_decl(existing.value()).value().span);
	} else module->scope->decl_map.add(ident.value(), decl);
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
	std::vector<u32> ids_to_remove = {};
	HashSet<Ast_Ident> name_set = {};
	name_set.init(64);

	auto reset_collections = [&ids_to_remove, &name_set]() {
		ids_to_remove.clear();
		name_set.zero_reset();
	};

	auto proc_decl_remove_duplicates = [module, &ids_to_remove, &name_set](Ast_Decl_Proc* proc_decl) {
		for (const Ast_Proc_Param& param : proc_decl->params) {
			option<Ast_Ident> existing = name_set.find(param.ident);
			if (existing) {
				err_report(Error::DECL_PROC_DUPLICATE_PARAM);
				err_context(module->source, param.ident.span);
				err_context("already declared at:");
				err_context(module->source, existing.value().span);
			}
			else name_set.add(param.ident);
		}
		while (!ids_to_remove.empty()) {
			u32 last_id = ids_to_remove.back();
			ids_to_remove.pop_back();
			proc_decl->params.erase(proc_decl->params.begin() + last_id);
		}
	};

	auto enum_decl_remove_duplicates = [module, &ids_to_remove, &name_set](Ast_Decl_Enum* enum_decl) {
		for (const Ast_Enum_Variant& variant : enum_decl->variants) {
			option<Ast_Ident> existing = name_set.find(variant.ident);
			if (existing) {
				err_report(Error::DECL_ENUM_DUPLICATE_VARIANT);
				err_context(module->source, variant.ident.span);
				err_context("already declared at:");
				err_context(module->source, existing.value().span);
			}
			else name_set.add(variant.ident);
		}
		while (!ids_to_remove.empty()) {
			u32 last_id = ids_to_remove.back();
			ids_to_remove.pop_back();
			enum_decl->variants.erase(enum_decl->variants.begin() + last_id);
		}
	};

	auto struct_decl_remove_duplicates = [module, &ids_to_remove, &name_set](Ast_Decl_Struct* struct_decl) {
		for (const Ast_Struct_Field& field : struct_decl->fields) {
			option<Ast_Ident> existing = name_set.find(field.ident);
			if (existing) {
				err_report(Error::DECL_STRUCT_DUPLICATE_FIELD);
				err_context(module->source, field.ident.span);
				err_context("already declared at:");
				err_context(module->source, existing.value().span);
			}
			else name_set.add(field.ident);
		}
		while (!ids_to_remove.empty()) {
			u32 last_id = ids_to_remove.back();
			ids_to_remove.pop_back();
			struct_decl->fields.erase(struct_decl->fields.begin() + last_id);
		}
	};

	for (Ast_Decl* decl : module->decls) {
		switch (decl->tag()) {
		case Ast_Decl::Tag::Proc: {
			reset_collections();
			proc_decl_remove_duplicates(decl->as_proc);
		} break;
		case Ast_Decl::Tag::Impl: {
			for (Ast_Decl_Proc* proc_decl : decl->as_impl->member_procedures) {
				reset_collections();
				proc_decl_remove_duplicates(proc_decl);
			}
		} break;
		case Ast_Decl::Tag::Enum: {
			reset_collections();
			enum_decl_remove_duplicates(decl->as_enum);
		} break;
		case Ast_Decl::Tag::Struct: {
			reset_collections();
			struct_decl_remove_duplicates(decl->as_struct);
		} break;
		default: break;
		}
	}
}
