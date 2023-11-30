export module checker;

import ast;
import err_handler;

export bool check_ast(Ast* ast);

void init_global_scope(Global_Scope* scope);
template<typename Pass_Proc>
void module_pass(Ast* ast, Ast_Module* module, Pass_Proc pass);
void module_pass_init_scope(Ast* ast, Ast_Module* module);
void module_pass_add_decl_symbols(Ast* ast, Ast_Module* module);
void module_pass_external_proc_duplicates(Ast* ast, Ast_Module* module);
void module_pass_import_symbols(Ast* ast, Ast_Module* module);
void module_pass_resolve_impl(Ast* ast, Ast_Module* module);
void module_pass_remove_duplicates(Ast* ast, Ast_Module* module);

module : private;

bool check_ast(Ast* ast) {
	init_global_scope(ast->scope);
	module_pass(ast, ast->root, module_pass_init_scope);
	module_pass(ast, ast->root, module_pass_add_decl_symbols);
	module_pass(ast, ast->root, module_pass_external_proc_duplicates);
	module_pass(ast, ast->root, module_pass_import_symbols);
	module_pass(ast, ast->root, module_pass_resolve_impl);
	module_pass(ast, ast->root, module_pass_remove_duplicates);
	return !err_get_status();
}

void init_global_scope(Global_Scope* scope) {
	scope->external_proc_map.init(256);
}

template<typename Pass_Proc>
void module_pass(Ast* ast, Ast_Module* module, Pass_Proc pass) {
	pass(ast, module);
	for (Ast_Module* submodule : module->submodules) {
		module_pass(ast, submodule, pass);
	}
}

void module_pass_init_scope(Ast* ast, Ast_Module* module)
{
	module->scope->decl_map.init(128);
	module->scope->imported_decl_map.init(128);
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

option<bool> pub_from_decl(Ast_Decl* decl) {
	switch (decl->tag()) {
	case Ast_Decl::Tag::Proc: return decl->as_proc->is_public;
	case Ast_Decl::Tag::Enum: return decl->as_enum->is_public;
	case Ast_Decl::Tag::Struct: return decl->as_struct->is_public;
	case Ast_Decl::Tag::Global: return decl->as_global->is_public;
	default: return false;
	}
}

void module_scope_try_add_symbol(Ast_Module* module, Ast_Decl* decl) {
	option<Ast_Ident> ident = ident_from_decl(decl);
	if (!ident) return;
	option<Ast_Decl*> existing = module->scope->decl_map.find(ident.value());
	if (existing) {
		err_report(Error::DECL_SYMBOL_ALREADY_DECLARED);
		err_context(module->source, ident.value().span);
		err_context("already declared at:");
		err_context(module->source, ident_from_decl(existing.value()).value().span);
	} else module->scope->decl_map.add(ident.value(), decl);
}

void module_pass_add_decl_symbols(Ast* ast, Ast_Module* module) {
	for (Ast_Decl* decl : module->decls) {
		module_scope_try_add_symbol(module, decl);
	}
}

void module_pass_external_proc_duplicates(Ast* ast, Ast_Module* module) {
	for (Ast_Decl* decl : module->decls) {
		if (decl->tag() != Ast_Decl::Tag::Proc) continue;
		Ast_Decl_Proc* proc_decl = decl->as_proc;
		if (!proc_decl->is_external) continue;

		option<Ast_Decl*> existing = ast->scope->external_proc_map.find(proc_decl->ident);
		if (existing) {
			err_report(Error::DECL_PROC_DUPLICATE_EXTERNAL);
			err_context(module->source, proc_decl->ident.span);
			err_context("already declared at:");
			//@todo decl doesnt have associated module or source to report it
		}
		else ast->scope->external_proc_map.add(proc_decl->ident, decl);
	}
}

option<Ast_Module*> find_submodule(Ast_Module* module, Ast_Ident ident) {
	for (Ast_Module* submodule : module->submodules) {
		if (submodule->name == ident) return submodule;
	}
	return {};
}

option<Ast_Module*> check_module_path(Ast* ast, Ast_Module* module, const std::vector<Ast_Ident>& modules) {
	Ast_Module* curr = ast->root;
	if (curr->name != modules[0]) {
		err_internal("module access chain must start from the root (main) currently");
		err_context(module->source, modules[0].span);
		return {};
	}
	for (u32 i = 1; i < modules.size(); i += 1) {
		option<Ast_Module*> found = find_submodule(curr, modules[i]);
		if (!found) {
			err_internal("module name isnt found in submodules list");
			err_context(module->source, modules[i].span);
			return {};
		}
		curr = found.value();
	}
	return curr;
}

void module_pass_import_symbols(Ast* ast, Ast_Module* module) {
	//resolve import decls
	//import symbols -> add to module scope

	for (Ast_Decl* decl : module->decls) {
		if (decl->tag() != Ast_Decl::Tag::Import) continue;
		Ast_Decl_Import* import_decl = decl->as_import;
		option<Ast_Module*> target_module = check_module_path(ast, module, import_decl->modules);
		if (!target_module) continue;
		if (!import_decl->target) continue;
		Ast_Import_Target* target = import_decl->target.value();

		switch (target->tag()) {
		case Ast_Import_Target::Tag::Wildcard: {
			//@todo hashmap iterator between all symbols
		} break;
		case Ast_Import_Target::Tag::Symbol_List: {
			for (const Ast_Ident& ident : target->as_symbol_list.symbols) {
				option<Ast_Decl*> symbol_decl = target_module.value()->scope->decl_map.find(ident);
				if (!symbol_decl) {
					err_report(Error::IMPORT_SYMBOL_NOT_FOUND);
					err_context(module->source, ident.span);
					continue;
				}
				option<bool> pub = pub_from_decl(symbol_decl.value());
				if (!pub.value()) {
					err_report(Error::IMPORT_SYMBOL_NOT_PUB);
					err_context(module->source, ident.span);
					continue;
				}
				option<Ast_Decl*> existing = module->scope->decl_map.find(ident);
				if (existing) {
					err_report(Error::IMPORT_SYMBOL_ALREADY_DEFINED);
					err_context(module->source, ident.span);
					err_context("already defined at:");
					err_context(module->source, ident_from_decl(existing.value()).value().span);
					continue;
				}
				option<Ast_Decl*> existing_import = module->scope->imported_decl_map.find(ident);
				if (existing_import) {
					err_report(Error::IMPORT_SYMBOL_ALREADY_IMPORTED);
					err_context(module->source, ident.span);
					//@context where it was imported in this file
					continue;
				}
				module->scope->imported_decl_map.add(ident, symbol_decl.value());
			}
		} break;
		case Ast_Import_Target::Tag::Symbol_Or_Module: {
			Ast_Ident ident = target->as_symbol_or_module.ident;
			option<Ast_Module*> found = find_submodule(target_module.value(), ident);
			if (found) break; //@add to module anchor, to use in module accesses
			
			option<Ast_Decl*> symbol_decl = target_module.value()->scope->decl_map.find(ident);
			if (!symbol_decl) {
				err_report(Error::IMPORT_SYMBOL_NOT_FOUND);
				err_context(module->source, ident.span);
				break;
			}
			option<bool> pub = pub_from_decl(symbol_decl.value());
			if (!pub.value()) {
				err_report(Error::IMPORT_SYMBOL_NOT_PUB);
				err_context(module->source, ident.span);
				break;
			}
			option<Ast_Decl*> existing = module->scope->decl_map.find(ident);
			if (existing) {
				err_report(Error::IMPORT_SYMBOL_ALREADY_DEFINED);
				err_context(module->source, ident.span);
				err_context("already defined at:");
				err_context(module->source, ident_from_decl(existing.value()).value().span);
				break;
			}
			option<Ast_Decl*> existing_import = module->scope->imported_decl_map.find(ident);
			if (existing_import) {
				err_report(Error::IMPORT_SYMBOL_ALREADY_IMPORTED);
				err_context(module->source, ident.span);
				//@context where it was imported in this file
				break;
			}
			module->scope->imported_decl_map.add(ident, symbol_decl.value());
		} break;
		}
	}
}

void module_pass_resolve_impl(Ast* ast, Ast_Module* module) {
	//resolve impl types
	//check duplicate impl methods -> add to impl scope
}

void module_pass_remove_duplicates(Ast* ast, Ast_Module* module) {
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
