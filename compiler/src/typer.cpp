
enum Primitive_Type
{
	TYPE_I8,   TYPE_U8,
	TYPE_I16,  TYPE_U16,
	TYPE_I32,  TYPE_U32,
	TYPE_I64,  TYPE_U64,
	TYPE_F32,  TYPE_F64,
	TYPE_BOOL, TYPE_STRING,
};

enum Type_Tag
{
	TYPE_TAG_STRUCT,
	TYPE_TAG_ENUM,
	TYPE_TAG_PRIMITIVE,
};

struct Type_Info
{
	Type_Tag tag;
	u64 runtime_size;
	
	union
	{
		Ast_Struct_Decl* as_struct_decl;
		Ast_Enum_Decl* as_enum_decl;
		Primitive_Type as_primitive_type;
	};

	bool is_user_defined() { return tag != TYPE_TAG_PRIMITIVE; }
	bool is_uint() { return (as_primitive_type <= TYPE_U64) && (as_primitive_type % 2 == 1); }
	bool is_float() { return as_primitive_type == TYPE_F32 || as_primitive_type == TYPE_F64; }
	bool is_bool() { return as_primitive_type == TYPE_BOOL; }
};

struct Typer
{
	void init_primitive_types();
	void add_struct_type(Ast_Struct_Decl* struct_decl);
	void add_enum_type(Ast_Enum_Decl* enum_decl);
	bool is_type_in_scope(Ast_Ident* type_ident);
	Type_Info get_type_info(Ast_Ident* type_ident);
	Type_Info get_primitive_type_info(Primitive_Type type);
	bool is_type_equals_type(Type_Info* t, Type_Info* t_other);

	void debug_print_type_info(Type_Info* t);

	HashTable<StringView, Type_Info, u32, match_string_view> type_table;
	Type_Info primitive_type_table[12];
};

void Typer::init_primitive_types()
{
	type_table.init(4);
	Type_Info info = { TYPE_TAG_PRIMITIVE };
	info.runtime_size = 1;
	info.as_primitive_type = TYPE_I8;
	primitive_type_table[TYPE_I8] = info;
	info.runtime_size = 1;
	info.as_primitive_type = TYPE_U8;
	primitive_type_table[TYPE_U8] = info;
	info.runtime_size = 2;
	info.as_primitive_type = TYPE_I16;
	primitive_type_table[TYPE_I16] = info;
	info.runtime_size = 2;
	info.as_primitive_type = TYPE_U16;
	primitive_type_table[TYPE_U16] = info;
	info.runtime_size = 4;
	info.as_primitive_type = TYPE_I32;
	primitive_type_table[TYPE_I32] = info;
	info.runtime_size = 4;
	info.as_primitive_type = TYPE_U32;
	primitive_type_table[TYPE_U32] = info;
	info.runtime_size = 8;
	info.as_primitive_type = TYPE_I64;
	primitive_type_table[TYPE_I64] = info;
	info.runtime_size = 8;
	info.as_primitive_type = TYPE_U64;
	primitive_type_table[TYPE_U64] = info;
	info.runtime_size = 4;
	info.as_primitive_type = TYPE_F32;
	primitive_type_table[TYPE_F32] = info;
	info.runtime_size = 8;
	info.as_primitive_type = TYPE_F64;
	primitive_type_table[TYPE_F64] = info;
	info.runtime_size = 1;
	info.as_primitive_type = TYPE_BOOL;
	primitive_type_table[TYPE_BOOL] = info;
	info.runtime_size = 1; //@Not set yet
	info.as_primitive_type = TYPE_STRING;
	primitive_type_table[TYPE_STRING] = info;
}

void Typer::add_struct_type(Ast_Struct_Decl* struct_decl)
{
	Type_Info info = { TYPE_TAG_STRUCT };
	info.runtime_size = 0; //@Not doing sizing yet
	info.as_struct_decl = struct_decl;

	type_table.add(struct_decl->type.token.string_value, info, hash_fnv1a_32(struct_decl->type.token.string_value));
}

void Typer::add_enum_type(Ast_Enum_Decl* enum_decl)
{
	Type_Info info = { TYPE_TAG_ENUM };
	info.runtime_size = 0; //@Not doing sizing yet
	info.as_enum_decl = enum_decl;

	type_table.add(enum_decl->type.token.string_value, info, hash_fnv1a_32(enum_decl->type.token.string_value));
}

bool Typer::is_type_in_scope(Ast_Ident* type_ident)
{
	TokenType token_type = type_ident->token.type;
	if (token_type >= TOKEN_TYPE_I8 && token_type <= TOKEN_TYPE_STRING) 
	return true;
	return type_table.find(type_ident->token.string_value, hash_fnv1a_32(type_ident->token.string_value)).has_value();
}

Type_Info Typer::get_type_info(Ast_Ident* type_ident)
{
	TokenType token_type = type_ident->token.type;
	if (token_type >= TOKEN_TYPE_I8 && token_type <= TOKEN_TYPE_STRING)
	return primitive_type_table[token_type - TOKEN_TYPE_I8];
	return type_table.find(type_ident->token.string_value, hash_fnv1a_32(type_ident->token.string_value)).value();
}

Type_Info Typer::get_primitive_type_info(Primitive_Type type)
{
	return primitive_type_table[type];
}

bool Typer::is_type_equals_type(Type_Info* t, Type_Info* t_other) //@Should work, all we need is to compare the memory of the union
{
	return (t->tag == t_other->tag) && (t->as_struct_decl == t_other->as_struct_decl);
}

void Typer::debug_print_type_info(Type_Info* t)
{
	switch (t->tag)
	{
		case TYPE_TAG_STRUCT:
		{
			debug_print_token(t->as_struct_decl->type.token, true, false);
		} break;
		case TYPE_TAG_ENUM:
		{
			debug_print_token(t->as_enum_decl->type.token, true, false);
		} break;
		case TYPE_TAG_PRIMITIVE:
		{
			Token token = {};
			token.type = TokenType(t->as_primitive_type + TOKEN_TYPE_I8);
			debug_print_token(token, true, false);
		} break;
	}
}
