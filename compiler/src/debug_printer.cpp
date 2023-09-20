
void debug_print_ast(Ast* ast);
void debug_print_token(Token token, bool endl, bool location = false);

void debug_print_unary_op(UnaryOp op);
void debug_print_binary_op(BinaryOp op);
void debug_print_assign_op(AssignOp op);
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
void debug_print_proc_call(Ast_Proc_Call* proc_call, u32 depth);
void debug_print_var_assign(Ast_Var_Assign* var_assign, u32 depth);
void debug_print_var_declare(Ast_Var_Declare* var_declare, u32 depth);
void debug_print_tokenizer_info(const std::vector<Token>& tokens);

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
		if (!decl.input_params.empty())
		{
			printf("\n");
			for (const IdentTypePair& param : decl.input_params)
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
	if (location) printf("l: %lu c: %lu token: ", token.l0, token.c0);

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
	if (term->tag == Ast_Term::Tag::Access_Chain)
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
		case Ast_Term::Tag::Literal: debug_print_token(term->as_literal.token, true); break;
		case Ast_Term::Tag::Access_Chain: debug_print_access_chain(term->as_access_chain); break;
		case Ast_Term::Tag::Proc_Call: debug_print_proc_call(term->as_proc_call, depth); break;
		default: break;
	}
}

void debug_print_expr(Ast_Expression* expr, u32 depth)
{
	switch (expr->tag)
	{
		case Ast_Expression::Tag::Term: debug_print_term(expr->as_term, depth); break;
		case Ast_Expression::Tag::UnaryExpression: debug_print_unary_expr(expr->as_unary_expr, depth); break;
		case Ast_Expression::Tag::BinaryExpression: debug_print_binary_expr(expr->as_binary_expr, depth); break;
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
		case Ast_Statement::Tag::If: debug_print_if(statement->as_if, depth); break;
		case Ast_Statement::Tag::For: debug_print_for(statement->as_for, depth); break;
		case Ast_Statement::Tag::Break: debug_print_break(statement->as_break, depth); break;
		case Ast_Statement::Tag::Return: debug_print_return(statement->as_return, depth); break;
		case Ast_Statement::Tag::Continue: debug_print_continue(statement->as_continue, depth); break;
		case Ast_Statement::Tag::Proc_Call: debug_print_proc_call(statement->as_proc_call, depth); break;
		case Ast_Statement::Tag::Var_Assign: debug_print_var_assign(statement->as_var_assign, depth); break;
		case Ast_Statement::Tag::Var_Declare: debug_print_var_declare(statement->as_var_declare, depth); break;
		default: break;
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
		default: break;
	}
}

void debug_print_for(Ast_For* _for, u32 depth)
{
	debug_print_branch(depth);
	printf("For\n");

	debug_print_spacing(depth);
	printf("For_Var_Declaration: ");
	if (_for->var_declare.has_value())
	{
		printf("\n");
		debug_print_var_declare(_for->var_declare.value(), depth);
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
	debug_print_token(proc_call->ident.token, true);

	debug_print_spacing(depth);
	printf("Input_Exprs: ");
	if (!proc_call->input_expressions.empty())
	{
		printf("\n");
		for (Ast_Expression* expr : proc_call->input_expressions)
		debug_print_expr(expr, depth);
	}
	else printf("---\n");
}

void debug_print_var_assign(Ast_Var_Assign* var_assign, u32 depth)
{
	debug_print_branch(depth);
	printf("Var_Assign: ");

	debug_print_access_chain(var_assign->access_chain);
	debug_print_spacing(depth);
	debug_print_assign_op(var_assign->op);
	debug_print_expr(var_assign->expr, depth);
}

void debug_print_var_declare(Ast_Var_Declare* var_declare, u32 depth)
{
	debug_print_branch(depth);
	printf("Var_Declare: ");
	debug_print_token(var_declare->ident.token, false);
	printf(": ");
	if (var_declare->type.has_value())
		debug_print_token(var_declare->type.value().token, true);
	else printf("[?]\n");

	debug_print_spacing(depth);
	printf("Var_Declare_Expr: ");
	if (var_declare->expr.has_value())
	{
		printf("\n");
		debug_print_expr(var_declare->expr.value(), depth);
	}
	else printf("---\n");
}

void debug_print_tokenizer_info(const std::vector<Token>& tokens)
{
	u64 ident_count = 0;
	u64 number_count = 0;
	u64 string_count = 0;
	u64 keyword_count = 0;
	u64 symbol_count = 0;

	for (const Token& token : tokens)
	{
		if (token.type == TOKEN_IDENT) ident_count += 1;
		else if (token.type == TOKEN_NUMBER) number_count += 1;
		else if (token.type == TOKEN_STRING) string_count += 1;
		else if (token.type >= TOKEN_KEYWORD_STRUCT && token.type <= TOKEN_KEYWORD_CONTINUE) keyword_count += 1;
		else symbol_count += 1;
	}

	printf("\nTokenCount:      %llu \n", tokens.size());
	printf("[IdentCount]:    %llu \n", ident_count);
	printf("[NumberCount]:   %llu \n", number_count);
	printf("[StringCount]:   %llu \n", string_count);
	printf("[KeywordCount]:  %llu \n", keyword_count);
	printf("[SymbolCount]:   %llu \n", symbol_count);
	printf("MemoryUsed (Mb): %f \n", double(sizeof(Token) * tokens.size()) / (1024.0 * 1024.0));
	printf("MemoryCap  (Mb): %f \n\n", double(sizeof(Token) * tokens.capacity()) / (1024.0 * 1024.0));
}
