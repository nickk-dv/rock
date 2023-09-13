
bool check_ast(Ast* ast);
bool check_enum(const Ast_Enum_Declaration& decl);
bool check_struct(const Ast_Struct_Declaration& decl);
bool check_procedure(const Ast_Procedure_Declaration& decl);
enum class PrimitiveType;
PrimitiveType get_primitive_type_of_ident(const StringView& str);

bool check_ast(Ast* ast)
{
	for (const auto& decl : ast->enums)
	if (!check_enum(decl)) return false;

	for (const auto& decl : ast->structs)
	if (!check_struct(decl)) return false;

	for (const auto& decl : ast->procedures)
	if (!check_procedure(decl)) return false;

	return true;
}

bool check_enum(const Ast_Enum_Declaration& decl)
{
	if (decl.variants.empty())
	{
		printf("Enum must have at least 1 variant."); 
		return false; 
	}

	return true;
}

bool check_struct(const Ast_Struct_Declaration& decl)
{
	if (decl.fields.empty())
	{
		printf("Struct must have at least 1 field."); 
		return false;
	}

	return true;
}

bool check_procedure(const Ast_Procedure_Declaration& decl)
{
	return true;
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

PrimitiveType get_primitive_type_of_ident(const StringView& str)
{
	if (str.count > 4) return PrimitiveType::NotPrimitive;
	u64 hash = string_hash_ascii_9(str);
	bool is_primitive_type = ident_hash_to_primitive_type.find(hash) != ident_hash_to_primitive_type.end();
	return is_primitive_type ? ident_hash_to_primitive_type.at(hash) : PrimitiveType::NotPrimitive;
}
