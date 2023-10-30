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
	if (location) printf("%lu:%lu ", token.span.start, token.span.end);

	if (token.type == TokenType::IDENT)
	{
		for (u64 i = 0; i < token.string_value.count; i++)
			printf("%c", token.string_value.data[i]);
	}
	else if (token.type == TokenType::BOOL_LITERAL)
	{
		if (token.bool_value)
			printf("true");
		else printf("false");
	}
	else if (token.type == TokenType::FLOAT_LITERAL)
	{
		printf("%f", token.float64_value);
	}
	else if (token.type == TokenType::INTEGER_LITERAL)
	{
		printf("%llu", token.integer_value);
	}
	else if (token.type == TokenType::STRING_LITERAL)
	{
		printf("%s", token.string_literal_value); //@Later print raw string without escape characters
	}
	else
	{
		switch (token.type) 
		{
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

		default: printf("[UNKNOWN TOKEN]"); break;
		}
	}

	if (endl) printf("\n");
}

void debug_print_ident(Ast_Ident ident, bool endl, bool location)
{
	if (location) printf("%lu:%lu ", ident.span.start, ident.span.end);
	
	for (u64 i = 0; i < ident.str.count; i++)
		printf("%c", ident.str.data[i]);
	
	if (endl) printf("\n");
}

void debug_print_unary_op(UnaryOp op)
{
	printf("UnaryOp: ");
	switch (op)
	{
	case UnaryOp::MINUS: printf("-"); break;
	case UnaryOp::LOGIC_NOT: printf("!"); break;
	case UnaryOp::BITWISE_NOT: printf("~"); break;
	case UnaryOp::ADDRESS_OF: printf("*"); break;
	case UnaryOp::DEREFERENCE: printf("<<"); break;
	default: printf("[UNKNOWN UNARY OP]"); break;
	}
	printf("\n");
}

void debug_print_binary_op(BinaryOp op)
{
	printf ("BinaryOp: ");
	switch (op)
	{
	case BinaryOp::LOGIC_AND: printf("&&"); break;
	case BinaryOp::LOGIC_OR: printf("||"); break;
	case BinaryOp::LESS: printf("<"); break;
	case BinaryOp::GREATER: printf(">"); break;
	case BinaryOp::LESS_EQUALS: printf("<="); break;
	case BinaryOp::GREATER_EQUALS: printf(">="); break;
	case BinaryOp::IS_EQUALS: printf("=="); break;
	case BinaryOp::NOT_EQUALS: printf("!="); break;
	case BinaryOp::PLUS: printf("+"); break;
	case BinaryOp::MINUS: printf("-"); break;
	case BinaryOp::TIMES: printf("*"); break;
	case BinaryOp::DIV: printf("/"); break;
	case BinaryOp::MOD: printf("%%"); break;
	case BinaryOp::BITWISE_AND: printf("&"); break;
	case BinaryOp::BITWISE_OR: printf("|"); break;
	case BinaryOp::BITWISE_XOR: printf("^"); break;
	case BinaryOp::BITSHIFT_LEFT: printf("<<"); break;
	case BinaryOp::BITSHIFT_RIGHT: printf(">>"); break;
	default: printf("[UNKNOWN BINARY OP]"); break;
	}
	printf("\n");
}

void debug_print_assign_op(AssignOp op)
{
	printf("AssignOp: ");
	switch (op)
	{
	case AssignOp::NONE: printf("="); break;
	case AssignOp::PLUS: printf("+="); break;
	case AssignOp::MINUS: printf("-="); break;
	case AssignOp::TIMES: printf("*="); break;
	case AssignOp::DIV: printf("/="); break;
	case AssignOp::MOD: printf("%%="); break;
	case AssignOp::BITWISE_AND: printf("&="); break;
	case AssignOp::BITWISE_OR: printf("|="); break;
	case AssignOp::BITWISE_XOR: printf("^="); break;
	case AssignOp::BITSHIFT_LEFT: printf("<<="); break;
	case AssignOp::BITSHIFT_RIGHT: printf(">>="); break;
	default: printf("[UNKNOWN ASSIGN OP]"); break;
	}
	printf("\n");
}

void debug_print_basic_type(BasicType type)
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
	default: printf("[UNKNOWN BASIC TYPE]"); break;
	}
}

void debug_print_type(Ast_Type type)
{
	for (u32 i = 0; i < type.pointer_level; i += 1)
	{
		printf("*");
	}

	switch (type.tag)
	{
	case Ast_Type_Tag::Basic: debug_print_basic_type(type.as_basic); break;
	case Ast_Type_Tag::Array: debug_print_array_type(type.as_array); break;
	case Ast_Type_Tag::Custom: debug_print_custom_type(type.as_custom); break;
	case Ast_Type_Tag::Struct: debug_print_ident(type.as_struct.struct_decl->ident, false, false); break;
	case Ast_Type_Tag::Enum: debug_print_ident(type.as_enum.enum_decl->ident, false, false); break;
	}
}

void debug_print_array_type(Ast_Array_Type* array_type)
{
	printf("[size expr]");
	debug_print_type(array_type->element_type);
}

void debug_print_custom_type(Ast_Custom_Type* custom_type)
{
	if (custom_type->import)
	{
		debug_print_ident(custom_type->import.value(), false, false);
		printf(".");
	}
	debug_print_ident(custom_type->ident, false, false);
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
	debug_print_ident(struct_decl->ident, true, false);

	for (const Ast_Struct_Field& field : struct_decl->fields)
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
	debug_print_ident(enum_decl->ident, false, false);
	
	printf (": ");
	debug_print_basic_type(enum_decl->basic_type);
	printf("\n");

	for (const Ast_Enum_Variant& variant : enum_decl->variants)
	{
		debug_print_ident(variant.ident, false, false);
		printf("Variant Expr: \n");
		debug_print_expr(variant.const_expr.expr, 0);
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
		for (const Ast_Proc_Param& param : proc_decl->input_params)
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

	if (proc_decl->is_external) printf("External\n");
	else debug_print_block(proc_decl->block, 0);
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
	case Ast_Statement_Tag::If: debug_print_if(statement->as_if, depth); break;
	case Ast_Statement_Tag::For: debug_print_for(statement->as_for, depth); break;
	case Ast_Statement_Tag::Block: debug_print_block(statement->as_block, depth); break;
	case Ast_Statement_Tag::Defer: debug_print_defer(statement->as_defer, depth); break;
	case Ast_Statement_Tag::Break: debug_print_break(statement->as_break, depth); break;
	case Ast_Statement_Tag::Return: debug_print_return(statement->as_return, depth); break;
	case Ast_Statement_Tag::Switch: debug_print_switch(statement->as_switch, depth); break;
	case Ast_Statement_Tag::Continue: debug_print_continue(statement->as_continue, depth); break;
	case Ast_Statement_Tag::Var_Decl: debug_print_var_decl(statement->as_var_decl, depth); break;
	case Ast_Statement_Tag::Var_Assign: debug_print_var_assign(statement->as_var_assign, depth); break;
	case Ast_Statement_Tag::Proc_Call: debug_print_proc_call(statement->as_proc_call, depth); break;
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
	case Ast_Else_Tag::If: debug_print_if(_else->as_if, depth); break;
	case Ast_Else_Tag::Block: debug_print_block(_else->as_block, depth); break;
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

void debug_print_switch(Ast_Switch* _switch, u32 depth)
{
	debug_print_branch(depth);
	printf("Switch: \n");

	debug_print_spacing(depth);
	printf("On_Expr: \n");
	debug_print_expr(_switch->expr, depth);
	
	for (Ast_Switch_Case& _case : _switch->cases)
	{
		debug_print_spacing(depth);
		printf("Case: \n");
		debug_print_expr(_case.const_expr.expr, depth);
		if (_case.block)
		debug_print_block(_case.block.value(), depth);
	}
}

void debug_print_continue(Ast_Continue* _continue, u32 depth)
{
	debug_print_branch(depth);
	printf("Continue\n");
	(void)_continue;
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

void debug_print_expr(Ast_Expr* expr, u32 depth)
{
	switch (expr->tag)
	{
	case Ast_Expr_Tag::Term: debug_print_term(expr->as_term, depth); break;
	case Ast_Expr_Tag::Unary_Expr: debug_print_unary_expr(expr->as_unary_expr, depth); break;
	case Ast_Expr_Tag::Binary_Expr: debug_print_binary_expr(expr->as_binary_expr, depth); break;
	case Ast_Expr_Tag::Folded_Expr: debug_print_folded_expr(expr->as_folded_expr, depth); break;
	}
}

void debug_print_term(Ast_Term* term, u32 depth)
{
	if (term->tag == Ast_Term_Tag::Var)
	{
		debug_print_branch(depth);
		printf("Term_Var: ");
	}
	else if (term->tag == Ast_Term_Tag::Enum)
	{
		debug_print_branch(depth);
		printf("Term_Enum: ");
	}
	else if (term->tag == Ast_Term_Tag::Sizeof)
	{
		debug_print_branch(depth);
		printf("Term_Sizeof: ");
	}
	else if (term->tag == Ast_Term_Tag::Literal)
	{
		debug_print_branch(depth);
		printf("Term_Literal: ");
	}

	switch (term->tag)
	{
	case Ast_Term_Tag::Var: debug_print_var(term->as_var); break;
	case Ast_Term_Tag::Enum: debug_print_enum(term->as_enum); break;
	case Ast_Term_Tag::Sizeof: debug_print_sizeof(term->as_sizeof); break;
	case Ast_Term_Tag::Literal: debug_print_token(term->as_literal.token, true); break;
	case Ast_Term_Tag::Proc_Call: debug_print_proc_call(term->as_proc_call, depth); break;
	case Ast_Term_Tag::Struct_Init: debug_print_struct_init(term->as_struct_init, depth); break;
	case Ast_Term_Tag::Array_Init: debug_print_array_init(term->as_array_init, depth); break;
	}
}

void debug_print_var(Ast_Var* var)
{
	debug_print_ident(var->ident, false, false);
	if (var->access) debug_print_access(var->access.value());
	printf("\n");
}

void debug_print_access(Ast_Access* access)
{
	if (access->tag == Ast_Access_Tag::Var)
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
	debug_print_ident(_enum->ident, false, false);
	printf("::");
	debug_print_ident(_enum->variant, true, false);
}

void debug_print_sizeof(Ast_Sizeof* _sizeof)
{
	debug_print_type(_sizeof->type);
}

void debug_print_struct_init(Ast_Struct_Init* struct_init, u32 depth)
{
	debug_print_branch(depth);
	printf("Struct_Init\n");

	debug_print_spacing(depth);
	printf("Struct_Type: ");
	if (struct_init->import)
	{
		debug_print_ident(struct_init->import.value(), false, false);
		printf(".");
	}
	if (struct_init->ident)
	{
		debug_print_ident(struct_init->ident.value(), true, false);
	}
	else printf("[?]\n");
	
	debug_print_spacing(depth);
	printf("Input_Exprs: ");
	if (!struct_init->input_exprs.empty())
	{
		printf("\n");
		for (Ast_Expr* expr : struct_init->input_exprs)
		debug_print_expr(expr, depth);
	}
	else printf("---\n");
}

void debug_print_array_init(Ast_Array_Init* array_init, u32 depth)
{
	debug_print_branch(depth);
	printf("Array_Init\n");

	debug_print_spacing(depth);
	printf("Array_Type: ");
	if (array_init->type)
		debug_print_type(array_init->type.value());
	else printf("[?]");
	printf("\n");

	debug_print_spacing(depth);
	printf("Input_Exprs: ");
	if (!array_init->input_exprs.empty())
	{
		printf("\n");
		for (Ast_Expr* expr : array_init->input_exprs)
		debug_print_expr(expr, depth);
	}
	else printf("---\n");
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

void debug_print_folded_expr(Ast_Folded_Expr folded_expr, u32 depth)
{
	debug_print_branch(depth);
	printf("Const_Expr: ");

	switch (folded_expr.basic_type)
	{
	case BasicType::BOOL:
	{
		if (folded_expr.as_bool) printf("true");
		else printf("false");
	} break;
	case BasicType::F32:
	case BasicType::F64: 
	{
		printf("%f", folded_expr.as_f64);
		break;
	}
	case BasicType::I8:
	case BasicType::I16:
	case BasicType::I32:
	case BasicType::I64: 
	{
		printf("%lld", folded_expr.as_i64);
		break;
	}
	default:
	{
		printf("%llu", folded_expr.as_u64);
		break;
	}
	}
	printf("\n");
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
