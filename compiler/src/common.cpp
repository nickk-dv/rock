#include "common.h"

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

u32 hash_fnv1a_32(const StringView& str)
{
	#define FNV_PRIME_32 0x01000193UL
	#define FNV_OFFSET_32 0x811c9dc5UL

	u32 hash = FNV_OFFSET_32;
	for (u32 i = 0; i < str.count; i++)
	{
		hash ^= str.data[i];
		hash *= FNV_PRIME_32;
	}
	return hash;
}

u64 hash_fnv1a_64(const StringView& str)
{
	#define FNV_PRIME_64 0x00000100000001B3UL
	#define FNV_OFFSET_64 0xcbf29ce484222325UL

	u64 hash = FNV_OFFSET_64;
	for (u32 i = 0; i < str.count; i++)
	{
		hash ^= str.data[i];
		hash *= FNV_PRIME_64;
	}
	return hash;
}

bool match_string_view(StringView& a, StringView& b)
{
	if (a.count != b.count) return false;
	for (u64 i = 0; i < a.count; i++)
	if (a.data[i] != b.data[i]) return false;
	return true;
}
