export module printer;

import general;
import ast;
import token;

struct Loc;

export void print_span_context(Ast* source, Span span);
export void print_str(StringView str);
export void print_type(Ast_Type type);
export void print_token_type(TokenType type);

Loc print_loc_from_span(Ast* source, Span span);
void print_basic_type(BasicType type);

module : private;

struct Loc
{
	u32 line;
	u32 col;
	Span span;
};

void print_span_context(Ast* source, Span span)
{
	Loc loc = print_loc_from_span(source, span);
	printf("%s", source->source->filepath.c_str());
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
		u8 c = source->source->str.data[i];
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
		u8 c = source->source->str.data[cursor];
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
	
	switch (type.tag())
	{
	case Ast_Type::Tag::Basic: print_basic_type(type.as_basic); break;
	case Ast_Type::Tag::Enum: print_str(type.as_enum.enum_decl->ident.str); break;
	case Ast_Type::Tag::Struct: print_str(type.as_struct.struct_decl->ident.str); break;
	case Ast_Type::Tag::Array:
	{
		Ast_Type_Array* array_type = type.as_array;
		if (array_type->size_expr->tag() != Ast_Expr::Tag::Folded) printf("[size expr]");
		else printf("[%llu]", array_type->size_expr->as_folded_expr.as_u64);
		print_type(array_type->element_type);
	} break;
	case Ast_Type::Tag::Procedure:
	{
		Ast_Type_Procedure* procedure = type.as_procedure;
		printf("(");
		for (u32 i = 0; i < procedure->input_types.size(); i += 1)
		{
			print_type(procedure->input_types[i]);
			if ((i + 1) != procedure->input_types.size()) printf(", ");
		}
		printf(")");
		if (procedure->return_type)
		{
			printf("::");
			print_type(procedure->return_type.value());
		}
	} break;
	case Ast_Type::Tag::Unresolved:
	{
		if (type.as_unresolved->module_access)
		{
			for (Ast_Ident ident : type.as_unresolved->module_access.value()->modules)
			{
				print_str(ident.str);
				printf("::");
			}
		}
		print_str(type.as_unresolved->ident.str);
	} break;
	case Ast_Type::Tag::Poison: printf("'Poison'"); break;
	default: break; //@enum err
	}
}

void print_token_type(TokenType type)
{
	switch (type)
	{
	case TokenType::IDENT:                 printf("identifier"); break;
	case TokenType::LITERAL_INT:           printf("literal int"); break;
	case TokenType::LITERAL_FLOAT:         printf("literal float"); break;
	case TokenType::LITERAL_BOOL:          printf("literal bool"); break;
	case TokenType::LITERAL_STRING:        printf("literal string"); break;

	case TokenType::KEYWORD_SELF:          printf("self"); break;
	case TokenType::KEYWORD_IMPL:          printf("impl"); break;
	case TokenType::KEYWORD_ENUM:          printf("enum"); break;
	case TokenType::KEYWORD_STRUCT:        printf("struct"); break;
	case TokenType::KEYWORD_IMPORT:        printf("import"); break;

	case TokenType::KEYWORD_IF:            printf("if"); break;
	case TokenType::KEYWORD_ELSE:          printf("else"); break;
	case TokenType::KEYWORD_FOR:           printf("for"); break;
	case TokenType::KEYWORD_DEFER:         printf("defer"); break;
	case TokenType::KEYWORD_BREAK:         printf("break"); break;
	case TokenType::KEYWORD_RETURN:        printf("return"); break;
	case TokenType::KEYWORD_SWITCH:        printf("switch"); break;
	case TokenType::KEYWORD_CONTINUE:      printf("continue"); break;

	case TokenType::KEYWORD_MUT:           printf("mut"); break;
	case TokenType::KEYWORD_CAST:          printf("cast"); break;
	case TokenType::KEYWORD_SIZEOF:        printf("sizeof"); break;
	case TokenType::KEYWORD_TRUE:          printf("true"); break;
	case TokenType::KEYWORD_FALSE:         printf("false"); break;

	case TokenType::KEYWORD_I8:            printf("i8"); break;
	case TokenType::KEYWORD_I16:           printf("i16"); break;
	case TokenType::KEYWORD_I32:           printf("i32"); break;
	case TokenType::KEYWORD_I64:           printf("i64"); break;
	case TokenType::KEYWORD_U8:            printf("u8"); break;
	case TokenType::KEYWORD_U16:           printf("u16"); break;
	case TokenType::KEYWORD_U32:           printf("u32"); break;
	case TokenType::KEYWORD_U64:           printf("u64"); break;
	case TokenType::KEYWORD_F32:           printf("f32"); break;
	case TokenType::KEYWORD_F64:           printf("f64"); break;
	case TokenType::KEYWORD_BOOL:          printf("bool"); break;
	case TokenType::KEYWORD_STRING:        printf("string"); break;

	case TokenType::AT:                    printf("@"); break;
	case TokenType::DOT:                   printf("."); break;
	case TokenType::COLON:                 printf(":"); break;
	case TokenType::COMMA:                 printf(","); break;
	case TokenType::SEMICOLON:             printf(";"); break;
	case TokenType::DOUBLE_DOT:            printf(".."); break;
	case TokenType::DOUBLE_COLON:          printf("::"); break;
	case TokenType::ARROW_THIN:            printf("->"); break;
	case TokenType::ARROW_WIDE:            printf("=>"); break;
	case TokenType::PAREN_START:           printf("("); break;
	case TokenType::PAREN_END:             printf(")"); break;
	case TokenType::BLOCK_START:           printf("{"); break;
	case TokenType::BLOCK_END:             printf("}"); break;
	case TokenType::BRACKET_START:         printf("["); break;
	case TokenType::BRACKET_END:           printf("]"); break;

	case TokenType::LOGIC_NOT:             printf("!"); break;
	case TokenType::BITWISE_NOT:           printf("~"); break;

	case TokenType::LOGIC_AND:             printf("&&"); break;
	case TokenType::LOGIC_OR:              printf("||"); break;
	case TokenType::LESS:                  printf("<"); break;
	case TokenType::GREATER:               printf(">"); break;
	case TokenType::LESS_EQUALS:           printf("<="); break;
	case TokenType::GREATER_EQUALS:        printf(">="); break;
	case TokenType::IS_EQUALS:             printf("=="); break;
	case TokenType::NOT_EQUALS:            printf("!="); break;

	case TokenType::PLUS:                  printf("+"); break;
	case TokenType::MINUS:                 printf("-"); break;
	case TokenType::TIMES:                 printf("*"); break;
	case TokenType::DIV:                   printf("/"); break;
	case TokenType::MOD:                   printf("%%"); break;
	case TokenType::BITWISE_AND:           printf("&"); break;
	case TokenType::BITWISE_OR:            printf("|"); break;
	case TokenType::BITWISE_XOR:           printf("^"); break;
	case TokenType::BITSHIFT_LEFT:         printf("<<"); break;
	case TokenType::BITSHIFT_RIGHT:        printf(">>"); break;

	case TokenType::ASSIGN:                printf("="); break;
	case TokenType::PLUS_EQUALS:           printf("+="); break;
	case TokenType::MINUS_EQUALS:          printf("-="); break;
	case TokenType::TIMES_EQUALS:          printf("*="); break;
	case TokenType::DIV_EQUALS:            printf("/="); break;
	case TokenType::MOD_EQUALS:            printf("%%="); break;
	case TokenType::BITWISE_AND_EQUALS:    printf("&="); break;
	case TokenType::BITWISE_OR_EQUALS:     printf("|="); break;
	case TokenType::BITWISE_XOR_EQUALS:    printf("^="); break;
	case TokenType::BITSHIFT_LEFT_EQUALS:  printf("^="); break;
	case TokenType::BITSHIFT_RIGHT_EQUALS: printf("^="); break;

	case TokenType::ERROR:                 printf("error token"); break;
	case TokenType::INPUT_END:             printf("end of file"); break;
	default: break; //@enum err
	}
}

Loc print_loc_from_span(Ast* source, Span span)
{
	Loc loc = { 1, 1 };

	for (Span line_span : source->source->line_spans)
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
	case BasicType::I8:     printf("i8"); break;
	case BasicType::I16:    printf("i16"); break;
	case BasicType::I32:    printf("i32"); break;
	case BasicType::I64:    printf("i64"); break;
	case BasicType::U8:     printf("u8"); break;
	case BasicType::U16:    printf("u16"); break;
	case BasicType::U32:    printf("u32"); break;
	case BasicType::U64:    printf("u64"); break;
	case BasicType::F32:    printf("f32"); break;
	case BasicType::F64:    printf("f64"); break;
	case BasicType::BOOL:   printf("bool"); break;
	case BasicType::STRING: printf("string"); break;
	default: break; //@enum err
	}
}
