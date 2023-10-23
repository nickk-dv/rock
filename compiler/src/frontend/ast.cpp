#include "ast.h"

Ast_Ident token_to_ident(const Token& token)
{
	return Ast_Ident { token.span, token.string_value };
}

u32 hash_ident(Ast_Ident& ident)
{
	return hash_fnv1a_32(ident.str);
}

bool match_ident(Ast_Ident& a, Ast_Ident& b)
{
	return match_string_view(a.str, b.str);
}

bool match_string(std::string& a, std::string& b)
{
	return a == b;
}
