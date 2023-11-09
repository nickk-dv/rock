#include "printer.h"

void print_span_context(Ast* source, Span span)
{
	Loc loc = print_loc_from_span(source, span);
	printf("%s", source->filepath.c_str());
	printf(" %lu:%lu\n", loc.line, loc.col);
	
	u32 bar_offset = 1;
	{
		u32 line = loc.line;
		while (line > 0)
		{
			line /= 10;
			bar_offset += 1;
		}
	}

	for (u32 i = 0; i < bar_offset; i += 1) printf(" ");
	printf("|\n");
	printf("%lu |", loc.line);

	bool has_endl = false;
	for (u32 i = loc.span.start; i <= loc.span.end; i += 1)
	{
		u8 c = source->source.data[i];
		if (c == '\t') printf(" ");
		else printf("%c", c);
		if (c == '\n') has_endl = true;
	}
	if (!has_endl) printf("\n");

	for (u32 i = 0; i < bar_offset; i += 1) printf(" ");
	printf("|");
	bool multiline = span.end > loc.span.end;
	if (multiline) span.end = loc.span.end;
	u32 offset = span.start - loc.span.start;
	u32 width = span.end - span.start;
	
	u32 cursor = span.end;
	while (cursor != span.start)
	{
		u8 c = source->source.data[cursor];
		if (c == ' ' || c == '\r' || c == '\n') width -= 1;
		else break;
		cursor -= 1;
	}

	for (u32 i = 0; i < offset; i += 1) printf(" ");
	for (u32 i = 0; i <= width; i += 1) printf("^");
	printf("\n");

	if (multiline)
	{
		for (u32 i = 0; i < bar_offset; i += 1) printf(" ");
		printf("| ...\n");
	}
}

void print_str(StringView str)
{
	for (u32 i = 0; i < str.count; i += 1)
	{
		printf("%c", str.data[i]);
	}
}

void print_type(Ast_Type type)
{
	for (u32 i = 0; i < type.pointer_level; i += 1) printf("*");
	
	switch (type.tag)
	{
	case Ast_Type_Tag::Basic: print_basic_type(type.as_basic); break;
	case Ast_Type_Tag::Array:
	{
		//@Fold size expr using i64 instead of u64 termporarely
		if (type.as_array->size_expr->tag != Ast_Expr_Tag::Folded_Expr) printf("[size expr]");
		else printf("[%llu]", (u64)type.as_array->size_expr->as_folded_expr.as_i64);
		print_type(type.as_array->element_type);
	} break;
	case Ast_Type_Tag::Struct: print_str(type.as_struct.struct_decl->ident.str); break;
	case Ast_Type_Tag::Enum: print_str(type.as_enum.enum_decl->ident.str); break;
	case Ast_Type_Tag::Unresolved:
	{
		if (type.as_unresolved->import)
		{
			print_str(type.as_unresolved->import.value().str);
			printf(".");
		}
		print_str(type.as_unresolved->ident.str);
	} break;
	case Ast_Type_Tag::Poison: printf("'Poison'"); break;
	default: err_internal("print_type: invalid Ast_Type_Tag"); break;
	}
}

void print_token_type(TokenType type)
{
	switch (type)
	{
	case TokenType::IDENT: printf("identifier"); break;
	case TokenType::BOOL_LITERAL: printf("bool literal"); break;
	case TokenType::FLOAT_LITERAL: printf("float literal"); break;
	case TokenType::INTEGER_LITERAL: printf("integer literal"); break;
	case TokenType::STRING_LITERAL: printf("string literal"); break;

	case TokenType::KEYWORD_STRUCT: printf("struct"); break;
	case TokenType::KEYWORD_ENUM: printf("enum"); break;
	case TokenType::KEYWORD_IF: printf("if"); break;
	case TokenType::KEYWORD_ELSE: printf("else"); break;
	case TokenType::KEYWORD_TRUE: printf("true"); break;
	case TokenType::KEYWORD_FALSE: printf("false"); break;
	case TokenType::KEYWORD_FOR: printf("for"); break;
	case TokenType::KEYWORD_DEFER: printf("defer"); break;
	case TokenType::KEYWORD_BREAK: printf("break"); break;
	case TokenType::KEYWORD_RETURN: printf("return"); break;
	case TokenType::KEYWORD_SWITCH: printf("switch"); break;
	case TokenType::KEYWORD_CONTINUE: printf("continue"); break;
	case TokenType::KEYWORD_SIZEOF: printf("sizeof"); break;
	case TokenType::KEYWORD_IMPORT: printf("import"); break;
	case TokenType::KEYWORD_USE: printf("use"); break;

	case TokenType::TYPE_I8: printf("i8"); break;
	case TokenType::TYPE_U8: printf("u8"); break;
	case TokenType::TYPE_I16: printf("i16"); break;
	case TokenType::TYPE_U16: printf("u16"); break;
	case TokenType::TYPE_I32: printf("i32"); break;
	case TokenType::TYPE_U32: printf("u32"); break;
	case TokenType::TYPE_I64: printf("i64"); break;
	case TokenType::TYPE_U64: printf("u64"); break;
	case TokenType::TYPE_F32: printf("f32"); break;
	case TokenType::TYPE_F64: printf("f64"); break;
	case TokenType::TYPE_BOOL: printf("bool"); break;
	case TokenType::TYPE_STRING: printf("string"); break;

	case TokenType::DOT: printf("."); break;
	case TokenType::COLON: printf(":"); break;
	case TokenType::COMMA: printf(","); break;
	case TokenType::SEMICOLON: printf(";"); break;
	case TokenType::DOUBLE_DOT: printf(".."); break;
	case TokenType::DOUBLE_COLON: printf("::"); break;
	case TokenType::BLOCK_START: printf("{"); break;
	case TokenType::BLOCK_END: printf("}"); break;
	case TokenType::BRACKET_START: printf("["); break;
	case TokenType::BRACKET_END: printf("]"); break;
	case TokenType::PAREN_START: printf("("); break;
	case TokenType::PAREN_END: printf(")"); break;
	case TokenType::AT: printf("@"); break;
	case TokenType::HASH: printf("#"); break;
	case TokenType::QUESTION: printf("?"); break;

	case TokenType::ASSIGN: printf("="); break;
	case TokenType::PLUS: printf("+"); break;
	case TokenType::MINUS: printf("-"); break;
	case TokenType::TIMES: printf("*"); break;
	case TokenType::DIV: printf("/"); break;
	case TokenType::MOD: printf("%%"); break;
	case TokenType::BITWISE_AND: printf("&"); break;
	case TokenType::BITWISE_OR: printf("|"); break;
	case TokenType::BITWISE_XOR: printf("^"); break;
	case TokenType::LESS: printf("<"); break;
	case TokenType::GREATER: printf(">"); break;
	case TokenType::LOGIC_NOT: printf("!"); break;
	case TokenType::IS_EQUALS: printf("=="); break;
	case TokenType::PLUS_EQUALS: printf("+="); break;
	case TokenType::MINUS_EQUALS: printf("-="); break;
	case TokenType::TIMES_EQUALS: printf("*="); break;
	case TokenType::DIV_EQUALS: printf("/="); break;
	case TokenType::MOD_EQUALS: printf("%%="); break;
	case TokenType::BITWISE_AND_EQUALS: printf("&="); break;
	case TokenType::BITWISE_OR_EQUALS: printf("|="); break;
	case TokenType::BITWISE_XOR_EQUALS: printf("^="); break;
	case TokenType::LESS_EQUALS: printf("<="); break;
	case TokenType::GREATER_EQUALS: printf(">="); break;
	case TokenType::NOT_EQUALS: printf("!="); break;
	case TokenType::LOGIC_AND: printf("&&"); break;
	case TokenType::LOGIC_OR: printf("||"); break;
	case TokenType::BITWISE_NOT: printf("~"); break;
	case TokenType::BITSHIFT_LEFT: printf("<<"); break;
	case TokenType::BITSHIFT_RIGHT: printf(">>"); break;

	case TokenType::ERROR: printf("Token Error"); break;
	case TokenType::INPUT_END: printf("End of file"); break;
	default: err_internal("print_token_type: invalid TokenType");
	}
}

Loc print_loc_from_span(Ast* source, Span span)
{
	Loc loc = { 1, 1 };

	for (Span line_span : source->line_spans)
	{
		if (span.start >= line_span.start && span.start <= line_span.end)
		{
			loc.col += span.start - line_span.start;
			loc.span = line_span;
			break;
		}
		else loc.line += 1; 
	}

	return loc;
}

void print_basic_type(BasicType type)
{
	switch (type)
	{
	case BasicType::I8: printf("i8"); break;
	case BasicType::U8: printf("u8"); break;
	case BasicType::I16: printf("i16"); break;
	case BasicType::U16: printf("u16"); break;
	case BasicType::I32: printf("i32"); break;
	case BasicType::U32: printf("u32"); break;
	case BasicType::I64: printf("i64"); break;
	case BasicType::U64: printf("u64"); break;
	case BasicType::F32: printf("f32"); break;
	case BasicType::F64: printf("f64"); break;
	case BasicType::BOOL: printf("bool"); break;
	case BasicType::STRING: printf("string"); break;
	default: err_internal("print_basic_type: invalid BasicType"); break;
	}
}
