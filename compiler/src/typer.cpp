/*

What type needs to have:

identifier
runtime_size
pointers to enum / struct decl

*/

enum Type_Tag
{
	TYPE_TAG_ENUM,
	TYPE_TAG_STRUCT,
};

struct Type_Info
{
	Type_Tag tag;
	u64 runtime_size;
	
	union
	{
		Ast_Enum_Declaration* as_enum_decl;
		Ast_Struct_Declaration* as_struct_decl;
	};
};

//@Q maybe should use some sorf of type_ids and linear type tables
//@Q On which stage and where in Ast to represent pointers to a type
struct Typer
{
	std::unordered_map<StringView, Type_Info, StringViewHasher> type_table;
};
