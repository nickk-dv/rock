#ifndef PARSER_H
#define PARSER_H

#include "ast.h"
#include "tokenizer.h"
#include "general/arena.h"

struct Parser;

Ast_Program* parse_program(Parser* parser);

static Ast* parse_ast(Parser* parser, StringView source, std::string& filepath);
static option<Ast_Type> parse_type(Parser* parser);
static Ast_Array_Type* parse_array_type(Parser* parser);
static Ast_Unresolved_Type* parse_unresolved_type(Parser* parser);

static Ast_Import_Decl* parse_import_decl(Parser* parser);
static Ast_Use_Decl* parse_use_decl(Parser* parser);
static Ast_Struct_Decl* parse_struct_decl(Parser* parser);
static Ast_Enum_Decl* parse_enum_decl(Parser* parser);
static Ast_Proc_Decl* parse_proc_decl(Parser* parser);
static Ast_Global_Decl* parse_global_decl(Parser* parser);

static Ast_Block* parse_block(Parser* parser);
static Ast_Block* parse_small_block(Parser * parser);
static Ast_Statement* parse_statement(Parser* parser);
static Ast_If* parse_if(Parser* parser);
static Ast_Else* parse_else(Parser* parser);
static Ast_For* parse_for(Parser* parser);
static Ast_Defer* parse_defer(Parser* parser);
static Ast_Break* parse_break(Parser* parser);
static Ast_Return* parse_return(Parser* parser);
static Ast_Switch* parse_switch(Parser* parser);
static Ast_Continue* parse_continue(Parser* parser);
static Ast_Var_Decl* parse_var_decl(Parser* parser);
static Ast_Var_Assign* parse_var_assign(Parser* parser);
static Ast_Proc_Call* parse_proc_call(Parser* parser, bool import);

static Ast_Expr* parse_expr(Parser* parser);
static Ast_Expr* parse_sub_expr(Parser* parser, u32 min_prec = 0);
static Ast_Expr* parse_primary_expr(Parser* parser);
static Ast_Consteval_Expr* parse_consteval_expr(Parser* parser, Ast_Expr* expr);
static Ast_Term* parse_term(Parser* parser);
static Ast_Var* parse_var(Parser* parser);
static Ast_Access* parse_access(Parser* parser);
static Ast_Var_Access* parse_var_access(Parser* parser);
static Ast_Array_Access* parse_array_access(Parser* parser);
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
