#ifndef PARSER_H
#define PARSER_H

#include "ast.h"
#include "lexer.h"
#include "general/arena.h"

struct Parser
{
private:
	Ast* ast;
	Arena arena;
	Lexer lexer;
	StringStorage strings;
	u32 peek_index = 0;
	Token prev_last;
	Token tokens[Lexer::TOKEN_BUFFER_SIZE];

public:
	Ast_Program* parse_program();

private:
	Ast* parse_ast(StringView source, std::string& filepath);
	option<Ast_Type> parse_type();
	Ast_Type_Array* parse_type_array();
	Ast_Type_Procedure* parse_type_procedure();
	Ast_Type_Unresolved* parse_type_unresolved();

	Ast_Decl_Impl* parse_decl_impl();
	Ast_Decl_Use* parse_decl_use();
	Ast_Decl_Proc* parse_decl_proc(bool in_impl);
	Ast_Decl_Enum* parse_decl_enum();
	Ast_Decl_Struct* parse_decl_struct();
	Ast_Decl_Global* parse_decl_global();
	Ast_Decl_Import* parse_decl_import();

	Ast_Stmt* parse_stmt();
	Ast_Stmt_If* parse_stmt_if();
	Ast_Else* parse_else();
	Ast_Stmt_For* parse_stmt_for();
	Ast_Stmt_Block* parse_stmt_block();
	Ast_Stmt_Block* parse_stmt_block_short();
	Ast_Stmt_Defer* parse_stmt_defer();
	Ast_Stmt_Break* parse_stmt_break();
	Ast_Stmt_Return* parse_stmt_return();
	Ast_Stmt_Switch* parse_stmt_switch();
	Ast_Stmt_Continue* parse_stmt_continue();
	Ast_Stmt_Var_Decl* parse_stmt_var_decl();
	Ast_Stmt_Var_Assign* parse_stmt_var_assign();
	Ast_Proc_Call* parse_proc_call(bool import);

	Ast_Expr* parse_expr();
	Ast_Expr* parse_sub_expr(u32 min_prec = 0);
	Ast_Expr* parse_primary_expr();
	Ast_Consteval_Expr* parse_consteval_expr(Ast_Expr* expr);
	Ast_Term* parse_term();
	Ast_Var* parse_var();
	Ast_Access* parse_access();
	Ast_Access_Var* parse_access_var(Ast_Access* target);
	Ast_Access_Array* parse_access_array(Ast_Access* target);
	Ast_Enum* parse_enum(bool import);
	Ast_Cast* parse_cast();
	Ast_Sizeof* parse_sizeof();
	Ast_Struct_Init* parse_struct_init(bool import, bool type);
	Ast_Array_Init* parse_array_init();

	//@new syntax
	option<Ast_Module_Access*> parse_module_access();
	Ast_Decl_Import_New* parse_decl_import_new();

	Token peek(u32 offset = 0);
	void consume();
	Token consume_get();
	option<Token> try_consume(TokenType token_type);
	u32 get_span_start();
	u32 get_span_end();
	void err_parse(TokenType expected, option<const char*> in, u32 offset = 0);
};

#endif
