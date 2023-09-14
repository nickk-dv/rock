
enum class PrimitiveType;

bool check_ast(Ast* ast);
bool check_enum(const Ast_Enum_Declaration& decl, Ast* ast);
bool check_struct(const Ast_Struct_Declaration& decl, Ast* ast);
bool check_procedure(const Ast_Procedure_Declaration& decl, Ast* ast);
bool check_procedure_block(const Ast_Procedure_Declaration& decl, Ast* ast);
bool check_compare_ident(const StringView& str, const StringView& str2);
bool check_is_ident_type_unique(const StringView& str, Ast* ast);
bool check_is_ident_type_in_scope(const StringView& str, Ast* ast);
bool check_is_ident_a_primitive_type(const StringView& str);
PrimitiveType check_get_primitive_type_of_ident(const StringView& str);

bool check_ast(Ast* ast)
{
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

bool check_enum(const Ast_Enum_Declaration& decl, Ast* ast)
{
	//No variants
	if (decl.variants.empty())
	{
		printf("Enum must have at least 1 variant.\n"); 
		return false; 
	}

	//Typename is primitive type
	if (check_is_ident_a_primitive_type(decl.type.token.string_value))
	{
		printf("Enum type cannot use identifier of a primitive type.\n");
		return false;
	}

	//Typename is taken
	if (!check_is_ident_type_unique(decl.type.token.string_value, ast))
	{
		printf("Enum typename is taken by other type declaration.\n");
		return false;
	}

	for (u32 i = 0; i < decl.variants.size(); i++)
	{
		const IdentTypePair& check_ident = decl.variants[i];

		//Variant uses primitive type name @Design: not forced, avoiding confusing identifiers
		if (check_is_ident_a_primitive_type(check_ident.ident.token.string_value))
		{
			printf("Enum variant cannot use identifier of a primitive type.\n");
			return false;
		}

		//Repeating variant names
		for (u32 k = i + 1; k < decl.variants.size(); k++)
		{
			const IdentTypePair& ident = decl.variants[k];
			if (check_compare_ident(check_ident.ident.token.string_value, ident.ident.token.string_value))
			{
				printf("Enum has multiple variants with same identifier.\n");
				return false;
			}
		}
	}

	return true;
}

//@Incomplete: recursive infinite size detection
bool check_struct(const Ast_Struct_Declaration& decl, Ast* ast)
{
	//No fields
	if (decl.fields.empty())
	{
		printf("Struct must have at least 1 field.\n"); 
		return false;
	}

	//Typename is primitive type
	if (check_is_ident_a_primitive_type(decl.type.token.string_value))
	{
		printf("Struct type cannot use identifier of a primitive type.\n");
		return false;
	}

	//Typename is taken
	if (!check_is_ident_type_unique(decl.type.token.string_value, ast))
	{
		printf("Struct typename is taken by other type declaration.\n");
		return false;
	}

	for (u32 i = 0; i < decl.fields.size(); i++)
	{
		const IdentTypePair& ident = decl.fields[i];

		//Field uses primitive type name @Design: not forced, avoiding confusing identifiers
		if (check_is_ident_a_primitive_type(ident.ident.token.string_value))
		{
			printf("Struct field cannot use identifier of a primitive type.\n");
			return false;
		}

		//Repeating field names
		for (u32 k = i + 1; k < decl.fields.size(); k++)
		{
			const IdentTypePair& check_ident = decl.fields[k];
			if (check_compare_ident(check_ident.ident.token.string_value, ident.ident.token.string_value))
			{
				printf("Struct has multiple fields with same identifier.\n");
				return false;
			}
		}

		//Field type not in scope
		if (!check_is_ident_type_in_scope(ident.type.token.string_value, ast))
		{
			printf("Struct field type is not in scope.\n");
			return false;
		}
	}

	return true;
}

bool check_procedure(const Ast_Procedure_Declaration& decl, Ast* ast)
{
	//Procedure name is primitive type
	if (check_is_ident_a_primitive_type(decl.ident.token.string_value))
	{
		printf("Function cannot use identifier of a primitive type.\n");
		return false;
	}

	//Procedure name is taken
	for (const auto& proc_decl : ast->procedures)
	{
		if (check_compare_ident(decl.ident.token.string_value, proc_decl.ident.token.string_value)
			&& decl.ident.token.string_value.data != proc_decl.ident.token.string_value.data)
		{
			printf("Function name is defined multiple times.\n");
			return false;
		}
	}

	for (u32 i = 0; i < decl.input_parameters.size(); i++)
	{
		const IdentTypePair& ident = decl.input_parameters[i];

		//Parameter uses primitive type name @Design: not forced, avoiding confusing identifiers
		if (check_is_ident_a_primitive_type(ident.ident.token.string_value))
		{
			printf("Function parameter cannot use identifier of a primitive type.\n");
			return false;
		}

		//Repeating parameter names
		for (u32 k = i + 1; k < decl.input_parameters.size(); k++)
		{
			const IdentTypePair& check_ident = decl.input_parameters[k];
			if (check_compare_ident(check_ident.ident.token.string_value, ident.ident.token.string_value))
			{
				printf("Function has multiple parameters with same identifier.\n");
				return false;
			}
		}

		//Parameter type not in scope
		if (!check_is_ident_type_in_scope(ident.type.token.string_value, ast))
		{
			printf("Function parameter type is not in scope.\n");
			return false;
		}
	}

	//Return type not in scope
	if (decl.return_type.has_value() && !check_is_ident_type_in_scope(decl.return_type.value().token.string_value, ast))
	{
		printf("Function return type is not in scope.\n");
		return false;
	}

	return true;
}

bool check_procedure_block(const Ast_Procedure_Declaration& decl, Ast* ast)
{
	return true;
}

bool check_compare_ident(const StringView& str, const StringView& str2)
{
	if (str.count != str2.count) return false;
	for (u64 i = 0; i < str.count; i++)
	if (str.data[i] != str2.data[i]) return false;
	return true;
}

bool check_is_ident_type_unique(const StringView& str, Ast* ast)
{
	for (const auto& struct_decl : ast->structs)
	{
		if (check_compare_ident(str, struct_decl.type.token.string_value)
			&& str.data != struct_decl.type.token.string_value.data) 
			return false;
	}
	for (const auto& enum_decl : ast->enums)
	{
		if (check_compare_ident(str, enum_decl.type.token.string_value)
			&& str.data != enum_decl.type.token.string_value.data)
			return false;
	}
	return true;
}

bool check_is_ident_type_in_scope(const StringView& str, Ast* ast)
{
	if (check_is_ident_a_primitive_type(str)) return true;

	bool type_in_scope = false;

	for (const auto& struct_decl : ast->structs)
	{
		if (check_compare_ident(str, struct_decl.type.token.string_value))
		{
			type_in_scope = true;
			break;
		}
	}

	if (!type_in_scope)
	{
		for (const auto& enum_decl : ast->enums)
		{
			if (check_compare_ident(str, enum_decl.type.token.string_value))
			{
				type_in_scope = true;
				break;
			}
		}
	}

	return type_in_scope;
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
	NotPrimitive,
};

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

bool check_is_ident_a_primitive_type(const StringView& str)
{
	return check_get_primitive_type_of_ident(str) != PrimitiveType::NotPrimitive;
}

PrimitiveType check_get_primitive_type_of_ident(const StringView& str)
{
	if (str.count > 4) return PrimitiveType::NotPrimitive;
	u64 hash = string_hash_ascii_9(str);
	bool is_primitive_type = ident_hash_to_primitive_type.find(hash) != ident_hash_to_primitive_type.end();
	return is_primitive_type ? ident_hash_to_primitive_type.at(hash) : PrimitiveType::NotPrimitive;
}
