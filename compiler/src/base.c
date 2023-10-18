#include "base.h"

#include <stdlib.h>
#include <string.h>
/*
void arena_init(Arena* arena, u64 block_size)
{
	arena->block_size = block_size;
	arena->curr = NULL;
	arena_alloc_block(arena);
}

void arena_deinit(Arena* arena)
{
	Arena_Block* block = arena->curr;
	while (block != NULL)
	{
		Arena_Block* prev = block->prev;
		free(block->data);
		block = prev;
	}
}

void* arena_void_alloc(Arena* arena, u64 size)
{
	if (arena->offset + size > arena->block_size)
		arena_alloc_block(arena);
	void* ptr = (u8*)arena->data + arena->offset;
	arena->offset += size;
	return ptr;
}

void arena_alloc_block(Arena* arena)
{
	arena->data = malloc(arena->block_size);
	arena->offset = 0;
	memset(arena->data, 0, arena->block_size);

	Arena_Block* block = arena_alloc(arena, Arena_Block);
	block->data = arena->data;
	block->prev = arena->curr;
	arena->curr = block;
}
*/