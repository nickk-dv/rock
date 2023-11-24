#ifndef FMT_H
#define FMT_H

#include "ast.h"

void fmt_endl()
{
	printf("\n");
}

void fmt_tab()
{
	printf("\t");
}

void fmt_space()
{
	printf(" ");
}

void fmt_token(TokenType type)
{
	switch (type)
	{
	case TokenType::IDENT:              printf("identifier"); break; //@dont print (use token span instead)
	case TokenType::BOOL_LITERAL:       printf("bool literal"); break; //@dont print (use token span instead)
	case TokenType::FLOAT_LITERAL:      printf("float literal"); break; //@dont print (use token span instead)
	case TokenType::INTEGER_LITERAL:    printf("integer literal"); break; //@dont print (use token span instead)
	case TokenType::STRING_LITERAL:     printf("string literal"); break; //@dont print (use token span instead)

	case TokenType::KEYWORD_STRUCT:     printf("struct"); break;
	case TokenType::KEYWORD_ENUM:       printf("enum"); break;
	case TokenType::KEYWORD_IF:         printf("if"); break;
	case TokenType::KEYWORD_ELSE:       printf("else"); break;
	case TokenType::KEYWORD_TRUE:       printf("true"); break;
	case TokenType::KEYWORD_FALSE:      printf("false"); break;
	case TokenType::KEYWORD_FOR:        printf("for"); break;
	case TokenType::KEYWORD_DEFER:      printf("defer"); break;
	case TokenType::KEYWORD_BREAK:      printf("break"); break;
	case TokenType::KEYWORD_RETURN:     printf("return"); break;
	case TokenType::KEYWORD_SWITCH:     printf("switch"); break;
	case TokenType::KEYWORD_CONTINUE:   printf("continue"); break;
	case TokenType::KEYWORD_SIZEOF:     printf("sizeof"); break;
	case TokenType::KEYWORD_IMPORT:     printf("import"); break;
	case TokenType::KEYWORD_IMPL:       printf("impl"); break;
	case TokenType::KEYWORD_SELF:       printf("self"); break;

	case TokenType::TYPE_I8:            printf("i8"); break;
	case TokenType::TYPE_U8:            printf("u8"); break;
	case TokenType::TYPE_I16:           printf("i16"); break;
	case TokenType::TYPE_U16:           printf("u16"); break;
	case TokenType::TYPE_I32:           printf("i32"); break;
	case TokenType::TYPE_U32:           printf("u32"); break;
	case TokenType::TYPE_I64:           printf("i64"); break;
	case TokenType::TYPE_U64:           printf("u64"); break;
	case TokenType::TYPE_F32:           printf("f32"); break;
	case TokenType::TYPE_F64:           printf("f64"); break;
	case TokenType::TYPE_BOOL:          printf("bool"); break;
	case TokenType::TYPE_STRING:        printf("string"); break;

	case TokenType::DOT:                printf("."); break;
	case TokenType::COLON:              printf(":"); break;
	case TokenType::COMMA:              printf(","); break;
	case TokenType::SEMICOLON:          printf(";"); break;
	case TokenType::DOUBLE_DOT:         printf(".."); break;
	case TokenType::DOUBLE_COLON:       printf("::"); break;
	case TokenType::BLOCK_START:        printf("{"); break;
	case TokenType::BLOCK_END:          printf("}"); break;
	case TokenType::BRACKET_START:      printf("["); break;
	case TokenType::BRACKET_END:        printf("]"); break;
	case TokenType::PAREN_START:        printf("("); break;
	case TokenType::PAREN_END:          printf(")"); break;
	case TokenType::AT:                 printf("@"); break;
	case TokenType::HASH:               printf("#"); break;
	case TokenType::QUESTION:           printf("?"); break;

	case TokenType::ARROW:              printf("->"); break;
	case TokenType::ASSIGN:             printf("="); break;
	case TokenType::PLUS:               printf("+"); break;
	case TokenType::MINUS:              printf("-"); break;
	case TokenType::TIMES:              printf("*"); break;
	case TokenType::DIV:                printf("/"); break;
	case TokenType::MOD:                printf("%%"); break;
	case TokenType::BITWISE_AND:        printf("&"); break;
	case TokenType::BITWISE_OR:         printf("|"); break;
	case TokenType::BITWISE_XOR:        printf("^"); break;
	case TokenType::LESS:               printf("<"); break;
	case TokenType::GREATER:            printf(">"); break;
	case TokenType::LOGIC_NOT:          printf("!"); break;
	case TokenType::IS_EQUALS:          printf("=="); break;
	case TokenType::PLUS_EQUALS:        printf("+="); break;
	case TokenType::MINUS_EQUALS:       printf("-="); break;
	case TokenType::TIMES_EQUALS:       printf("*="); break;
	case TokenType::DIV_EQUALS:         printf("/="); break;
	case TokenType::MOD_EQUALS:         printf("%%="); break;
	case TokenType::BITWISE_AND_EQUALS: printf("&="); break;
	case TokenType::BITWISE_OR_EQUALS:  printf("|="); break;
	case TokenType::BITWISE_XOR_EQUALS: printf("^="); break;
	case TokenType::LESS_EQUALS:        printf("<="); break;
	case TokenType::GREATER_EQUALS:     printf(">="); break;
	case TokenType::NOT_EQUALS:         printf("!="); break;
	case TokenType::LOGIC_AND:          printf("&&"); break;
	case TokenType::LOGIC_OR:           printf("||"); break;
	case TokenType::BITWISE_NOT:        printf("~"); break;
	case TokenType::BITSHIFT_LEFT:      printf("<<"); break;
	case TokenType::BITSHIFT_RIGHT:     printf(">>"); break;

	case TokenType::ERROR:     printf("error token"); break; //@dont print
	case TokenType::INPUT_END: printf("end of file"); break; //@dont print
	default: err_internal_enum(type); break;
	}
}

void fmt_unary_op(UnaryOp op)
{
	switch (op)
	{
	case UnaryOp::MINUS:       fmt_token(TokenType::MINUS); break;
	case UnaryOp::LOGIC_NOT:   fmt_token(TokenType::LOGIC_NOT); break;
	case UnaryOp::BITWISE_NOT: fmt_token(TokenType::BITWISE_NOT); break;
	case UnaryOp::ADDRESS_OF:  fmt_token(TokenType::TIMES); break;
	case UnaryOp::DEREFERENCE: fmt_token(TokenType::BITSHIFT_LEFT); break;
	default: err_internal_enum(op); break;
	}
}

void fmt_binary_op(BinaryOp op)
{
	switch (op)
	{
	case BinaryOp::LOGIC_AND:      fmt_token(TokenType::LOGIC_AND); break;
	case BinaryOp::LOGIC_OR:       fmt_token(TokenType::LOGIC_OR); break;
	case BinaryOp::LESS:           fmt_token(TokenType::LESS); break;
	case BinaryOp::GREATER:        fmt_token(TokenType::GREATER); break;
	case BinaryOp::LESS_EQUALS:    fmt_token(TokenType::LESS_EQUALS); break;
	case BinaryOp::GREATER_EQUALS: fmt_token(TokenType::GREATER_EQUALS); break;
	case BinaryOp::IS_EQUALS:      fmt_token(TokenType::IS_EQUALS); break;
	case BinaryOp::NOT_EQUALS:     fmt_token(TokenType::NOT_EQUALS); break;
	case BinaryOp::PLUS:           fmt_token(TokenType::PLUS); break;
	case BinaryOp::MINUS:          fmt_token(TokenType::MINUS); break;
	case BinaryOp::TIMES:          fmt_token(TokenType::TIMES); break;
	case BinaryOp::DIV:            fmt_token(TokenType::DIV); break;
	case BinaryOp::MOD:            fmt_token(TokenType::MOD); break;
	case BinaryOp::BITWISE_AND:    fmt_token(TokenType::BITWISE_AND); break;
	case BinaryOp::BITWISE_OR:     fmt_token(TokenType::BITWISE_OR); break;
	case BinaryOp::BITWISE_XOR:    fmt_token(TokenType::BITWISE_XOR); break;
	case BinaryOp::BITSHIFT_LEFT:  fmt_token(TokenType::BITSHIFT_LEFT); break;
	case BinaryOp::BITSHIFT_RIGHT: fmt_token(TokenType::BITSHIFT_RIGHT); break;
	default: err_internal_enum(op); break;
	}
}

void fmt_assign_op(AssignOp op)
{
	switch (op)
	{
	case AssignOp::NONE:           fmt_token(TokenType::ASSIGN); break;
	case AssignOp::PLUS:           fmt_token(TokenType::PLUS_EQUALS); break;
	case AssignOp::MINUS:          fmt_token(TokenType::MINUS_EQUALS); break;
	case AssignOp::TIMES:          fmt_token(TokenType::TIMES_EQUALS); break;
	case AssignOp::DIV:            fmt_token(TokenType::DIV_EQUALS); break;
	case AssignOp::MOD:            fmt_token(TokenType::MOD_EQUALS); break;
	case AssignOp::BITWISE_AND:    fmt_token(TokenType::BITWISE_AND_EQUALS); break;
	case AssignOp::BITWISE_OR:     fmt_token(TokenType::BITWISE_OR_EQUALS); break;
	case AssignOp::BITWISE_XOR:    fmt_token(TokenType::BITWISE_XOR_EQUALS); break;
	case AssignOp::BITSHIFT_LEFT:  fmt_token(TokenType::BITSHIFT_LEFT_EQUALS); break;
	case AssignOp::BITSHIFT_RIGHT: fmt_token(TokenType::BITSHIFT_RIGHT_EQUALS); break;
	default: err_internal_enum(op); break;
	}
}

void fmt_basic_type(BasicType type)
{
	switch (type)
	{
	case BasicType::I8:     fmt_token(TokenType::TYPE_I8); break;
	case BasicType::U8:     fmt_token(TokenType::TYPE_U8); break;
	case BasicType::I16:    fmt_token(TokenType::TYPE_I16); break;
	case BasicType::U16:    fmt_token(TokenType::TYPE_U16); break;
	case BasicType::I32:    fmt_token(TokenType::TYPE_I32); break;
	case BasicType::U32:    fmt_token(TokenType::TYPE_U32); break;
	case BasicType::I64:    fmt_token(TokenType::TYPE_I64); break;
	case BasicType::U64:    fmt_token(TokenType::TYPE_U64); break;
	case BasicType::F32:    fmt_token(TokenType::TYPE_F32); break;
	case BasicType::F64:    fmt_token(TokenType::TYPE_F64); break;
	case BasicType::BOOL:   fmt_token(TokenType::TYPE_BOOL); break;
	case BasicType::STRING: fmt_token(TokenType::TYPE_STRING); break;
	default: err_internal_enum(type); break;
	}
}

void fmt_ident(Ast_Ident ident)
{
	for (u32 i = 0; i < ident.str.count; i += 1)
	{
		printf("%c", ident.str.data[i]);
	}
}

void fmt_module_access(option<Ast_Module_Access*> module_access)
{
	if (!module_access) return;
	for (const Ast_Ident& module : module_access.value()->modules)
	{
		fmt_ident(module);
		fmt_token(TokenType::DOUBLE_COLON);
	}
}

void fmt_expr(Ast_Expr* expr)
{
	printf("!EXPR!");
}

void fmt_block(Ast_Stmt_Block* block)
{
	if (block->statements.empty())
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_token(TokenType::BLOCK_END);
	}
	else
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_endl();
		fmt_endl();
		fmt_token(TokenType::BLOCK_END);
	}
}

void fmt_type(Ast_Type type)
{
	for (u32 i = 0; i < type.pointer_level; i += 1)
	{
		fmt_token(TokenType::TIMES);
	}

	switch (type.tag())
	{
	case Ast_Type::Tag::Basic:
	{
		fmt_basic_type(type.as_basic);
	} break;
	case Ast_Type::Tag::Array:
	{
		fmt_token(TokenType::BRACKET_START);
		fmt_expr(type.as_array->size_expr);
		fmt_token(TokenType::BRACKET_END);
		fmt_type(type.as_array->element_type);
	} break;
	case Ast_Type::Tag::Procedure:
	{
		printf("!PROC_TYPE!");
	} break;
	case Ast_Type::Tag::Unresolved:
	{
		fmt_module_access(type.as_unresolved->module_access);
		fmt_ident(type.as_unresolved->ident);
	} break;
	default: err_internal_enum(type.tag()); break;
	}
}

void fmt_decl_proc(Ast_Decl_Proc* decl)
{
	fmt_ident(decl->ident);
	fmt_space();
	fmt_token(TokenType::DOUBLE_COLON);
	fmt_space();

	fmt_token(TokenType::PAREN_START);
	for (u32 i = 0; i < decl->input_params.size(); i += 1)
	{
		Ast_Proc_Param param = decl->input_params[i];
		fmt_ident(param.ident);
		fmt_token(TokenType::COLON);
		fmt_space();
		fmt_type(param.type);
		if (i + 1 != decl->input_params.size())
		{
			fmt_token(TokenType::COMMA);
			fmt_space();
		}
	}
	fmt_token(TokenType::PAREN_END);
	fmt_space();
	
	if (decl->return_type)
	{
		fmt_token(TokenType::ARROW);
		fmt_space();
		fmt_type(decl->return_type.value());
		fmt_space();
	}
	if (decl->is_external)
		fmt_token(TokenType::AT);
	else fmt_block(decl->block);
	
	fmt_endl();
	fmt_endl();
}

void fmt_decl_impl(Ast_Decl_Impl* decl)
{
	fmt_token(TokenType::KEYWORD_IMPL);
	fmt_space();
	fmt_type(decl->type);
	fmt_space();

	if (decl->member_procedures.empty())
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_token(TokenType::BLOCK_END);
	}
	else
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_endl();
		for (Ast_Decl_Proc* member : decl->member_procedures) //@tab indentation offset for all
		{
			fmt_decl_proc(member); //@2 endl after proc decl leaves empty line
		}
		fmt_token(TokenType::BLOCK_END);
	}

	fmt_endl();
	fmt_endl();
}

void fmt_decl_enum(Ast_Decl_Enum* decl)
{
	fmt_ident(decl->ident);
	fmt_space();
	fmt_token(TokenType::DOUBLE_COLON);
	fmt_space();
	fmt_token(TokenType::KEYWORD_ENUM);
	fmt_space();
	
	fmt_token(TokenType::DOUBLE_COLON); //@basic type isnt known to be implicit / explicit
	fmt_space();
	fmt_basic_type(decl->basic_type);
	fmt_space();

	if (decl->variants.empty())
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_token(TokenType::BLOCK_END);
	}
	else
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_endl();
		for (Ast_Enum_Variant& variant : decl->variants)
		{
			fmt_tab();
			fmt_ident(variant.ident);
			fmt_space();
			fmt_token(TokenType::ASSIGN);
			fmt_space();
			fmt_expr(variant.consteval_expr->expr);
			fmt_token(TokenType::SEMICOLON);
			fmt_endl();
		}
		fmt_token(TokenType::BLOCK_END);
	}
	
	fmt_endl();
	fmt_endl();
}

void fmt_decl_struct(Ast_Decl_Struct* decl)
{
	fmt_ident(decl->ident);
	fmt_space();
	fmt_token(TokenType::DOUBLE_COLON);
	fmt_space();
	fmt_token(TokenType::KEYWORD_STRUCT);
	fmt_space();

	if (decl->fields.empty())
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_token(TokenType::BLOCK_END);
	}
	else
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_endl();
		for (Ast_Struct_Field& field : decl->fields)
		{
			fmt_tab();
			fmt_ident(field.ident);
			fmt_token(TokenType::COLON);
			fmt_space();
			fmt_type(field.type);
			if (field.default_expr)
			{
				fmt_space();
				fmt_token(TokenType::ASSIGN);
				fmt_space();
				fmt_expr(field.default_expr.value());
			}
			fmt_token(TokenType::SEMICOLON);
			fmt_endl();
		}
		fmt_token(TokenType::BLOCK_END);
	}

	fmt_endl();
	fmt_endl();
}

void fmt_decl_global(Ast_Decl_Global* decl) //@support dense 1 liners?
{
	fmt_ident(decl->ident);
	fmt_space();
	fmt_token(TokenType::DOUBLE_COLON);
	fmt_space();
	fmt_expr(decl->consteval_expr->expr);
	fmt_token(TokenType::SEMICOLON);
	
	fmt_endl();
	fmt_endl();
}

void fmt_decl_import(Ast_Decl_Import* decl) //@support dense 1 liners?
{
	fmt_token(TokenType::KEYWORD_IMPORT);
	fmt_space();

	if (decl->modules.size() == 1) //@might change parsing of this case
	{
		fmt_ident(decl->modules[0]);
	}
	else
	{
		for (Ast_Ident& module : decl->modules)
		{
			fmt_ident(module);
			fmt_token(TokenType::DOUBLE_COLON);
		}
	}

	if (decl->target)
	{
		Ast_Import_Target* target = decl->target.value();
		switch (target->tag())
		{
		case Ast_Import_Target::Tag::Wildcard: fmt_token(TokenType::TIMES); break;
		case Ast_Import_Target::Tag::Symbol_List: //@multiline on large sets, come up with a rule
		{
			fmt_token(TokenType::BLOCK_START);
			for (u32 i = 0; i < target->as_symbol_list.symbols.size(); i += 1)
			{
				fmt_ident(target->as_symbol_list.symbols[i]);
				if (i + 1 != target->as_symbol_list.symbols.size())
				{
					fmt_token(TokenType::COMMA);
					fmt_space();
				}
			}
			fmt_token(TokenType::BLOCK_END);
		} break;
		case Ast_Import_Target::Tag::Symbol_Or_Module: fmt_ident(target->as_symbol_or_module.ident); break;
		default: err_internal_enum(target->tag()); break;
		}
	}
	fmt_token(TokenType::SEMICOLON);
	
	fmt_endl();
	fmt_endl();
}

void fmt_ast(Ast* ast)
{
	printf("------------------------\n");
	fmt_endl();
	for (Ast_Decl* decl : ast->decls)
	{
		switch (decl->tag())
		{
		case Ast_Decl::Tag::Proc: fmt_decl_proc(decl->as_proc); break;
		case Ast_Decl::Tag::Impl: fmt_decl_impl(decl->as_impl); break;
		case Ast_Decl::Tag::Enum: fmt_decl_enum(decl->as_enum); break;
		case Ast_Decl::Tag::Struct: fmt_decl_struct(decl->as_struct); break;
		case Ast_Decl::Tag::Global: fmt_decl_global(decl->as_global); break;
		case Ast_Decl::Tag::Import: fmt_decl_import(decl->as_import); break;
		default: err_internal_enum(decl->tag()); break;
		}
	}
}

#endif
