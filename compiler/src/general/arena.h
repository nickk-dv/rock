#ifndef ARENA_H
#define ARENA_H

#include "general.h"

struct Arena_Block
{
	void* data;
	Arena_Block* prev;
};

struct Arena
{
	u8* data;
	u64 offset;
	u64 block_size;
	Arena_Block* curr;
};

void arena_init(Arena* arena, u64 block_size);
void arena_deinit(Arena* arena);
void arena_alloc_block(Arena* arena);

template<typename T>
T* arena_alloc_buffer(Arena* arena, u64 size)
{
	if (arena->offset + size * sizeof(T) > arena->block_size)
		arena_alloc_block(arena);
	T* ptr = (T*)(arena->data + arena->offset);
	arena->offset += size * sizeof(T);
	return ptr;
}

template<typename T>
T* arena_alloc(Arena* arena)
{
	return arena_alloc_buffer<T>(arena, 1);
}

#endif
