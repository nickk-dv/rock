
enum class Primitive_Type
{
	s8_t, u8_t,
	s16_t, u16_t,
	s32_t, u32_t,
	s64_t, u64_t,
	f32_t, f64_t,
	bool_t, string_t,
};

enum Type_Tag
{
	TYPE_TAG_PRIMITIVE,
	TYPE_TAG_ENUM,
	TYPE_TAG_STRUCT,
};

struct Type_Info
{
	Type_Tag tag;
	u64 runtime_size;
	
	union
	{
		Primitive_Type as_primitive;
		Ast_Enum_Declaration* as_enum_decl;
		Ast_Struct_Declaration* as_struct_decl;
	};
};

//@Q maybe should use some sorf of type_ids and linear type tables
//@Q On which stage and where in Ast to represent pointers to a type
struct Typer
{
	void init();
	bool is_type_in_scope(Ast_Identifier* ident);
	void add_type_info(Ast_Identifier* ident, Type_Info type_info);
	void add_struct_type(Ast_Struct_Declaration* struct_decl);
	void add_enum_type(Ast_Enum_Declaration* enum_decl);
	Type_Info get_type_info(Ast_Identifier* ident);

	std::unordered_map<StringView, Type_Info, StringViewHasher> type_table;
};

void Typer::init()
{
	//@Issue cant store basic type info with StringView keys
}

bool Typer::is_type_in_scope(Ast_Identifier* type_ident)
{
	return type_table.find(type_ident->token.string_value) != type_table.end();
}

void Typer::add_type_info(Ast_Identifier* type_ident, Type_Info type_info)
{
	type_table.emplace(type_ident->token.string_value, type_info);
}

void Typer::add_struct_type(Ast_Struct_Declaration* struct_decl)
{
	Type_Info info = { TYPE_TAG_STRUCT };
	info.runtime_size = 0; //@Not doing sizing yet
	info.as_struct_decl = struct_decl;
	add_type_info(&struct_decl->type, info);
}

void Typer::add_enum_type(Ast_Enum_Declaration* enum_decl)
{
	Type_Info info = { TYPE_TAG_ENUM };
	info.runtime_size = 0; //@Not doing sizing yet
	info.as_enum_decl = enum_decl;
	add_type_info(&enum_decl->type, info);
}

Type_Info Typer::get_type_info(Ast_Identifier* ident)
{
	return type_table.at(ident->token.string_value);
}
