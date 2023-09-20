
//@Notice need better type system Apis
//@Notice need possibly multiple passes for typing and other validation checks

enum class PrimitiveType;
struct Expr_Type_Info;
struct Block_Checker;

bool check_is_ident_a_primitive_type(const StringView& str);
PrimitiveType check_get_primitive_type_of_ident(const StringView& str);
bool check_ast(Ast* ast);
bool check_populate_types_and_procedures(Ast* ast);
bool check_is_type_in_scope(const StringView& str);
bool check_is_proc_in_scope(const StringView& str);
bool check_enum(const Ast_Enum_Declaration& decl, Ast* ast);
bool check_struct(const Ast_Struct_Declaration& decl, Ast* ast);
bool check_procedure(const Ast_Procedure_Declaration& decl, Ast* ast);
bool check_procedure_block(const Ast_Procedure_Declaration& decl, Ast* ast);
//@Later Ast* is not used, due to having tables for types & proc names accesible
bool check_block(Ast_Block* block, Ast* ast, Block_Checker* bc, bool is_entry, bool is_inside_loop);
bool check_if(Ast_If* _if, Ast* ast, Block_Checker* bc);
bool check_else(Ast_Else* _else, Ast* ast, Block_Checker* bc);
bool check_for(Ast_For* _for, Ast* ast, Block_Checker* bc);
bool check_break(Ast_Break* _break, Ast* ast, Block_Checker* bc);
bool check_return(Ast_Return* _return, Ast* ast, Block_Checker* bc);
bool check_continue(Ast_Continue* _continue, Ast* ast, Block_Checker* bc);
bool check_proc_call(Ast_Proc_Call* proc_call, Ast* ast, Block_Checker* bc);
bool check_var_assign(Ast_Var_Assign* var_assign, Ast* ast, Block_Checker* bc);
bool check_var_declare(Ast_Var_Declare* var_declare, Ast* ast, Block_Checker* bc);
Expr_Type_Info check_access_chain(Ast_Access_Chain* access_chain, Ast* ast, Block_Checker* bc);
Expr_Type_Info check_expr(Ast_Expression* expr, Ast* ast, Block_Checker* bc);

struct Expr_Type_Info
{
	void set_primitive_type(PrimitiveType type);

	bool is_valid;
	bool is_primitive;
	StringView type_ident;
	PrimitiveType primitive_type;
};

void Expr_Type_Info::set_primitive_type(PrimitiveType type)
{
	primitive_type = type;
	is_primitive = true;
}

enum class PrimitiveType
{
	i8,
	u8,
	i16,
	u16,
	i32,
	u32,
	i64,
	u64,
	f32,
	f64,
	Bool,
	string, //@Design strings as primitive types is a good idea probably
	NotPrimitive,
};

bool check_is_ident_a_primitive_type(const StringView& str)
{
	return check_get_primitive_type_of_ident(str) != PrimitiveType::NotPrimitive;
}

static const std::unordered_map<u64, PrimitiveType> ident_hash_to_primitive_type =
{
	{ hash_ascii_9("i8"),   PrimitiveType::i8 },
	{ hash_ascii_9("u8"),   PrimitiveType::u8 },
	{ hash_ascii_9("i16"),  PrimitiveType::i16 },
	{ hash_ascii_9("u16"),  PrimitiveType::u16 },
	{ hash_ascii_9("i32"),  PrimitiveType::i32 },
	{ hash_ascii_9("u32"),  PrimitiveType::u32 },
	{ hash_ascii_9("i64"),  PrimitiveType::i64 },
	{ hash_ascii_9("u64"),  PrimitiveType::u64 },
	{ hash_ascii_9("f32"),  PrimitiveType::f32 },
	{ hash_ascii_9("f64"),  PrimitiveType::f64 },
	{ hash_ascii_9("bool"), PrimitiveType::Bool },
};

PrimitiveType check_get_primitive_type_of_ident(const StringView& str)
{
	if (str.count > 4) return PrimitiveType::NotPrimitive;
	u64 hash = string_hash_ascii_9(str);
	bool is_primitive_type = ident_hash_to_primitive_type.find(hash) != ident_hash_to_primitive_type.end();
	return is_primitive_type ? ident_hash_to_primitive_type.at(hash) : PrimitiveType::NotPrimitive;
}

struct Type_Info
{
	enum class Tag
	{
		Struct, Enum,
	} tag;
	
	union
	{
		Ast_Struct_Declaration* struct_decl;
		Ast_Enum_Declaration* enum_decl;
	};
};

struct Proc_Info
{
	Ast_Procedure_Declaration* proc_decl;
};

static std::unordered_map<StringView, Type_Info, StringViewHasher> type_table;
static std::unordered_map<StringView, Proc_Info, StringViewHasher> proc_table;

bool check_ast(Ast* ast)
{
	if (!check_populate_types_and_procedures(ast)) return false;
	
	bool declarations_valid = true;
	for (const auto& decl : ast->structs)
	if (!check_struct(decl, ast)) declarations_valid = false;
	for (const auto& decl : ast->enums)
	if (!check_enum(decl, ast)) declarations_valid = false;
	for (const auto& decl : ast->procedures)
	if (!check_procedure(decl, ast)) declarations_valid = false;
	if (!declarations_valid) return false;

	bool procedure_blocks_valid = true;
	for (const auto& decl : ast->procedures)
	if (!check_procedure_block(decl, ast)) procedure_blocks_valid = false;
	if (!procedure_blocks_valid) return false;

	return true;
}

//Populates type and proc tables + checks for redifinition and bans usage of primitive type names
bool check_populate_types_and_procedures(Ast* ast)
{
	for (auto& decl : ast->structs)
	{
		if (check_is_type_in_scope(decl.type.token.string_value)) { printf("Struct type redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(decl.type.token.string_value)) { printf("Struct typename can not be a primitive type.\n"); return false; }
		Type_Info type_info = { Type_Info::Tag::Struct };
		type_info.struct_decl = &decl;
		type_table.emplace(decl.type.token.string_value, type_info);
	}
	for (auto& decl : ast->enums)
	{
		if (check_is_type_in_scope(decl.type.token.string_value)) { printf("Enum type redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(decl.type.token.string_value)) { printf("Enum typename can not be a primitive type.\n"); return false; }
		Type_Info type_info = { Type_Info::Tag::Enum };
		type_info.enum_decl = &decl;
		type_table.emplace(decl.type.token.string_value, type_info);
	}
	for (auto& decl : ast->procedures)
	{
		if (check_is_proc_in_scope(decl.ident.token.string_value)) { printf("Procedure identifier redifinition"); return false; }
		if (check_is_ident_a_primitive_type(decl.ident.token.string_value)) { printf("Procedure name can not be a primitive type.\n"); return false; }
		proc_table.emplace(decl.ident.token.string_value, Proc_Info { &decl });
	}
	return true;
}

//@Maybe add basic types to a table somehow, but this works too
bool check_is_type_in_scope(const StringView& str)
{
	return (type_table.find(str) != type_table.end()) || check_is_ident_a_primitive_type(str);
}

bool check_is_proc_in_scope(const StringView& str)
{
	return proc_table.find(str) != proc_table.end();
}

//@Incomplete: recursive infinite size detection, general type sizing
//@Incomplete allow for multple errors per struct
bool check_struct(const Ast_Struct_Declaration& decl, Ast* ast)
{
	if (decl.fields.empty()) { printf("Struct must have at least 1 field.\n"); return false; }

	std::unordered_set<StringView, StringViewHasher> names; //@Perf re-use 1 set with clear() in between checks
	for (const auto& field : decl.fields)
	{
		if (names.find(field.ident.token.string_value) != names.end()) { printf("Field name redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(field.ident.token.string_value)) { printf("Field name can not be a primitive type.\n"); return false; }
		if (!check_is_type_in_scope(field.type.token.string_value)) { printf("Field type is not in scope.\n"); return false; }
		names.emplace(field.ident.token.string_value);
	}
	return true;
}

//@Incomplete allow for multple errors per enum
bool check_enum(const Ast_Enum_Declaration& decl, Ast* ast)
{
	if (decl.variants.empty()) { printf("Enum must have at least 1 variant.\n"); return false; }

	std::unordered_set<StringView, StringViewHasher> names; //@Perf re-use 1 set with clear() in between checks
	for (const auto& field : decl.variants)
	{
		if (names.find(field.ident.token.string_value) != names.end()) { printf("Variant name redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(field.ident.token.string_value)) { printf("Variant name can not be a primitive type.\n"); return false; }
		names.emplace(field.ident.token.string_value);
	}
	return true;
}

bool check_procedure(const Ast_Procedure_Declaration& decl, Ast* ast)
{
	std::unordered_set<StringView, StringViewHasher> names; //@Perf re-use 1 set with clear() in between checks
	for (const auto& param : decl.input_params)
	{
		if (names.find(param.ident.token.string_value) != names.end()) { printf("Procedure parameter name redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(param.ident.token.string_value)) { printf("Procedure parameter name can not be a primitive type.\n"); return false; }
		if (!check_is_type_in_scope(param.type.token.string_value)) { printf("Procedure parameter type is not in scope.\n"); return false; }
		names.emplace(param.ident.token.string_value);
	}
	if (decl.return_type.has_value() && !check_is_type_in_scope(decl.return_type.value().token.string_value)) { printf("Procedure return type is not in scope.\n"); return false; }
	return true;
}

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
	
	std::vector<Block_Info> block_stack; 
	std::vector<IdentTypePair> var_stack; //@Perf this is basic linear search symbol table for the proc block
};

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

bool check_procedure_block(const Ast_Procedure_Declaration& decl, Ast* ast)
{	
	Block_Checker bc = {};
	bc.block_enter(decl.block, false);
	for (const auto& param : decl.input_params)
	bc.var_add(param);

	bool result = check_block(decl.block, ast, &bc, true, false);
	return result;
}

bool check_block(Ast_Block* block, Ast* ast, Block_Checker* bc, bool is_entry, bool is_inside_loop)
{
	if (!is_entry) 
	bc->block_enter(block, is_inside_loop);

	for (Ast_Statement* stmt : block->statements)
	{
		switch (stmt->tag)
		{
			case Ast_Statement::Tag::If: { if (!check_if(stmt->as_if, ast, bc)) return false; } break;
			case Ast_Statement::Tag::For: { if (!check_for(stmt->as_for, ast, bc)) return false; } break;
			case Ast_Statement::Tag::Break: { if (!check_break(stmt->as_break, ast, bc)) return false; } break;
			case Ast_Statement::Tag::Return: { if (!check_return(stmt->as_return, ast, bc)) return false; } break;
			case Ast_Statement::Tag::Continue: { if (!check_continue(stmt->as_continue, ast, bc)) return false; } break;
			case Ast_Statement::Tag::Proc_Call: { if (!check_proc_call(stmt->as_proc_call, ast, bc)) return false; } break;
			case Ast_Statement::Tag::Var_Assign: { if (!check_var_assign(stmt->as_var_assign, ast, bc)) return false; } break;
			case Ast_Statement::Tag::Var_Declare: { if (!check_var_declare(stmt->as_var_declare, ast, bc)) return false; } break;
			default: break;
		}
	}

	bc->block_exit();
	return true;
}

bool check_if(Ast_If* _if, Ast* ast, Block_Checker* bc)
{
	/*
	condition_expr is bool
	condition_expr valid
	block valid
	else is valid or doesnt exist
	*/

	if (!check_block(_if->block, ast, bc, false, false)) return false;
	if (_if->_else.has_value() && !check_else(_if->_else.value(), ast, bc)) return false;
	
	return true;
}

bool check_else(Ast_Else* _else, Ast* ast, Block_Checker* bc)
{
	switch (_else->tag)
	{
		case Ast_Else::Tag::If: { if (!check_if(_else->as_if, ast, bc)) return false; } break;
		case Ast_Else::Tag::Block: { if (!check_block(_else->as_block, ast, bc, false, false)) return false; } break;
		default: break;
	}
	return true;
}

bool check_for(Ast_For* _for, Ast* ast, Block_Checker* bc)
{
	/*
	not sure about syntax & semantics yet
	*/

	if (!check_block(_for->block, ast, bc, false, true)) return false;
	return true;
}

bool check_break(Ast_Break* _break, Ast* ast, Block_Checker* bc)
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

bool check_return(Ast_Return* _return, Ast* ast, Block_Checker* bc)
{
	/*
	expr matches parent proc return type or lack of type
	*/

	return true;
}

bool check_continue(Ast_Continue* _continue, Ast* ast, Block_Checker* bc)
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

bool check_proc_call(Ast_Proc_Call* proc_call, Ast* ast, Block_Checker* bc)
{
	/*
	proc name in scope
	all input expr are valid
	all input expr match types
	*/

	if (!check_is_proc_in_scope(proc_call->ident.token.string_value))
	{
		printf("Calling unknown procedure.\n"); 
		debug_print_token(proc_call->ident.token, true, true); 
		return false;
	}

	return true;
}

bool check_var_assign(Ast_Var_Assign* var_assign, Ast* ast, Block_Checker* bc)
{
	/*
	access chain first ident must be in scope
	access chain must be valid + get type of last chain ident
	AssignOp is supported by lhs-rhs
	expr valid
	expr evaluates to same type
	*/

	if (!bc->is_var_declared(var_assign->access_chain->ident))
	{
		printf("Trying to assign to undeclared variable.\n");
		debug_print_token(var_assign->access_chain->ident.token, true, true);
		return false;
	}
	return true;
}

bool check_var_declare(Ast_Var_Declare* var_declare, Ast* ast, Block_Checker* bc)
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

	bc->var_add(var_declare->ident, Ast_Identifier{}); //@Incomplete discarding type for now
	return true;
}

Expr_Type_Info check_access_chain(Ast_Access_Chain* access_chain, Ast* ast, Block_Checker* bc) //@Hack very dumb Expr_Type_Info return type
{
	Expr_Type_Info info = {};
	info.is_valid = true;

	//also need to keep track of current Type until the end of the chain
	
	while (true)
	{
		if (!bc->is_var_declared(access_chain->ident)) //only for the first one its a local variable, later its recursive struct field
		{
			printf("Trying to access undeclared variable.\n");
			debug_print_token(access_chain->ident.token, true, true);
			info.is_valid = false;
			return info;
		}

		access_chain = access_chain->next;
		if (access_chain == NULL) break;
	}

	return info;
}

Expr_Type_Info check_expr(Ast_Expression* expr, Ast* ast, Block_Checker* bc)
{
	Expr_Type_Info info = {};
	info.is_valid = true;

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
					//@Incomplete no literal or numeric flags on tokens + floats arent parsed, defaulting to i32
					if (lit.token.type == TOKEN_NUMBER) info.set_primitive_type(PrimitiveType::i32);
					else if (lit.token.type == TOKEN_STRING) info.set_primitive_type(PrimitiveType::string); //@Incomplete strings are not first class supported yet
					else if (lit.token.type == TOKEN_BOOL_LITERAL) info.set_primitive_type(PrimitiveType::Bool);
				} break;
				case Ast_Term::Tag::Access_Chain:
				{
					info = check_access_chain(term->as_access_chain, ast, bc);
				} break;
				case Ast_Term::Tag::Proc_Call:
				{
					//validate proc_call
					//get return type, must exist
					//void return is invalid as part of expr
				} break;
				default: break;
			}
			//populate info
		} break;
		case Ast_Expression::Tag::UnaryExpression:
		{
			Ast_Unary_Expression* unary_expr = expr->as_unary_expr;
			Expr_Type_Info rhs = check_expr(unary_expr->right, ast, bc);
			UnaryOp op = unary_expr->op;
			//bubble up if any errored
			//check op applicability
			//populate info
		} break;
		case Ast_Expression::Tag::BinaryExpression:
		{
			Ast_Binary_Expression* bin_expr = expr->as_binary_expr;
			Expr_Type_Info lhs = check_expr(bin_expr->left, ast, bc);
			Expr_Type_Info rhs = check_expr(bin_expr->left, ast, bc);
			BinaryOp op = bin_expr->op;
			//bubble up if any errored
			//check op applicability
			//populate info
		} break;
		default: break;
	}

	return info;
}
