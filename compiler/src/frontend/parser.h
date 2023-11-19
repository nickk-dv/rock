#ifndef PARSER_H
#define PARSER_H

#include "ast.h"
#include "tokenizer.h"
#include "general/arena.h"

struct Parser;

Ast_Program* parse_program(Parser* parser);

static Ast* parse_ast(Parser* parser, StringView source, std::string& filepath);
static option<Ast_Type> parse_type(Parser* parser);
static Ast_Type_Array* parse_type_array(Parser* parser);
static Ast_Type_Procedure* parse_type_procedure(Parser* parser);
static Ast_Type_Unresolved* parse_type_unresolved(Parser* parser);

static Ast_Decl_Use* parse_decl_use(Parser* parser);
static Ast_Decl_Proc* parse_decl_proc(Parser* parser);
static Ast_Decl_Enum* parse_decl_enum(Parser* parser);
static Ast_Decl_Struct* parse_decl_struct(Parser* parser);
static Ast_Decl_Global* parse_decl_global(Parser* parser);
static Ast_Decl_Import* parse_decl_import(Parser* parser);

static Ast_Stmt* parse_stmt(Parser* parser);
static Ast_Stmt_If* parse_stmt_if(Parser* parser);
static Ast_Else* parse_else(Parser* parser);
static Ast_Stmt_For* parse_stmt_for(Parser* parser);
static Ast_Stmt_Block* parse_stmt_block(Parser* parser);
static Ast_Stmt_Block* parse_stmt_block_short(Parser * parser);
static Ast_Stmt_Defer* parse_stmt_defer(Parser* parser);
static Ast_Stmt_Break* parse_stmt_break(Parser* parser);
static Ast_Stmt_Return* parse_stmt_return(Parser* parser);
static Ast_Stmt_Switch* parse_stmt_switch(Parser* parser);
static Ast_Stmt_Continue* parse_stmt_continue(Parser* parser);
static Ast_Stmt_Var_Decl* parse_stmt_var_decl(Parser* parser);
static Ast_Stmt_Var_Assign* parse_stmt_var_assign(Parser* parser);
static Ast_Proc_Call* parse_proc_call(Parser* parser, bool import);

static Ast_Expr* parse_expr(Parser* parser);
static Ast_Expr* parse_sub_expr(Parser* parser, u32 min_prec = 0);
static Ast_Expr* parse_primary_expr(Parser* parser);
static Ast_Consteval_Expr* parse_consteval_expr(Parser* parser, Ast_Expr* expr);
static Ast_Term* parse_term(Parser* parser);
static Ast_Var* parse_var(Parser* parser);
static Ast_Access* parse_access(Parser* parser);
static Ast_Access_Var* parse_access_var(Ast_Access* target, Parser* parser);
static Ast_Access_Array* parse_access_array(Ast_Access* target, Parser* parser);
static Ast_Enum* parse_enum(Parser* parser, bool import);
static Ast_Cast* parse_cast(Parser* parser);
static Ast_Sizeof* parse_sizeof(Parser* parser);
static Ast_Struct_Init* parse_struct_init(Parser* parser, bool import, bool type);
static Ast_Array_Init* parse_array_init(Parser* parser);

static Token peek_token(Parser* parser, u32 offset);
static void consume_token(Parser* parser);
static Token consume_get_token(Parser* parser);
static option<Token> try_consume_token(Parser* parser, TokenType token_type);
static u32 get_span_start(Parser* parser);
static u32 get_span_end(Parser* parser);
static void err_parse(Parser* parser, TokenType expected, option<const char*> in, u32 offset = 0);

struct Parser
{
	Ast* ast;
	Arena arena;
	Tokenizer tokenizer;
	StringStorage strings;
	u32 peek_index = 0;
	Token prev_last;
	Token tokens[TOKENIZER_BUFFER_SIZE];
};

#endif
