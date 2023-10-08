#include "ast.h"

Ast_Ident token_to_ident(Token& token)
{
	return Ast_Ident{ token.l0, token.c0, token.string_value };
}

u32 hash_ident(Ast_Ident& ident)
{
	return hash_fnv1a_32(ident.str);
}

bool match_ident(Ast_Ident& a, Ast_Ident& b)
{
	return a.str == b.str;
}
