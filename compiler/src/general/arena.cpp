#include "arena.h"

#include <stdlib.h>
#include <string.h>

void arena_init(Arena* arena, u64 block_size)
{
	arena->offset = 0;
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

void arena_alloc_block(Arena* arena)
{
	arena->data = (u8*)malloc(arena->block_size);
	arena->offset = 0;
	memset(arena->data, 0, arena->block_size);

	Arena_Block* block = arena_alloc<Arena_Block>(arena);
	block->data = arena->data;
	block->prev = arena->curr;
	arena->curr = block;
}
