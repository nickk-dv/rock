
void debug_print_ast(Ast* ast);
void debug_print_expr(Ast_Expression* expr, u32 depth);
void debug_print_binary_expr(Ast_Binary_Expression* expr, u32 depth);

void debug_print_ast(Ast* ast)
{
	printf("\n");
	printf("[AST]\n\n");

	u32 depth = 1;

	for (const auto& proc : ast->procedures)
	{
		printf("ident:  ");
		error_report_token_ident(proc.ident.token, true);
		printf("params: ");
		if (proc.input_parameters.empty())
		{
			printf("--\n");
		}
		else
		{
			printf("(");
			u32 count = 0;
			for (const auto& param : proc.input_parameters)
			{
				if (count != 0)
					printf(", ");
				count += 1;
				error_report_token_ident(param.ident.token);
				printf(": ");
				error_report_token_ident(param.type.token);
			}
			printf(")\n");
		}

		printf("return: ");
		if (proc.return_type.has_value())
		{
			error_report_token_ident(proc.return_type.value().token, true);
		}
		else printf("--\n");

		printf("Block\n");
		for (Ast_Statement* statement : proc.block->statements)
		{
			printf("|___");
			switch (statement->tag)
			{
			case Ast_Statement::Tag::If:
			{
				printf("If\n");
			} break;
			case Ast_Statement::Tag::For:
			{
				printf("For\n");
			} break;
			case Ast_Statement::Tag::While:
			{
				printf("While\n");
			} break;
			case Ast_Statement::Tag::Break:
			{
				printf("Break\n");
			} break;
			case Ast_Statement::Tag::Return:
			{
				printf("Return\n");
				if (statement->_return->expr.has_value())
					debug_print_expr(statement->_return->expr.value(), depth);
			} break;
			case Ast_Statement::Tag::Continue:
			{
				printf("Continue\n");
			} break;
			case Ast_Statement::Tag::ProcedureCall:
			{
				printf("ProcedureCall\n");
			} break;
			case Ast_Statement::Tag::VariableAssignment:
			{
				printf("VariableAssignment:  ");
				error_report_token_ident(statement->_var_assignment->ident.token, true);
				debug_print_expr(statement->_var_assignment->expr, depth);
			} break;
			case Ast_Statement::Tag::VariableDeclaration:
			{
				printf("VariableDeclaration: ");
				error_report_token_ident(statement->_var_declaration->ident.token);
				printf(", ");
				error_report_token_ident(statement->_var_declaration->type.token, true);
				if (statement->_var_declaration->expr.has_value())
					debug_print_expr(statement->_var_declaration->expr.value(), depth);
			} break;
			}
		}
		printf("\n");
	}
}
bool foo() { return true; }
void debug_print_expr(Ast_Expression* expr, u32 depth)
{
	for (u32 i = 0; i < depth; i++)
		printf("    ");
	printf("|___");

	switch (expr->tag)
	{
		case Ast_Expression::Tag::Term:
		{
			printf("Term_");
			switch (expr->_term->tag)
			{
			case Ast_Term::Tag::Literal:
			{
				printf("Literal: %llu\n", expr->_term->_literal.token.integer_value);
			} break;
			case Ast_Term::Tag::Identifier:
			{
				printf("Identifier: ");
				error_report_token_ident(expr->_term->_ident.token, true);
			} break;
			case Ast_Term::Tag::ProcedureCall:
			{
				printf("Procedure_Call\n");
			} break;
			}
		} break;
		case Ast_Expression::Tag::BinaryExpression:
		{
			printf("BinaryExpression\n");
			debug_print_binary_expr(expr->_bin_expr, depth + 1);
		} break;
	}
}

void debug_print_binary_expr(Ast_Binary_Expression* expr, u32 depth)
{
	for (u32 i = 0; i < depth; i++)
		printf("    ");
	printf("|___");
	printf("BinaryOp: %i\n", (int)expr->op);

	debug_print_expr(expr->left, depth);
	debug_print_expr(expr->right, depth);
}
