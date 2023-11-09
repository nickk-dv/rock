#ifndef PRINTER_H
#define PRINTER_H

#include "ast.h"

struct Loc;

void print_span_context(Ast* source, Span span);
void print_str(StringView str);
void print_type(Ast_Type type);
void print_token_type(TokenType type);

static Loc print_loc_from_span(Ast* source, Span span);
static void print_basic_type(BasicType type);

struct Loc
{
	u32 line;
	u32 col;
	Span span;
};

#endif
