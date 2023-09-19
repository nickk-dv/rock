
enum class PrimitiveType;

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
bool check_block(std::vector<IdentTypePair>& vars_in_scope, Ast_Block* block, Ast* ast);

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

enum Type_Tag
{
	TYPE_TAG_STRUCT,
	TYPE_TAG_ENUM,
	TYPE_TAG_PROCEDURE,
};

struct Type_Info
{
	Type_Tag tag;
};

static std::unordered_map<StringView, Type_Info, StringViewHasher> type_table;
static std::unordered_map<StringView, Type_Info, StringViewHasher> proc_table;

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
	return true;

	bool procedure_blocks_valid = true;
	for (const auto& decl : ast->procedures)
	if (!check_procedure_block(decl, ast)) procedure_blocks_valid = false;
	if (!procedure_blocks_valid) return false;

	return true;
}

//Populates type and proc tables + checks for redifinition and bans usage of primitive type names
bool check_populate_types_and_procedures(Ast* ast)
{
	for (const auto& decl : ast->structs)
	{
		if (check_is_type_in_scope(decl.type.token.string_value)) { printf("Struct type redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(decl.type.token.string_value)) { printf("Struct typename can not be a primitive type.\n"); return false; }
		type_table.emplace(decl.type.token.string_value, Type_Info{ TYPE_TAG_STRUCT });
	}
	for (const auto& decl : ast->enums)
	{
		if (check_is_type_in_scope(decl.type.token.string_value)) { printf("Enum type redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(decl.type.token.string_value)) { printf("Enum typename can not be a primitive type.\n"); return false; }
		type_table.emplace(decl.type.token.string_value, Type_Info{ TYPE_TAG_ENUM });
	}
	for (const auto& decl : ast->procedures)
	{
		if (check_is_proc_in_scope(decl.ident.token.string_value)) { printf("Procedure identifier redifinition"); return false; }
		if (check_is_ident_a_primitive_type(decl.ident.token.string_value)) { printf("Procedure name can not be a primitive type.\n"); return false; }
		proc_table.emplace(decl.ident.token.string_value, Type_Info{ TYPE_TAG_PROCEDURE });
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
	for (const auto& param : decl.input_parameters)
	{
		if (names.find(param.ident.token.string_value) != names.end()) { printf("Procedure parameter name redifinition.\n"); return false; }
		if (check_is_ident_a_primitive_type(param.ident.token.string_value)) { printf("Procedure parameter name can not be a primitive type.\n"); return false; }
		if (!check_is_type_in_scope(param.type.token.string_value)) { printf("Procedure parameter type is not in scope.\n"); return false; }
		names.emplace(param.ident.token.string_value);
	}
	if (decl.return_type.has_value() && !check_is_type_in_scope(decl.return_type.value().token.string_value)) { printf("Procedure return type is not in scope.\n"); return false; }
	return true;
}

//@Rework in progress ->
bool check_procedure_block(const Ast_Procedure_Declaration& decl, Ast* ast)
{
	std::vector<IdentTypePair> vars_in_scope;

	for (const auto& param : decl.input_parameters)
		vars_in_scope.emplace_back(param);

	printf("Variable inputs of proc: "); 
	debug_print_token(decl.ident.token, true);
	for (const auto& ident : vars_in_scope)
		debug_print_token(ident.ident.token, true);

	bool block_check = check_block(vars_in_scope, decl.block, ast);
	return block_check;
}

bool check_block(std::vector<IdentTypePair>& vars_in_scope, Ast_Block* block, Ast* ast)
{
	for (Ast_Statement* stmt : block->statements)
	{
		switch (stmt->tag)
		{
			case Ast_Statement::Tag::VariableAssignment:
			{
				Ast_Variable_Assignment* var_assign = stmt->as_var_assignment;
				Ast_Identifier var_type = {};
				
				//variable must be in scope for an assigment
				bool var_already_in_scope = false;
				for (const auto& var : vars_in_scope)
				{
					if (check_compare_ident(var.ident.token.string_value, var_assign->access_chain->ident.token.string_value)) //@Incomplete check entire chain
					{
						var_already_in_scope = true;
						var_type = var.type;
						break;
					}
				}
				if (!var_already_in_scope)
				{
					printf("Error: assignment to a variable which is not in scope: ");
					debug_print_token(var_assign->access_chain->ident.token, true); //@Incomplete check entire chain
					return false;
				}

				//validation of expr:
				//terms are in scope
				//binary operators are applicable
				//return type of expression and validity
				
				//validate the expr
				//expr type must match the var_type
			} break;
			case Ast_Statement::Tag::VariableDeclaration:
			{
				Ast_Variable_Declaration* var_decl = stmt->as_var_declaration;

				//variable must not be in scope
				bool var_already_in_scope = false;
				for (const auto& var : vars_in_scope)
				{
					if (check_compare_ident(var.ident.token.string_value, var_decl->ident.token.string_value))
					{
						var_already_in_scope = true;
						break;
					}
				}
				if (var_already_in_scope)
				{
					printf("Error: variable is already defined in scope: "); 
					debug_print_token(var_decl->ident.token, true);
					return false;
				}
				
				//validate expr
				//expr type must match the var_type if its specified
				//otherwise expr type is put on the declared variable
				//declared variable added to a current scope
				vars_in_scope.emplace_back( IdentTypePair { var_decl->ident, {} });
			} break;
			default:
			{
				break;
			}
		}
	}

	return true;
}
