#ifndef LLVM_IR_BUILDER_H
#define LLVM_IR_BUILDER_H

#include "llvm-c/Types.h"
#include "ast.h"

struct IR_Context;
struct IR_Block_Info;
struct IR_Loop_Info;
struct IR_Var_Info;
struct IR_Var_Access_Info;
struct IR_Block_Stack;
enum class Terminator2;
enum class BlockFlags;
enum class ProcCallFlags;

LLVMModuleRef build_module(Ast_Program* program);

static IR_Context build_context_init(Ast_Program* program);
static void build_context_deinit(IR_Context* builder);
static char* ident_to_cstr(Ast_Ident& ident);
static LLVMBasicBlockRef add_bb(IR_Block_Stack* bc, const char* name);
static void set_bb(IR_Context* context, LLVMBasicBlockRef block);
static LLVMTypeRef basic_type_to_llvm_type(BasicType basic_type);
static LLVMTypeRef type_to_llvm_type(IR_Context* context, Ast_Type type);
static void block_stack_reset(IR_Block_Stack* bc, LLVMValueRef proc_value);
static void block_stack_add(IR_Block_Stack* bc);
static void block_stack_pop_back(IR_Block_Stack* bc);
static void block_stack_add_defer(IR_Block_Stack* bc, Ast_Defer* defer);
static void block_stack_add_loop(IR_Block_Stack* bc, IR_Loop_Info loop_info);
static void block_stack_add_var(IR_Block_Stack* bc, IR_Var_Info var_info);
static IR_Var_Info block_stack_find_var(IR_Block_Stack* bc, Ast_Ident ident);
static IR_Loop_Info block_stack_get_loop(IR_Block_Stack* bc);
static Terminator2 build_block(IR_Context* context, IR_Block_Stack* bc, Ast_Block* block, BlockFlags flags);
static void build_defer(IR_Context* context, IR_Block_Stack* bc, Terminator2 terminator);
static void build_if(IR_Context* context, IR_Block_Stack* bc, Ast_If* _if, LLVMBasicBlockRef cont_block);
static void build_for(IR_Context* context, IR_Block_Stack* bc, Ast_For* _for);
static void build_var_decl(IR_Context* context, IR_Block_Stack* bc, Ast_Var_Decl* var_decl);
static void build_var_assign(IR_Context* context, IR_Block_Stack* bc, Ast_Var_Assign* var_assign);
LLVMValueRef build_proc_call(IR_Context* context, IR_Block_Stack* bc, Ast_Proc_Call* proc_call, ProcCallFlags flags);
LLVMValueRef build_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Expr* expr);
LLVMValueRef build_term(IR_Context* context, IR_Block_Stack* bc, Ast_Term* term);
IR_Var_Access_Info build_var(IR_Context* context, IR_Block_Stack* bc, Ast_Var* var);
LLVMValueRef build_unary_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Unary_Expr* unary_expr);
LLVMValueRef build_binary_expr(IR_Context* context, IR_Block_Stack* bc, Ast_Binary_Expr* binary_expr);
void build_implicit_cast(IR_Context* context, LLVMValueRef* value, LLVMTypeRef type, LLVMTypeRef target_type);
void build_implicit_binary_cast(IR_Context* context, LLVMValueRef* value_lhs, LLVMValueRef* value_rhs, LLVMTypeRef type_lhs, LLVMTypeRef type_rhs);

struct IR_Context
{
	Ast_Program* program;
	LLVMBuilderRef builder;
	LLVMModuleRef module;
};

struct IR_Block_Info
{
	u32 defer_count;
	u32 loop_count;
	u32 var_count;
};

struct IR_Loop_Info
{
	LLVMBasicBlockRef break_block;
	LLVMBasicBlockRef continue_block;
	std::optional<Ast_Var_Assign*> var_assign;
};

struct IR_Var_Info
{
	StringView str;
	LLVMValueRef var_ptr;
	Ast_Type ast_type;
};

struct IR_Var_Access_Info
{
	LLVMValueRef var_ptr;
	LLVMTypeRef var_type;
};

struct IR_Block_Stack
{
	LLVMValueRef proc_value;
	std::vector<IR_Block_Info> blocks;
	std::vector<Ast_Defer*> defer_stack;
	std::vector<IR_Loop_Info> loop_stack;
	std::vector<IR_Var_Info> var_stack;
};

enum class Terminator2
{
	None,
	Break,
	Return,
	Continue,
};

enum class BlockFlags
{
	None,
	DisableBlockAdd,
};

enum class ProcCallFlags
{
	None,
	AsStatement,
};

#endif
