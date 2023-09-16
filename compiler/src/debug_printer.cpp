
void debug_print_ast(Ast* ast);
void debug_print_token(Token token, bool endl, bool location = false);

void debug_print_unary_op(UnaryOp op);
void debug_print_binary_op(BinaryOp op);
void debug_print_branch(u32& depth);
void debug_print_spacing(u32 depth);
void debug_print_access_chain(Ast_Access_Chain* access_chain);
void debug_print_term(Ast_Term* term, u32 depth);
void debug_print_expr(Ast_Expression* expr, u32 depth);
void debug_print_unary_expr(Ast_Unary_Expression* unary_expr, u32 depth);
void debug_print_binary_expr(Ast_Binary_Expression* bin_expr, u32 depth);
void debug_print_block(Ast_Block* block, u32 depth);
void debug_print_statement(Ast_Statement* statement, u32 depth);
void debug_print_if(Ast_If* _if, u32 depth);
void debug_print_else(Ast_Else* _else, u32 depth);
void debug_print_for(Ast_For* _for, u32 depth);
void debug_print_break(Ast_Break* _break, u32 depth);
void debug_print_return(Ast_Return* _return, u32 depth);
void debug_print_continue(Ast_Continue* _continue, u32 depth);
void debug_print_proc_call(Ast_Procedure_Call* _proc_call, u32 depth);
void debug_print_var_assign(Ast_Variable_Assignment* _var_assign, u32 depth);
void debug_print_var_decl(Ast_Variable_Declaration* _var_decl, u32 depth);

void debug_print_ast(Ast* ast)
{
	printf("\n[AST]\n");

	for (const Ast_Struct_Declaration& decl : ast->structs)
	{
		printf("\n[Struct] "); debug_print_token(decl.type.token, true);
		for (const IdentTypePair& field : decl.fields)
		{
			debug_print_token(field.ident.token, false);
			printf(": "); debug_print_token(field.type.token, true);
		}
	}

	for (const Ast_Enum_Declaration& decl : ast->enums)
	{
		printf("\n[Enum] "); debug_print_token(decl.type.token, true);
		for (const IdentTypePair& field : decl.variants)
		{
			debug_print_token(field.ident.token, true);
		}
	}

	for (const Ast_Procedure_Declaration& decl : ast->procedures)
	{
		printf("\n[Procedure] ");
		debug_print_token(decl.ident.token, true);

		printf("Params: ");
		if (!decl.input_parameters.empty())
		{
			printf("\n");
			for (const IdentTypePair& param : decl.input_parameters)
			{
				debug_print_token(param.ident.token, false);
				printf(": "); debug_print_token(param.type.token, true);
			}
		}
		else printf("---\n");

		printf("Return: ");
		if (decl.return_type.has_value())
		{
			debug_print_token(decl.return_type.value().token, true);
		}
		else printf("---\n");

		debug_print_block(decl.block, 0);
	}
}

void debug_print_token(Token token, bool endl, bool location)
{
	if (location) printf("l: %lu c: %lu", token.l0, token.c0);

	if (token.type == TOKEN_IDENT || token.type == TOKEN_STRING)
	{
		for (u64 i = 0; i < token.string_value.count; i++)
			printf("%c", token.string_value.data[i]);
	}
	else if (token.type == TOKEN_NUMBER)
	{
		printf("%llu", token.integer_value);
		//@Incomplete need to lex f32 f64 and store numeric flags
	}
	else if (token.type == TOKEN_BOOL_LITERAL)
	{
		if (token.bool_value)
			printf("true");
		else printf("false");
	}
	else
	{
		switch (token.type) 
		{
			case TOKEN_KEYWORD_STRUCT: printf("struct"); break;
			case TOKEN_KEYWORD_ENUM: printf("enum"); break;
			case TOKEN_KEYWORD_FN: printf("fn"); break;
			case TOKEN_KEYWORD_IF: printf("if"); break;
			case TOKEN_KEYWORD_ELSE: printf("else"); break;
			case TOKEN_KEYWORD_TRUE: printf("true"); break;
			case TOKEN_KEYWORD_FALSE: printf("false"); break;
			case TOKEN_KEYWORD_FOR: printf("for"); break;
			case TOKEN_KEYWORD_BREAK: printf("break"); break;
			case TOKEN_KEYWORD_RETURN: printf("return"); break;
			case TOKEN_KEYWORD_CONTINUE: printf("continue"); break;

			case TOKEN_DOT: printf("."); break;
			case TOKEN_COMMA: printf(","); break;
			case TOKEN_COLON: printf(":"); break;
			case TOKEN_SEMICOLON: printf(";"); break;
			case TOKEN_BLOCK_START: printf("{"); break;
			case TOKEN_BLOCK_END: printf("}"); break;
			case TOKEN_BRACKET_START: printf("["); break;
			case TOKEN_BRACKET_END: printf("]"); break;
			case TOKEN_PAREN_START: printf("("); break;
			case TOKEN_PAREN_END: printf(")"); break;
			case TOKEN_DOUBLE_COLON: printf("::"); break;

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

			case TOKEN_INPUT_END: printf("Input end"); break;
			case TOKEN_ERROR: printf("Token Error"); break;

			default: printf("[UNKNOWN TOKEN]"); break;
		}
	}

	if (endl) printf("\n");
}

void debug_print_unary_op(UnaryOp op)
{
	printf("UnaryOp: ");
	switch (op)
	{
		case UNARY_OP_MINUS: printf("-"); break;
		case UNARY_OP_LOGIC_NOT: printf("!"); break;
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

void debug_print_access_chain(Ast_Access_Chain* access_chain)
{
	while (access_chain != NULL)
	{
		debug_print_token(access_chain->ident.token, false);
		access_chain = access_chain->next;
		if (access_chain != NULL)
		printf(".");
	}
	printf("\n");
}

void debug_print_term(Ast_Term* term, u32 depth)
{
	if (term->tag == Ast_Term::Tag::AccessChain)
	{
		debug_print_branch(depth);
		printf("Term_Access_Chain: ");
	}
	else if (term->tag == Ast_Term::Tag::Literal)
	{
		debug_print_branch(depth);
		printf("Term_Liter: ");
	}

	switch (term->tag)
	{
		case Ast_Term::Tag::Literal: debug_print_token(term->_literal.token, true); break;
		case Ast_Term::Tag::AccessChain: debug_print_access_chain(term->_access_chain); break;
		case Ast_Term::Tag::ProcedureCall: debug_print_proc_call(term->_proc_call, depth); break;
		default: break;
	}
}

void debug_print_expr(Ast_Expression* expr, u32 depth)
{
	switch (expr->tag)
	{
		case Ast_Expression::Tag::Term: debug_print_term(expr->_term, depth); break;
		case Ast_Expression::Tag::UnaryExpression: debug_print_unary_expr(expr->_unary_expr, depth); break;
		case Ast_Expression::Tag::BinaryExpression: debug_print_binary_expr(expr->_bin_expr, depth); break;
		default: break;
	}
}

void debug_print_unary_expr(Ast_Unary_Expression* unary_expr, u32 depth)
{
	debug_print_branch(depth);
	printf("Unary_Expr\n");

	debug_print_spacing(depth);
	debug_print_unary_op(unary_expr->op);
	debug_print_expr(unary_expr->right, depth);
}

void debug_print_binary_expr(Ast_Binary_Expression* bin_expr, u32 depth)
{
	debug_print_branch(depth);
	printf("Bin_Expr\n");

	debug_print_spacing(depth);
	debug_print_binary_op(bin_expr->op);
	debug_print_expr(bin_expr->left, depth);
	debug_print_expr(bin_expr->right, depth);
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
		case Ast_Statement::Tag::If: debug_print_if(statement->_if, depth); break;
		case Ast_Statement::Tag::For: debug_print_for(statement->_for, depth); break;
		case Ast_Statement::Tag::Break: debug_print_break(statement->_break, depth); break;
		case Ast_Statement::Tag::Return: debug_print_return(statement->_return, depth); break;
		case Ast_Statement::Tag::Continue: debug_print_continue(statement->_continue, depth); break;
		case Ast_Statement::Tag::ProcedureCall: debug_print_proc_call(statement->_proc_call, depth); break;
		case Ast_Statement::Tag::VariableAssignment: debug_print_var_assign(statement->_var_assignment, depth); break;
		case Ast_Statement::Tag::VariableDeclaration: debug_print_var_decl(statement->_var_declaration, depth); break;
		default: break;
	}
}

void debug_print_if(Ast_If* _if, u32 depth)
{
	debug_print_branch(depth);
	printf("If\n");

	debug_print_spacing(depth);
	printf("If_Conditional_Expr:\n");
	debug_print_expr(_if->expr, depth);

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
		case Ast_Else::Tag::If: debug_print_if(_else->_if, depth); break;
		case Ast_Else::Tag::Block: debug_print_block(_else->_block, depth); break;
		default: break;
	}
}

void debug_print_for(Ast_For* _for, u32 depth)
{
	debug_print_branch(depth);
	printf("For\n");

	debug_print_spacing(depth);
	printf("For_Var_Declaration: ");
	if (_for->var_declaration.has_value())
	{
		printf("\n");
		debug_print_var_decl(_for->var_declaration.value(), depth);
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
	printf("For_Post_Expr: ");
	if (_for->post_expr.has_value())
	{
		printf("\n");
		debug_print_expr(_for->post_expr.value(), depth);
	}
	else printf("---\n");

	debug_print_spacing(depth);
	printf("For_Block:\n");
	debug_print_block(_for->block, depth);
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

void debug_print_proc_call(Ast_Procedure_Call* _proc_call, u32 depth)
{
	debug_print_branch(depth);
	printf("Procedure_Call: ");
	debug_print_token(_proc_call->ident.token, true);

	debug_print_spacing(depth);
	printf("Input_Exprs: ");
	if (!_proc_call->input_expressions.empty())
	{
		printf("\n");
		for (Ast_Expression* expr : _proc_call->input_expressions)
		debug_print_expr(expr, depth);
	}
	else printf("---\n");
}

void debug_print_var_assign(Ast_Variable_Assignment* _var_assign, u32 depth)
{
	debug_print_branch(depth);
	printf("Var_Assignment: ");

	debug_print_access_chain(_var_assign->access_chain);
	debug_print_expr(_var_assign->expr, depth);
}

void debug_print_var_decl(Ast_Variable_Declaration* _var_decl, u32 depth)
{
	debug_print_branch(depth);
	printf("Var_Declaration: ");
	debug_print_token(_var_decl->ident.token, false);
	printf(": ");
	if (_var_decl->type.has_value())
		debug_print_token(_var_decl->type.value().token, true);
	else printf("[?]\n");

	debug_print_spacing(depth);
	printf("Var_Declaration_Expr: ");
	if (_var_decl->expr.has_value())
	{
		printf("\n");
		debug_print_expr(_var_decl->expr.value(), depth);
	}
	else printf("---\n");
}
