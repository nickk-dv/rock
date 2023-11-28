export module fmt;

import general;
import ast;
import token;
import err_handler;
import <typeinfo>;

/*
@Tasks
use methods
decl spacing, impl proc spacing
short block info not kept
expr parentesis not kept -idea maybe use span if it captures any parens, or change parser to capture parens in expr span
decl_proc variadic not kept
rework switch syntax
handle & keep comments
keep & reduce logical empty lines to 1
modify file instead of printing
*/

//@macro export isnt supported, cannot be used globally
#define err_internal_enum(enum_var) \
{ \
err_internal("invalid or not implemented enum variant:"); \
printf("function: %s\n", __FUNCSIG__); \
printf("line:     %d\n", __LINE__); \
printf("type:     %s\n", typeid(enum_var).name()); \
}

Ast* fmt_curr_ast;

void fmt_tab()   { printf("\t"); }
void fmt_endl()  { printf("\n"); }
void fmt_space() { printf(" "); }

void fmt_literal(Ast_Literal* literal)
{
	for (u32 i = literal->span.start; i <= literal->span.end; i += 1)
	{
		printf("%c", fmt_curr_ast->source->str.data[i]);
	}
}

void fmt_token(TokenType type)
{
	switch (type)
	{
	case TokenType::KEYWORD_PUB:           printf("pub"); break;
	case TokenType::KEYWORD_MOD:           printf("mod"); break;
	case TokenType::KEYWORD_MUT:           printf("mut"); break;
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
	case AssignOp::ASSIGN:         fmt_token(TokenType::ASSIGN); break;
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
	case BasicType::I8:     fmt_token(TokenType::KEYWORD_I8); break;
	case BasicType::I16:    fmt_token(TokenType::KEYWORD_I16); break;
	case BasicType::I32:    fmt_token(TokenType::KEYWORD_I32); break;
	case BasicType::I64:    fmt_token(TokenType::KEYWORD_I64); break;
	case BasicType::U8:     fmt_token(TokenType::KEYWORD_U8); break;
	case BasicType::U16:    fmt_token(TokenType::KEYWORD_U16); break;
	case BasicType::U32:    fmt_token(TokenType::KEYWORD_U32); break;
	case BasicType::U64:    fmt_token(TokenType::KEYWORD_U64); break;
	case BasicType::F32:    fmt_token(TokenType::KEYWORD_F32); break;
	case BasicType::F64:    fmt_token(TokenType::KEYWORD_F64); break;
	case BasicType::BOOL:   fmt_token(TokenType::KEYWORD_BOOL); break;
	case BasicType::STRING: fmt_token(TokenType::KEYWORD_STRING); break;
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

void fmt_expr(Ast_Expr* expr); //@remove this forward declaration
void fmt_type(Ast_Type type); //@remove this forward declaration
void fmt_stmt(Ast_Stmt* stmt, u32 indent); //@remove this forward declaration

void fmt_indent(u32 indent)
{
	for (u32 i = 0; i < indent; i += 1) fmt_tab();
}

void fmt_expr_list(Ast_Expr_List* expr_list)
{
	for (u32 i = 0; i < expr_list->exprs.size(); i += 1)
	{
		fmt_expr(expr_list->exprs[i]);
		if (i + 1 != expr_list->exprs.size())
		{
			fmt_token(TokenType::COMMA);
			fmt_space();
		}
	}
}

void fmt_access(Ast_Access* access, bool first)
{
	switch (access->tag())
	{
	case Ast_Access::Tag::Ident:
	{
		if (!first) fmt_token(TokenType::DOT);
		fmt_ident(access->as_ident);
	} break;
	case Ast_Access::Tag::Array:
	{
		fmt_token(TokenType::BRACKET_START);
		fmt_expr(access->as_array.index_expr);
		fmt_token(TokenType::BRACKET_END);
	} break;
	case Ast_Access::Tag::Call:
	{
		if (!first) fmt_token(TokenType::DOT);
		fmt_ident(access->as_call.ident);
		fmt_token(TokenType::PAREN_START);
		fmt_expr_list(access->as_call.input);
		fmt_token(TokenType::PAREN_END);
	} break;
	default: err_internal_enum(access->tag()); break;
	}
}

void fmt_something(Ast_Something* something)
{
	fmt_module_access(something->module_access);
	fmt_access(something->access, true);
	option<Ast_Access*> next = something->access->next;
	while (next)
	{
		fmt_access(next.value(), false);
		next = next.value()->next;
	}
}

void fmt_term(Ast_Term* term) //@parens lost
{
	switch (term->tag())
	{
	case Ast_Term::Tag::Enum:
	{
		Ast_Enum* _enum = term->as_enum;
		fmt_token(TokenType::DOT);
		fmt_ident(_enum->unresolved.variant_ident);
	} break;
	case Ast_Term::Tag::Cast:
	{
		Ast_Cast* cast = term->as_cast;
		fmt_token(TokenType::KEYWORD_CAST);
		fmt_token(TokenType::PAREN_START);
		fmt_basic_type(cast->basic_type);
		fmt_token(TokenType::COMMA);
		fmt_space();
		fmt_expr(cast->expr);
		fmt_token(TokenType::PAREN_END);
	} break;
	case Ast_Term::Tag::Sizeof:
	{
		Ast_Sizeof* size_of = term->as_sizeof;
		fmt_token(TokenType::KEYWORD_SIZEOF);
		fmt_token(TokenType::PAREN_START);
		fmt_type(size_of->type);
		fmt_token(TokenType::PAREN_END);
	} break;
	case Ast_Term::Tag::Literal: fmt_literal(term->as_literal); break;
	case Ast_Term::Tag::Something: fmt_something(term->as_something); break;
	case Ast_Term::Tag::Array_Init:
	{
		Ast_Array_Init* array_init = term->as_array_init;
		if (array_init->type) fmt_type(array_init->type.value());
		fmt_token(TokenType::BLOCK_START);
		fmt_expr_list(array_init->input);
		fmt_token(TokenType::BLOCK_END);
	} break;
	case Ast_Term::Tag::Struct_Init:
	{
		Ast_Struct_Init* struct_init = term->as_struct_init;
		fmt_module_access(struct_init->unresolved.module_access);
		fmt_token(TokenType::DOT);
		fmt_token(TokenType::BLOCK_START);
		fmt_expr_list(struct_init->input);
		fmt_token(TokenType::BLOCK_END);
	} break;
	default: err_internal_enum(term->tag()); break;
	}
}

void fmt_unary(Ast_Unary_Expr* unary_expr) //@parens lost
{
	fmt_unary_op(unary_expr->op);
	fmt_expr(unary_expr->right);
}

void fmt_binary(Ast_Binary_Expr* binary_expr) //@parens lost
{
	fmt_expr(binary_expr->left);
	fmt_space();
	fmt_binary_op(binary_expr->op);
	fmt_space();
	fmt_expr(binary_expr->right);
}

void fmt_expr(Ast_Expr* expr)
{
	switch (expr->tag())
	{
	case Ast_Expr::Tag::Term:   fmt_term(expr->as_term); break;
	case Ast_Expr::Tag::Unary:  fmt_unary(expr->as_unary_expr); break;
	case Ast_Expr::Tag::Binary: fmt_binary(expr->as_binary_expr); break;
	default: err_internal_enum(expr->tag()); break;
	}
}

void fmt_block(Ast_Stmt_Block* block, u32 indent, bool attached)
{
	if (attached) fmt_space();
	else fmt_indent(indent);
	
	if (block->statements.empty())
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_token(TokenType::BLOCK_END);
	}
	else
	{
		fmt_token(TokenType::BLOCK_START);
		fmt_endl();
		for (Ast_Stmt* stmt : block->statements) fmt_stmt(stmt, indent + 1);
		fmt_indent(indent);
		fmt_token(TokenType::BLOCK_END);
	}
}

void fmt_stmt_var_decl(Ast_Stmt_Var_Decl* var_decl)
{
	if (var_decl->is_mutable)
	{
		fmt_token(TokenType::KEYWORD_MUT);
		fmt_space();
	}
	fmt_ident(var_decl->ident);
	fmt_space();
	fmt_token(TokenType::COLON);
	if (var_decl->type)
	{
		fmt_space();
		fmt_type(var_decl->type.value());
	}
	if (var_decl->expr)
	{
		if (var_decl->type) fmt_space();
		fmt_token(TokenType::ASSIGN);
		fmt_space();
		fmt_expr(var_decl->expr.value());
	}
}

void fmt_stmt_var_assign(Ast_Stmt_Var_Assign* var_assign)
{
	fmt_something(var_assign->something);
	fmt_space();
	fmt_assign_op(var_assign->op);
	fmt_space();
	fmt_expr(var_assign->expr);
}

void fmt_stmt(Ast_Stmt* stmt, u32 indent)
{
	fmt_indent(indent);

	switch (stmt->tag())
	{
	case Ast_Stmt::Tag::If:
	{
		Ast_Stmt_If* _if = stmt->as_if;
		fmt_token(TokenType::KEYWORD_IF);
		fmt_space();
		fmt_expr(_if->condition_expr);
		fmt_block(_if->block, indent, true);
	} break;
	case Ast_Stmt::Tag::For:
	{
		Ast_Stmt_For* _for = stmt->as_for;
		fmt_token(TokenType::KEYWORD_FOR);
		fmt_space();
		if (_for->var_decl)
		{
			fmt_stmt_var_decl(_for->var_decl.value());
			fmt_token(TokenType::SEMICOLON);
			fmt_space();
		}
		if (_for->condition_expr)
		{
			fmt_expr(_for->condition_expr.value());
		}
		if (_for->var_assign)
		{
			fmt_token(TokenType::SEMICOLON);
			fmt_space();
			fmt_stmt_var_assign(_for->var_assign.value());
			fmt_token(TokenType::SEMICOLON);
		}
		fmt_block(_for->block, indent, true);
	} break;
	case Ast_Stmt::Tag::Block:
	{
		fmt_block(stmt->as_block, indent, false);
	} break;
	case Ast_Stmt::Tag::Defer:
	{
		Ast_Stmt_Defer* defer = stmt->as_defer;
		fmt_token(TokenType::KEYWORD_DEFER);
		fmt_block(defer->block, indent, true);
	} break;
	case Ast_Stmt::Tag::Break:
	{
		fmt_token(TokenType::KEYWORD_BREAK);
		fmt_token(TokenType::SEMICOLON);
	} break;
	case Ast_Stmt::Tag::Return:
	{
		Ast_Stmt_Return* _return = stmt->as_return;
		fmt_token(TokenType::KEYWORD_RETURN);
		if (_return->expr)
		{
			fmt_space();
			fmt_expr(_return->expr.value());
		}
		fmt_token(TokenType::SEMICOLON);
	} break;
	case Ast_Stmt::Tag::Switch:
	{
		Ast_Stmt_Switch* _switch = stmt->as_switch;
		fmt_token(TokenType::KEYWORD_SWITCH);
		fmt_space();
		fmt_expr(_switch->expr);
		
		fmt_space();
		fmt_token(TokenType::BLOCK_START);
		fmt_endl();

		for (Ast_Switch_Case _case : _switch->cases)
		{
			fmt_indent(indent + 1);
			fmt_expr(_case.case_expr);
			if (_case.block)
			{
				fmt_block(_case.block.value(), indent + 1, true);
			}
			else
			{
				fmt_token(TokenType::COLON);
			}
			fmt_endl();
		}
		fmt_indent(indent);
		fmt_token(TokenType::BLOCK_END);
	} break;
	case Ast_Stmt::Tag::Continue:
	{
		fmt_token(TokenType::KEYWORD_CONTINUE);
		fmt_token(TokenType::SEMICOLON);
	} break;
	case Ast_Stmt::Tag::Var_Decl:
	{
		Ast_Stmt_Var_Decl* var_decl = stmt->as_var_decl;
		fmt_stmt_var_decl(var_decl);
		fmt_token(TokenType::SEMICOLON);
	} break;
	case Ast_Stmt::Tag::Var_Assign:
	{
		Ast_Stmt_Var_Assign* var_assign = stmt->as_var_assign;
		fmt_stmt_var_assign(var_assign);
		fmt_token(TokenType::SEMICOLON);
	} break;
	case Ast_Stmt::Tag::Proc_Call: 
	{
		Ast_Stmt_Proc_Call* proc_call = stmt->as_proc_call;
		fmt_something(proc_call->something);
		fmt_token(TokenType::SEMICOLON);
	} break;
	default: err_internal_enum(stmt->tag()); break;
	}

	fmt_endl();
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
	case Ast_Type::Tag::Procedure: //@not keeping optional arg names now + share parsing + output with normal proc decl?
	{
		Ast_Type_Procedure* procedure = type.as_procedure;

		fmt_token(TokenType::PAREN_START);
		for (u32 i = 0; i < procedure->input_types.size(); i += 1)
		{
			fmt_type(procedure->input_types[i]);
			if (i + 1 != procedure->input_types.size())
			{
				fmt_token(TokenType::COMMA);
				fmt_space();
			}
		}
		fmt_token(TokenType::PAREN_END);

		if (procedure->return_type)
		{
			fmt_space();
			fmt_token(TokenType::ARROW_THIN);
			fmt_space();
			fmt_type(procedure->return_type.value());
		}
	} break;
	case Ast_Type::Tag::Unresolved:
	{
		fmt_module_access(type.as_unresolved->module_access);
		fmt_ident(type.as_unresolved->ident);
	} break;
	default: err_internal_enum(type.tag()); break;
	}
}

void fmt_decl_mod(Ast_Decl_Mod* decl)
{
	if (decl->is_public)
	{
		fmt_token(TokenType::KEYWORD_PUB);
		fmt_space();
	}
	fmt_token(TokenType::KEYWORD_MOD);
	fmt_space();
	fmt_ident(decl->ident);
	fmt_token(TokenType::SEMICOLON);
	fmt_endl();
}

void fmt_decl_proc(Ast_Decl_Proc* decl, u32 indent)
{
	fmt_indent(indent);
	if (decl->is_public)
	{
		fmt_token(TokenType::KEYWORD_PUB);
		fmt_space();
	}
	fmt_ident(decl->ident);
	fmt_space();
	fmt_token(TokenType::DOUBLE_COLON);
	fmt_space();

	fmt_token(TokenType::PAREN_START);
	for (u32 i = 0; i < decl->input_params.size(); i += 1)
	{
		Ast_Proc_Param param = decl->input_params[i];
		if (param.is_mutable)
		{
			fmt_token(TokenType::KEYWORD_MUT);
			fmt_space();
		}
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
	
	if (decl->return_type)
	{
		fmt_space();
		fmt_token(TokenType::ARROW_THIN);
		fmt_space();
		fmt_type(decl->return_type.value());
	}

	if (decl->is_external)
	{
		fmt_space();
		fmt_token(TokenType::AT);
	}
	else fmt_block(decl->block, indent, true);
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
			fmt_decl_proc(member, 1); //@2 endl after proc decl leaves empty line
		}
		fmt_token(TokenType::BLOCK_END);
	}
	fmt_endl();
}

void fmt_decl_enum(Ast_Decl_Enum* decl)
{
	if (decl->is_public)
	{
		fmt_token(TokenType::KEYWORD_PUB);
		fmt_space();
	}
	fmt_ident(decl->ident);
	fmt_space();
	fmt_token(TokenType::DOUBLE_COLON);
	fmt_space();
	fmt_token(TokenType::KEYWORD_ENUM);
	fmt_space();
	
	if (decl->basic_type)
	{
		fmt_basic_type(decl->basic_type.value());
		fmt_space();
	}

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
}

void fmt_decl_struct(Ast_Decl_Struct* decl)
{
	if (decl->is_public)
	{
		fmt_token(TokenType::KEYWORD_PUB);
		fmt_space();
	}
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
			if (field.is_public)
			{
				fmt_token(TokenType::KEYWORD_PUB);
				fmt_space();
			}
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
}

void fmt_decl_global(Ast_Decl_Global* decl) //@support dense 1 liners?
{
	if (decl->is_public)
	{
		fmt_token(TokenType::KEYWORD_PUB);
		fmt_space();
	}
	fmt_ident(decl->ident);
	fmt_space();
	fmt_token(TokenType::DOUBLE_COLON);
	fmt_space();
	fmt_expr(decl->consteval_expr->expr);
	fmt_token(TokenType::SEMICOLON);
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
}

export void fmt_ast(Ast* ast)
{
	fmt_curr_ast = ast;
	printf("------------------------\n");
	for (Ast_Decl* decl : ast->decls)
	{
		switch (decl->tag())
		{
		case Ast_Decl::Tag::Mod: fmt_decl_mod(decl->as_mod); break;
		case Ast_Decl::Tag::Proc: fmt_decl_proc(decl->as_proc, 0); break;
		case Ast_Decl::Tag::Impl: fmt_decl_impl(decl->as_impl); break;
		case Ast_Decl::Tag::Enum: fmt_decl_enum(decl->as_enum); break;
		case Ast_Decl::Tag::Struct: fmt_decl_struct(decl->as_struct); break;
		case Ast_Decl::Tag::Global: fmt_decl_global(decl->as_global); break;
		case Ast_Decl::Tag::Import: fmt_decl_import(decl->as_import); break;
		default: err_internal_enum(decl->tag()); break;
		}
	}
}
