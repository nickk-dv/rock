#include "llvm_ir_types.h"

void Var_Block_Scope::add_block()
{
	block_stack.emplace_back(Var_Block_Info{ 0 });
}

void Var_Block_Scope::pop_block()
{
	Var_Block_Info info = block_stack[block_stack.size() - 1];
	for (u32 i = 0; i < info.var_count; i++) var_stack.pop_back();
	for (u32 i = 0; i < info.defer_count; i++) defer_stack.pop_back();
	block_stack.pop_back();
}

void Var_Block_Scope::add_var(const Var_Meta& var)
{
	block_stack[block_stack.size() - 1].var_count += 1;
	var_stack.emplace_back(var);
}

Var_Meta Var_Block_Scope::find_var(StringView str)
{
	for (const Var_Meta& var : var_stack)
		if (var.str == str) return var;
	// @Hack exiting here, this shouldnt happen in checked code
	printf("get_var_access_meta: failed to find var in scope");
	exit(EXIT_FAILURE);
	return Var_Meta{};
}

void Var_Block_Scope::add_defer(Ast_Defer* defer)
{
	block_stack[block_stack.size() - 1].defer_count += 1;
	defer_stack.emplace_back(defer);
}

u32 Var_Block_Scope::get_curr_defer_count()
{
	return block_stack[block_stack.size() - 1].defer_count;
}
