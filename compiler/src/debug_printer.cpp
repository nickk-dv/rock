#include "debug_printer.h"

void debug_print_ast(Ast* ast)
{
	printf("\n[AST]\n");

	for (Ast_Import_Decl* import_decl : ast->imports) debug_print_import_decl(import_decl);
	for (Ast_Use_Decl* use_decl : ast->uses) debug_print_use_decl(use_decl);
	for (Ast_Struct_Decl* struct_decl : ast->structs) debug_print_struct_decl(struct_decl);
	for (Ast_Enum_Decl* enum_decl : ast->enums) debug_print_enum_decl(enum_decl);
	for (Ast_Proc_Decl* proc_decl : ast->procs) debug_print_proc_decl(proc_decl);
}

void debug_print_token(Token token, bool endl, bool location)
{
	if (location) printf("%lu:%lu ", token.l0, token.c0);

	if (token.type == TOKEN_IDENT)
	{
		for (u64 i = 0; i < token.string_value.count; i++)
			printf("%c", token.string_value.data[i]);
	}
	else if (token.type == TOKEN_BOOL_LITERAL)
	{
		if (token.bool_value)
			printf("true");
		else printf("false");
	}
	else if (token.type == TOKEN_FLOAT_LITERAL)
	{
		printf("%f", token.float64_value);
	}
	else if (token.type == TOKEN_INTEGER_LITERAL)
	{
		printf("%llu", token.integer_value);
	}
	else if (token.type == TOKEN_STRING_LITERAL)
	{
		printf("%s", token.string_literal_value);
	}
	else
	{
		switch (token.type) 
		{
			case TOKEN_KEYWORD_STRUCT: printf("struct"); break;
			case TOKEN_KEYWORD_ENUM: printf("enum"); break;
			case TOKEN_KEYWORD_IF: printf("if"); break;
			case TOKEN_KEYWORD_ELSE: printf("else"); break;
			case TOKEN_KEYWORD_TRUE: printf("true"); break;
			case TOKEN_KEYWORD_FALSE: printf("false"); break;
			case TOKEN_KEYWORD_FOR: printf("for"); break;
			case TOKEN_KEYWORD_DEFER: printf("defer"); break;
			case TOKEN_KEYWORD_BREAK: printf("break"); break;
			case TOKEN_KEYWORD_RETURN: printf("return"); break;
			case TOKEN_KEYWORD_CONTINUE: printf("continue"); break;
			case TOKEN_KEYWORD_IMPORT: printf("import"); break;
			case TOKEN_KEYWORD_USE: printf("use"); break;

			case TOKEN_TYPE_I8: printf("i8"); break;
			case TOKEN_TYPE_U8: printf("u8"); break;
			case TOKEN_TYPE_I16: printf("i16"); break;
			case TOKEN_TYPE_U16: printf("u16"); break;
			case TOKEN_TYPE_I32: printf("i32"); break;
			case TOKEN_TYPE_U32: printf("u32"); break;
			case TOKEN_TYPE_I64: printf("i64"); break;
			case TOKEN_TYPE_U64: printf("u64"); break;
			case TOKEN_TYPE_F32: printf("f32"); break;
			case TOKEN_TYPE_F64: printf("f64"); break;
			case TOKEN_TYPE_BOOL: printf("bool"); break;
			case TOKEN_TYPE_STRING: printf("string"); break;

			case TOKEN_DOT: printf("."); break;
			case TOKEN_COLON: printf(":"); break;
			case TOKEN_QUOTE: printf("'"); break;
			case TOKEN_COMMA: printf(","); break;
			case TOKEN_SEMICOLON: printf(";"); break;
			case TOKEN_DOUBLE_DOT: printf(".."); break;
			case TOKEN_DOUBLE_COLON: printf("::"); break;
			case TOKEN_BLOCK_START: printf("{"); break;
			case TOKEN_BLOCK_END: printf("}"); break;
			case TOKEN_BRACKET_START: printf("["); break;
			case TOKEN_BRACKET_END: printf("]"); break;
			case TOKEN_PAREN_START: printf("("); break;
			case TOKEN_PAREN_END: printf(")"); break;
			case TOKEN_AT: printf("@"); break;
			case TOKEN_HASH: printf("#"); break;
			case TOKEN_QUESTION: printf("?"); break;

			case TOKEN_ASSIGN: printf("="); break;
			case TOKEN_PLUS: printf("+"); break;
			case TOKEN_MINUS: printf("-"); break;
			case TOKEN_TIMES: printf("*"); break;
			case TOKEN_DIV: printf("/"); break;
			case TOKEN_MOD: printf("%%"); break;
			case TOKEN_BITWISE_AND: printf("&"); break;
			case TOKEN_BITWISE_OR: printf("|"); break;
			case TOKEN_BITWISE_XOR: printf("^"); break;
			case TOKEN_LESS: printf("<"); break;
			case TOKEN_GREATER: printf(">"); break;
			case TOKEN_LOGIC_NOT: printf("!"); break;
			case TOKEN_IS_EQUALS: printf("=="); break;
			case TOKEN_PLUS_EQUALS: printf("+="); break;
			case TOKEN_MINUS_EQUALS: printf("-="); break;
			case TOKEN_TIMES_EQUALS: printf("*="); break;
			case TOKEN_DIV_EQUALS: printf("/="); break;
			case TOKEN_MOD_EQUALS: printf("%%="); break;
			case TOKEN_BITWISE_AND_EQUALS: printf("&="); break;
			case TOKEN_BITWISE_OR_EQUALS: printf("|="); break;
			case TOKEN_BITWISE_XOR_EQUALS: printf("^="); break;
			case TOKEN_LESS_EQUALS: printf("<="); break;
			case TOKEN_GREATER_EQUALS: printf(">="); break;
			case TOKEN_NOT_EQUALS: printf("!="); break;
			case TOKEN_LOGIC_AND: printf("&&"); break;
			case TOKEN_LOGIC_OR: printf("||"); break;
			case TOKEN_BITWISE_NOT: printf("~"); break;
			case TOKEN_BITSHIFT_LEFT: printf("<<"); break;
			case TOKEN_BITSHIFT_RIGHT: printf(">>"); break;

			case TOKEN_ERROR: printf("Token Error"); break;
			case TOKEN_EOF: printf("End of file"); break;

			default: printf("[UNKNOWN TOKEN]"); break;
		}
	}

	if (endl) printf("\n");
}

void debug_print_ident(Ast_Ident ident, bool endl, bool location)
{
	if (location) printf("%lu:%lu ", ident.l0, ident.c0);
	
	for (u64 i = 0; i < ident.str.count; i++)
		printf("%c", ident.str.data[i]);
	
	if (endl) printf("\n");
}

void debug_print_unary_op(UnaryOp op)
{
	printf("UnaryOp: ");
	switch (op)
	{
		case UNARY_OP_MINUS: printf("-"); break;
		case UNARY_OP_LOGIC_NOT: printf("!"); break;
		case UNARY_OP_ADRESS_OF: printf("&"); break;
		case UNARY_OP_BITWISE_NOT: printf("~"); break;
		default: printf("[UNKNOWN UNARY OP]"); break;
	}
	printf("\n");
}

void debug_print_binary_op(BinaryOp op)
{
	printf ("BinaryOp: ");
	switch (op)
	{
		case BINARY_OP_PLUS: printf("+"); break;
		case BINARY_OP_MINUS: printf("-"); break;
		case BINARY_OP_TIMES: printf("*"); break;
		case BINARY_OP_DIV: printf("/"); break;
		case BINARY_OP_MOD: printf("%%"); break;
		case BINARY_OP_BITSHIFT_LEFT: printf("<<"); break;
		case BINARY_OP_BITSHIFT_RIGHT: printf(">>"); break;
		case BINARY_OP_LESS: printf("<"); break;
		case BINARY_OP_GREATER: printf(">"); break;
		case BINARY_OP_LESS_EQUALS: printf("<="); break;
		case BINARY_OP_GREATER_EQUALS: printf(">="); break;
		case BINARY_OP_IS_EQUALS: printf("=="); break;
		case BINARY_OP_NOT_EQUALS: printf("!="); break;
		case BINARY_OP_BITWISE_AND: printf("&"); break;
		case BINARY_OP_BITWISE_XOR: printf("^"); break;
		case BINARY_OP_BITWISE_OR: printf("|"); break;
		case BINARY_OP_LOGIC_AND: printf("&&"); break;
		case BINARY_OP_LOGIC_OR: printf("||"); break;
		default: printf("[UNKNOWN BINARY OP]"); break;
	}
	printf("\n");
}

void debug_print_assign_op(AssignOp op)
{
	printf("AssignOp: ");
	switch (op)
	{
		case ASSIGN_OP_NONE: printf("="); break;
		case ASSIGN_OP_PLUS: printf("+="); break;
		case ASSIGN_OP_MINUS: printf("-="); break;
		case ASSIGN_OP_TIMES: printf("*="); break;
		case ASSIGN_OP_DIV: printf("/="); break;
		case ASSIGN_OP_MOD: printf("%%="); break;
		case ASSIGN_OP_BITWISE_AND: printf("&="); break;
		case ASSIGN_OP_BITWISE_OR: printf("|="); break;
		case ASSIGN_OP_BITWISE_XOR: printf("^="); break;
		case ASSIGN_OP_BITSHIFT_LEFT: printf("<<="); break;
		case ASSIGN_OP_BITSHIFT_RIGHT: printf(">>="); break;
		default: printf("[UNKNOWN ASSIGN OP]"); break;
	}
	printf("\n");
}

void debug_print_basic_type(BasicType type)
{
	switch (type)
	{
		case BASIC_TYPE_I8: printf("i8"); break;
		case BASIC_TYPE_U8: printf("u8"); break;
		case BASIC_TYPE_I16: printf("i16"); break;
		case BASIC_TYPE_U16: printf("u16"); break;
		case BASIC_TYPE_I32: printf("i32"); break;
		case BASIC_TYPE_U32: printf("u32"); break;
		case BASIC_TYPE_I64: printf("i64"); break;
		case BASIC_TYPE_U64: printf("u64"); break;
		case BASIC_TYPE_F32: printf("f32"); break;
		case BASIC_TYPE_F64: printf("f64"); break;
		case BASIC_TYPE_BOOL: printf("bool"); break;
		case BASIC_TYPE_STRING: printf("string"); break;
		default: printf("[UNKNOWN BASIC TYPE]"); break;
	}
}

void debug_print_branch(u32& depth)
{
	if (depth > 0)
	{
		debug_print_spacing(depth);
		printf("|____");
	}
	depth += 1;
}

void debug_print_spacing(u32 depth)
{
	for (u32 i = 0; i < depth; i++) 
		printf("     ");
}

void debug_print_import_decl(Ast_Import_Decl* import_decl)
{
	printf("\nImport_Decl: ");
	debug_print_ident(import_decl->alias, false, false);
	printf(" :: import ");

	printf("\"%s\"", import_decl->file_path.token.string_literal_value);
	printf("\n");
}

void debug_print_use_decl(Ast_Use_Decl* use_decl)
{
	printf("\nUse_Decl: ");
	debug_print_ident(use_decl->alias, false, false);
	printf(":: use ");

	debug_print_ident(use_decl->import, false, false);
	printf(".");
	debug_print_ident(use_decl->symbol, true, false);
}

void debug_print_struct_decl(Ast_Struct_Decl* struct_decl)
{
	printf("\nStruct_Decl: "); 
	debug_print_ident(struct_decl->type, true, false);

	for (const Ast_Ident_Type_Pair& field : struct_decl->fields)
	{
		debug_print_ident(field.ident, false, false);
		printf(": "); 
		debug_print_type(field.type);
		printf("\n");
	}
}

void debug_print_enum_decl(Ast_Enum_Decl* enum_decl)
{
	printf("\nEnum_Decl: "); 
	debug_print_ident(enum_decl->type, false, false);
	printf (": ");
	if (enum_decl->basic_type.has_value())
		debug_print_basic_type(enum_decl->basic_type.value());
	else debug_print_basic_type(BASIC_TYPE_I32);
	printf("\n");

	for (const Ast_Ident_Literal_Pair& variant : enum_decl->variants)
	{
		debug_print_ident(variant.ident, false, false);
		printf(": ");
		debug_print_token(variant.literal.token, true);
	}
}

void debug_print_proc_decl(Ast_Proc_Decl* proc_decl)
{
	printf("\nProc_Decl: ");
	debug_print_ident(proc_decl->ident, true, false);

	printf("Params: ");
	if (!proc_decl->input_params.empty())
	{
		printf("\n");
		for (const Ast_Ident_Type_Pair& param : proc_decl->input_params)
		{
			debug_print_ident(param.ident, false, false);
			printf(": ");
			debug_print_type(param.type);
			printf("\n");
		}
	}
	else printf("---\n");

	printf("Return: ");
	if (proc_decl->return_type.has_value())
	{
		debug_print_type(proc_decl->return_type.value());
		printf("\n");
	}
	else printf("---\n");

	if (proc_decl->is_external)
		printf("External\n");
	debug_print_block(proc_decl->block, 0);
}

void debug_print_type(Ast_Type* type)
{
	switch (type->tag)
	{
		case Ast_Type::Tag::Basic: debug_print_basic_type(type->as_basic); break;
		case Ast_Type::Tag::Pointer: { printf("*"); debug_print_type(type->as_pointer); } break;
		case Ast_Type::Tag::Array: debug_print_array_type(type->as_array); break;
		case Ast_Type::Tag::Custom: debug_print_custom_type(type->as_custom); break;
	}
}

void debug_print_array_type(Ast_Array_Type* array_type)
{
	if (array_type->is_dynamic) printf("[..]");
	else printf("[%llu]", array_type->fixed_size);
	debug_print_type(array_type->element_type);
}

void debug_print_custom_type(Ast_Custom_Type* custom_type)
{
	if (custom_type->import)
	{
		debug_print_ident(custom_type->import.value(), false, false);
		printf(".");
	}
	debug_print_ident(custom_type->type, false, false);
}

void debug_print_var(Ast_Var* var)
{
	debug_print_ident(var->ident, false, false);
	if (var->access) debug_print_access(var->access.value());
	printf("\n");
}

void debug_print_access(Ast_Access* access)
{
	if (access->tag == Ast_Access::Tag::Var)
	{
		printf(".");
		debug_print_var_access(access->as_var);
	}
	else debug_print_array_access(access->as_array);
}

void debug_print_var_access(Ast_Var_Access* var_access)
{
	debug_print_ident(var_access->ident, false, false);
	if (var_access->next) debug_print_access(var_access->next.value());
}

void debug_print_array_access(Ast_Array_Access* array_access)
{
	printf("[expr]");
	if (array_access->next) debug_print_access(array_access->next.value());
}

void debug_print_enum(Ast_Enum* _enum)
{
	if (_enum->import)
	{
		debug_print_ident(_enum->import.value(), false, false);
		printf(".");
	}
	debug_print_ident(_enum->type, false, false);
	printf("::");
	debug_print_ident(_enum->variant, true, false);
}

void debug_print_term(Ast_Term* term, u32 depth)
{
	if (term->tag == Ast_Term::Tag::Var)
	{
		debug_print_branch(depth);
		printf("Term_Var: ");
	}
	else if (term->tag == Ast_Term::Tag::Enum)
	{
		debug_print_branch(depth);
		printf("Term_Enum: ");
	}
	else if (term->tag == Ast_Term::Tag::Literal)
	{
		debug_print_branch(depth);
		printf("Term_Literal: ");
	}

	switch (term->tag)
	{
		case Ast_Term::Tag::Var: debug_print_var(term->as_var); break;
		case Ast_Term::Tag::Enum: debug_print_enum(term->as_enum); break;
		case Ast_Term::Tag::Literal: debug_print_token(term->as_literal.token, true); break;
		case Ast_Term::Tag::Proc_Call: debug_print_proc_call(term->as_proc_call, depth); break;
	}
}

void debug_print_expr(Ast_Expr* expr, u32 depth)
{
	switch (expr->tag)
	{
		case Ast_Expr::Tag::Term: debug_print_term(expr->as_term, depth); break;
		case Ast_Expr::Tag::Unary_Expr: debug_print_unary_expr(expr->as_unary_expr, depth); break;
		case Ast_Expr::Tag::Binary_Expr: debug_print_binary_expr(expr->as_binary_expr, depth); break;
	}
}

void debug_print_unary_expr(Ast_Unary_Expr* unary_expr, u32 depth)
{
	debug_print_branch(depth);
	printf("Unary_Expr\n");

	debug_print_spacing(depth);
	debug_print_unary_op(unary_expr->op);
	debug_print_expr(unary_expr->right, depth);
}

void debug_print_binary_expr(Ast_Binary_Expr* binary_expr, u32 depth)
{
	debug_print_branch(depth);
	printf("Binary_Expr\n");

	debug_print_spacing(depth);
	debug_print_binary_op(binary_expr->op);
	debug_print_expr(binary_expr->left, depth);
	debug_print_expr(binary_expr->right, depth);
}

void debug_print_block(Ast_Block* block, u32 depth)
{
	debug_print_branch(depth);
	printf("Block: ");

	if (!block->statements.empty())
	{
		printf("\n");
		for (Ast_Statement* statement : block->statements)
		debug_print_statement(statement, depth);
	}
	else printf("---\n");
}

void debug_print_statement(Ast_Statement* statement, u32 depth)
{
	switch (statement->tag)
	{
		case Ast_Statement::Tag::If: debug_print_if(statement->as_if, depth); break;
		case Ast_Statement::Tag::For: debug_print_for(statement->as_for, depth); break;
		case Ast_Statement::Tag::Block: debug_print_block(statement->as_block, depth); break;
		case Ast_Statement::Tag::Defer: debug_print_defer(statement->as_defer, depth); break;
		case Ast_Statement::Tag::Break: debug_print_break(statement->as_break, depth); break;
		case Ast_Statement::Tag::Return: debug_print_return(statement->as_return, depth); break;
		case Ast_Statement::Tag::Continue: debug_print_continue(statement->as_continue, depth); break;
		case Ast_Statement::Tag::Proc_Call: debug_print_proc_call(statement->as_proc_call, depth); break;
		case Ast_Statement::Tag::Var_Decl: debug_print_var_decl(statement->as_var_decl, depth); break;
		case Ast_Statement::Tag::Var_Assign: debug_print_var_assign(statement->as_var_assign, depth); break;
	}
}

void debug_print_if(Ast_If* _if, u32 depth)
{
	debug_print_branch(depth);
	printf("If\n");

	debug_print_spacing(depth);
	printf("If_Conditional_Expr:\n");
	debug_print_expr(_if->condition_expr, depth);

	debug_print_block(_if->block, depth);
	if (_if->_else.has_value())
	debug_print_else(_if->_else.value(), depth);
}

void debug_print_else(Ast_Else* _else, u32 depth)
{
	debug_print_branch(depth);
	printf("Else\n");

	switch (_else->tag)
	{
		case Ast_Else::Tag::If: debug_print_if(_else->as_if, depth); break;
		case Ast_Else::Tag::Block: debug_print_block(_else->as_block, depth); break;
	}
}

void debug_print_for(Ast_For* _for, u32 depth)
{
	debug_print_branch(depth);
	printf("For\n");

	debug_print_spacing(depth);
	printf("For_Var_Decl: ");
	if (_for->var_decl.has_value())
	{
		printf("\n");
		debug_print_var_decl(_for->var_decl.value(), depth);
	}
	else printf("---\n");

	debug_print_spacing(depth);
	printf("For_Conditional_Expr: ");
	if (_for->condition_expr.has_value())
	{
		printf("\n");
		debug_print_expr(_for->condition_expr.value(), depth);
	}
	else printf("---\n");

	debug_print_spacing(depth);
	printf("For_Var_Assign: ");
	if (_for->var_assign.has_value())
	{
		printf("\n");
		debug_print_var_assign(_for->var_assign.value(), depth);
	}
	else printf("---\n");

	debug_print_spacing(depth);
	printf("For_Block:\n");
	debug_print_block(_for->block, depth);
}

void debug_print_defer(Ast_Defer* defer, u32 depth)
{
	debug_print_branch(depth);
	printf("Defer\n");

	debug_print_spacing(depth);
	printf("Defer_Block:\n");
	debug_print_block(defer->block, depth);
}

void debug_print_break(Ast_Break* _break, u32 depth)
{
	debug_print_branch(depth);
	printf("Break\n");
	(void)_break;
}

void debug_print_return(Ast_Return* _return, u32 depth)
{
	debug_print_branch(depth);
	printf("Return: ");

	if (_return->expr.has_value())
	{
		printf("\n");
		debug_print_expr(_return->expr.value(), depth);
	}
	else printf("---\n");
}

void debug_print_continue(Ast_Continue* _continue, u32 depth)
{
	debug_print_branch(depth);
	printf("Continue\n");
	(void)_continue;
}

void debug_print_proc_call(Ast_Proc_Call* proc_call, u32 depth)
{
	debug_print_branch(depth);
	printf("Proc_Call: ");
	if (proc_call->import)
	{
		debug_print_ident(proc_call->import.value(), false, false);
		printf(".");
	}
	debug_print_ident(proc_call->ident, true, false);

	debug_print_spacing(depth);
	printf("Access: ");
	if (proc_call->access)
	{
		debug_print_access(proc_call->access.value());
		printf("\n");
	}
	else printf("---\n");

	debug_print_spacing(depth);
	printf("Input_Exprs: ");
	if (!proc_call->input_exprs.empty())
	{
		printf("\n");
		for (Ast_Expr* expr : proc_call->input_exprs)
		debug_print_expr(expr, depth);
	}
	else printf("---\n");
}

void debug_print_var_decl(Ast_Var_Decl* var_decl, u32 depth)
{
	debug_print_branch(depth);
	printf("Var_Decl: ");
	debug_print_ident(var_decl->ident, false, false);
	printf(": ");
	if (var_decl->type.has_value())
	{
		debug_print_type(var_decl->type.value());
		printf("\n");
	}
	else printf("[?]\n");

	debug_print_spacing(depth);
	printf("Var_Decl_Expr: ");
	if (var_decl->expr.has_value())
	{
		printf("\n");
		debug_print_expr(var_decl->expr.value(), depth);
	}
	else printf("---\n");
}

void debug_print_var_assign(Ast_Var_Assign* var_assign, u32 depth)
{
	debug_print_branch(depth);
	printf("Var_Assign\n");

	debug_print_spacing(depth);
	printf("Var: ");
	debug_print_var(var_assign->var);
	debug_print_spacing(depth);
	debug_print_assign_op(var_assign->op);
	debug_print_expr(var_assign->expr, depth);
}
