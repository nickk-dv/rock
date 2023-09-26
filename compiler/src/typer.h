#ifndef TYPER_H
#define TYPER_H

#include "common.h"
#include "ast.h"

enum Primitive_Type;
enum Type_Tag;
struct Type_Info;
struct Typer;

enum Primitive_Type
{
	TYPE_I8, TYPE_U8,
	TYPE_I16, TYPE_U16,
	TYPE_I32, TYPE_U32,
	TYPE_I64, TYPE_U64,
	TYPE_F32, TYPE_F64,
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
public:
	void init_primitive_types();
	void add_struct_type(Ast_Struct_Decl* struct_decl);
	void add_enum_type(Ast_Enum_Decl* enum_decl);
	bool is_type_in_scope(Ast_Ident* type_ident);
	Type_Info get_type_info(Ast_Ident* type_ident);
	Type_Info get_primitive_type_info(Primitive_Type type);
	bool is_type_equals_type(Type_Info* t, Type_Info* t_other);
	void debug_print_type_info(Type_Info* t);

private:
	HashMap<StringView, Type_Info, u32, match_string_view> type_table;
	Type_Info primitive_type_table[12];
};

#endif
