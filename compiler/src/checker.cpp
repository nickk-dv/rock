
struct Block_Info;
struct Block_Checker;

struct Checker
{
	bool check_ast(Ast* ast);
	bool check_types_and_proc_definitions(Ast* ast);
	bool check_is_proc_in_scope(Ast_Identifier* proc_ident);
	bool check_enum(Ast_Enum_Declaration* decl);
	bool check_struct(Ast_Struct_Declaration* decl);
	bool check_procedure(Ast_Procedure_Declaration* decl);
	bool check_procedure_block(Ast_Procedure_Declaration* decl);

	bool check_block(Ast_Block* block, Block_Checker* bc, bool is_entry, bool is_inside_loop);
	bool check_if(Ast_If* _if, Block_Checker* bc);
	bool check_else(Ast_Else* _else, Block_Checker* bc);
	bool check_for(Ast_For* _for, Block_Checker* bc);
	bool check_break(Ast_Break* _break, Block_Checker* bc);
	bool check_return(Ast_Return* _return, Block_Checker* bc);
	bool check_continue(Ast_Continue* _continue, Block_Checker* bc);
	bool check_proc_call(Ast_Proc_Call* proc_call, Block_Checker* bc);
	bool check_var_assign(Ast_Var_Assign* var_assign, Block_Checker* bc);
	bool check_var_declare(Ast_Var_Declare* var_declare, Block_Checker* bc);
	std::optional<Type_Info> check_access_chain(Ast_Access_Chain* access_chain, Block_Checker* bc);
	std::optional<Type_Info> check_expr(Ast_Expression* expr, Block_Checker* bc);

	std::unordered_map<StringView, Ast_Procedure_Declaration*, StringViewHasher> proc_table;
	Typer typer;
};

struct Block_Info
{
	Ast_Block* block;
	u32 var_count;
	bool is_inside_loop;
};

struct Block_Checker
{
	void block_enter(Ast_Block* block, bool is_inside_loop);
	void block_exit();
	void var_add(const IdentTypePair& ident_type);
	void var_add(const Ast_Identifier& ident, const Ast_Identifier& type);
	bool is_var_declared(const Ast_Identifier& ident);
	bool is_inside_a_loop();
	Ast_Identifier var_get_type(const Ast_Identifier& ident);

	std::vector<Block_Info> block_stack;
	std::vector<IdentTypePair> var_stack; //@Perf this is basic linear search symbol table for the proc block
};

bool Checker::check_ast(Ast* ast)
{
	typer.init_primitive_types();

	if (!check_types_and_proc_definitions(ast)) return false;
	
	bool declarations_valid = true;
	for (auto& decl : ast->structs) if (!check_struct(&decl)) declarations_valid = false;
	for (auto& decl : ast->enums) if (!check_enum(&decl)) declarations_valid = false;
	for (auto& decl : ast->procedures) if (!check_procedure(&decl)) declarations_valid = false;
	if (!declarations_valid) return false;

	bool procedure_blocks_valid = true;
	for (auto& decl : ast->procedures)
	if (!check_procedure_block(&decl)) procedure_blocks_valid = false;
	if (!procedure_blocks_valid) return false;

	return true;
}

bool Checker::check_types_and_proc_definitions(Ast* ast)
{
	for (auto& decl : ast->structs)
	{
		if (typer.is_type_in_scope(&decl.type)) { printf("Struct type redifinition.\n"); return false; }
		typer.add_struct_type(&decl);
	}
	for (auto& decl : ast->enums)
	{
		if (typer.is_type_in_scope(&decl.type)) { printf("Enum type redifinition.\n"); return false; }
		typer.add_enum_type(&decl);
	}
	for (auto& decl : ast->procedures)
	{
		if (check_is_proc_in_scope(&decl.ident)) { printf("Procedure redifinition"); return false; }
		proc_table.emplace(decl.ident.token.string_value, &decl);
	}
	return true;
}

bool Checker::check_is_proc_in_scope(Ast_Identifier* proc_ident)
{
	return proc_table.find(proc_ident->token.string_value) != proc_table.end();
}

bool Checker::check_struct(Ast_Struct_Declaration* decl) //@Incomplete allow for multple errors
{
	if (decl->fields.empty()) { printf("Struct must have at least 1 field.\n"); return false; }

	std::unordered_set<StringView, StringViewHasher> names; //@Perf
	for (auto& field : decl->fields)
	{
		if (names.find(field.ident.token.string_value) != names.end()) { printf("Field name redifinition.\n"); return false; }
		if (!typer.is_type_in_scope(&field.type)) { printf("Field type is not in scope.\n"); return false; }
		names.emplace(field.ident.token.string_value);
	}
	return true;
}

bool Checker::check_enum(Ast_Enum_Declaration* decl) //@Incomplete allow for multple errors
{
	if (decl->variants.empty()) { printf("Enum must have at least 1 variant.\n"); return false; }

	std::unordered_set<StringView, StringViewHasher> names; //@Perf
	for (const auto& field : decl->variants)
	{
		if (names.find(field.ident.token.string_value) != names.end()) { printf("Variant name redifinition.\n"); return false; }
		names.emplace(field.ident.token.string_value);
	}
	return true;
}

bool Checker::check_procedure(Ast_Procedure_Declaration* decl)
{
	std::unordered_set<StringView, StringViewHasher> names; //@Perf
	for (auto& param : decl->input_params)
	{
		if (names.find(param.ident.token.string_value) != names.end()) { printf("Procedure parameter name redifinition.\n"); return false; }
		if (!typer.is_type_in_scope(&param.type)) { printf("Procedure parameter type is not in scope.\n"); return false; }
		names.emplace(param.ident.token.string_value);
	}
	if (decl->return_type.has_value() && !typer.is_type_in_scope(&decl->return_type.value())) { printf("Procedure return type is not in scope.\n"); return false; }
	return true;
}

bool Checker::check_procedure_block(Ast_Procedure_Declaration* decl)
{
	Block_Checker bc = {};
	bc.block_enter(decl->block, false);
	for (const auto& param : decl->input_params)
	bc.var_add(param);

	bool result = check_block(decl->block, &bc, true, false);
	return result;
}

void Block_Checker::block_enter(Ast_Block* block, bool is_inside_loop)
{
	if (block_stack.size() > 0 && is_inside_a_loop()) is_inside_loop = true;
	block_stack.emplace_back( Block_Info { block, 0, is_inside_loop });
}

void Block_Checker::block_exit()
{
	Block_Info block = block_stack[block_stack.size() - 1]; //@Perf copying this its small for now
	for (u32 i = 0; i < block.var_count; i++)
		var_stack.pop_back();
	block_stack.pop_back();
}

void Block_Checker::var_add(const IdentTypePair& ident_type)
{
	var_stack.emplace_back(ident_type);
	block_stack[block_stack.size() - 1].var_count += 1;
}

void Block_Checker::var_add(const Ast_Identifier& ident, const Ast_Identifier& type)
{
	var_stack.emplace_back(IdentTypePair { ident, type });
	block_stack[block_stack.size() - 1].var_count += 1;
}

bool Block_Checker::is_var_declared(const Ast_Identifier& ident) //@Perf linear search for now
{
	for (const auto& var : var_stack)
	if (var.ident.token.string_value == ident.token.string_value) return true;
	return false;
}

bool Block_Checker::is_inside_a_loop()
{
	return block_stack[block_stack.size() - 1].is_inside_loop;
}

Ast_Identifier Block_Checker::var_get_type(const Ast_Identifier & ident)
{
	for (const auto& var : var_stack)
	if (var.ident.token.string_value == ident.token.string_value) return var.type;
	printf("FATAL Block_Checker::var_get_type expected to find the var but didnt");
	return {};
}

bool Checker::check_block(Ast_Block* block, Block_Checker* bc, bool is_entry, bool is_inside_loop)
{
	if (!is_entry) 
	bc->block_enter(block, is_inside_loop);

	for (Ast_Statement* stmt : block->statements)
	{
		switch (stmt->tag)
		{
			case Ast_Statement::Tag::If: { if (!check_if(stmt->as_if, bc)) return false; } break;
			case Ast_Statement::Tag::For: { if (!check_for(stmt->as_for, bc)) return false; } break;
			case Ast_Statement::Tag::Break: { if (!check_break(stmt->as_break, bc)) return false; } break;
			case Ast_Statement::Tag::Return: { if (!check_return(stmt->as_return, bc)) return false; } break;
			case Ast_Statement::Tag::Continue: { if (!check_continue(stmt->as_continue, bc)) return false; } break;
			case Ast_Statement::Tag::Proc_Call: { if (!check_proc_call(stmt->as_proc_call, bc)) return false; } break;
			case Ast_Statement::Tag::Var_Assign: { if (!check_var_assign(stmt->as_var_assign, bc)) return false; } break;
			case Ast_Statement::Tag::Var_Declare: { if (!check_var_declare(stmt->as_var_declare, bc)) return false; } break;
			default: break;
		}
	}

	bc->block_exit();
	return true;
}

bool Checker::check_if(Ast_If* _if, Block_Checker* bc)
{
	/*
	condition_expr is bool
	condition_expr valid
	block valid
	else is valid or doesnt exist
	*/

	if (!check_block(_if->block, bc, false, false)) return false;
	if (_if->_else.has_value() && !check_else(_if->_else.value(), bc)) return false;
	
	return true;
}

bool Checker::check_else(Ast_Else* _else, Block_Checker* bc)
{
	switch (_else->tag)
	{
		case Ast_Else::Tag::If: { if (!check_if(_else->as_if, bc)) return false; } break;
		case Ast_Else::Tag::Block: { if (!check_block(_else->as_block, bc, false, false)) return false; } break;
		default: break;
	}
	return true;
}

bool Checker::check_for(Ast_For* _for, Block_Checker* bc)
{
	/*
	not sure about syntax & semantics yet
	*/

	if (!check_block(_for->block, bc, false, true)) return false;
	return true;
}

bool Checker::check_break(Ast_Break* _break, Block_Checker* bc)
{
	/*
	only called inside for loop
	*/

	if (!bc->is_inside_a_loop())
	{
		printf("Invalid 'break' outside a for loop.\n");
		debug_print_token(_break->token, true, true);
		return false;
	}
	return true;
}

bool Checker::check_return(Ast_Return* _return, Block_Checker* bc)
{
	/*
	expr matches parent proc return type or lack of type
	*/

	return true;
}

bool Checker::check_continue(Ast_Continue* _continue, Block_Checker* bc)
{
	/*
	only called inside for loop
	*/

	if (!bc->is_inside_a_loop())
	{
		printf("Invalid 'continue' outside a for loop.\n");
		debug_print_token(_continue->token, true, true);
		return false;
	}
	return true;
}

bool Checker::check_proc_call(Ast_Proc_Call* proc_call, Block_Checker* bc)
{
	/*
	proc name in scope
	all input expr are valid
	all input expr match types
	*/

	if (!check_is_proc_in_scope(&proc_call->ident))
	{
		printf("Calling unknown procedure.\n"); 
		debug_print_token(proc_call->ident.token, true, true); 
		return false;
	}

	return true;
}

bool Checker::check_var_assign(Ast_Var_Assign* var_assign, Block_Checker* bc)
{
	/*
	access chain must be valid
	AssignOp is supported by lhs-rhs
	expr valid
	expr evaluates to same type
	*/

	auto chain_type = check_access_chain(var_assign->access_chain, bc);
	if (!chain_type) return false;

	return true;
}

bool Checker::check_var_declare(Ast_Var_Declare* var_declare, Block_Checker* bc)
{
	/*
	ident must not be in scope
	[has expr & has type]
	expr valid
	expr evaluates to same type
	[has expr]
	expr valid
	infer var type
	[result]
	existing or inferred type must be in scope
	add ident & type to scope
	*/

	if (bc->is_var_declared(var_declare->ident)) 
	{ 
		printf("Trying to shadow already existing variable.\n");
		debug_print_token(var_declare->ident.token, true, true);
		return false; 
	}

	Ast_Identifier default_type = { Token { TOKEN_TYPE_I64 }}; //@Default type not evaluating expr yet for inference
	bc->var_add(var_declare->ident, var_declare->type.value_or(default_type)); //@Incomplete discarding type for now
	
	return true;
}

std::optional<Type_Info> Checker::check_access_chain(Ast_Access_Chain* access_chain, Block_Checker* bc)
{
	/*
	first ident is declared variable
	further idents exist within the type
	return the last type
	*/

	if (!bc->is_var_declared(access_chain->ident))
	{
		printf("Trying to access undeclared variable.\n");
		debug_print_token(access_chain->ident.token, true, true);
		return {};
	}

	Ast_Identifier type = bc->var_get_type(access_chain->ident);
	Type_Info type_info = typer.get_type_info(&type);

	while (true)
	{
		access_chain = access_chain->next;
		if (access_chain == NULL) break;

		if (type_info.tag == TYPE_TAG_STRUCT) //@Perf switch?
		{
			bool found_field = false;

			for (const auto& field : type_info.as_struct_decl->fields)
			{
				if (field.ident.token.string_value == access_chain->ident.token.string_value)
				{
					found_field = true;
					type = field.type;
					type_info = typer.get_type_info(&type);
					break;
				}
			}

			if (!found_field)
			{
				printf("Trying to access struct field which doesnt exist.\n");
				debug_print_token(access_chain->ident.token, true, true);
				return {};
			}
		}
		else if (type_info.tag == TYPE_TAG_ENUM)
		{
			printf("Accessing fields of an Enum is NOT YET SUPPORTED.\n");
			debug_print_token(access_chain->ident.token, true, true);
			return {};
		}
		else if (type_info.tag == TYPE_TAG_PRIMITIVE) //@Assuming that primitive types dont have any accesible fields within it
		{
			printf("Trying to access a field of a primitive type.\n");
			debug_print_token(access_chain->ident.token, true, true);
			return {};
		}
	}

	return type_info;
}

std::optional<Type_Info> Checker::check_expr(Ast_Expression* expr, Block_Checker* bc)
{
	Type_Info info = {};

	switch (expr->tag)
	{
		case Ast_Expression::Tag::Term:
		{
			Ast_Term* term = expr->as_term;
			switch (term->tag)
			{
				case Ast_Term::Tag::Literal:
				{
					Ast_Literal lit = term->as_literal;
					//@determine how to get type of literal token
					//maybe mark the Ast_Literal token type with basic type tokens
					//while also keeping literal values (for easier further codegen)
				} break;
				case Ast_Term::Tag::Access_Chain:
				{
					auto chain_type = check_access_chain(term->as_access_chain, bc);
					if (!chain_type) return {};
					info = chain_type.value();
				} break;
				case Ast_Term::Tag::Proc_Call:
				{
					//validate proc_call
					//get return type it must exist
					//set type
				} break;
				default: break;
			}
		} break;
		case Ast_Expression::Tag::UnaryExpression:
		{
			Ast_Unary_Expression* unary_expr = expr->as_unary_expr;
			auto rhs_type = check_expr(unary_expr->right, bc);
			if (!rhs_type) return {};
			UnaryOp op = unary_expr->op;

			//check op applicability
			//set type
		} break;
		case Ast_Expression::Tag::BinaryExpression:
		{
			Ast_Binary_Expression* bin_expr = expr->as_binary_expr;
			auto lhs_type = check_expr(bin_expr->left, bc);
			if (!lhs_type) return {};
			auto rhs_type = check_expr(bin_expr->left, bc);
			if (!rhs_type) return {};
			BinaryOp op = bin_expr->op;

			//check op applicability
			//set type
		} break;
		default: break;
	}

	return info;
}
