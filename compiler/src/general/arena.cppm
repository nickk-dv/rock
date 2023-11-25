module;
#include <stdlib.h>
#include <string.h>
export module arena;

import basic;

struct Arena_Block
{
	void* data;
	Arena_Block* prev;
};

export struct Arena
{
private:
	u8* data;
	u64 offset;
	u64 block_size;
	Arena_Block* curr;

public:
	void init(u64 block_size);
	void deinit();
	template<typename T> T* alloc();
	template<typename T> T* alloc_buffer(u64 size);

private:
	void alloc_block();
};

void Arena::init(u64 block_size)
{
	this->offset = 0;
	this->block_size = block_size;
	this->curr = NULL;
	alloc_block();
}

void Arena::deinit()
{
	Arena_Block* block = this->curr;
	while (block != NULL)
	{
		Arena_Block* prev = block->prev;
		free(block->data);
		block = prev;
	}
}

template<typename T>
T* Arena::alloc()
{
	return alloc_buffer<T>(arena, 1);
}

template<typename T>
T* Arena::alloc_buffer(u64 size)
{
	if (this->offset + size * sizeof(T) > this->block_size) alloc_block();
	T* ptr = (T*)(this->data + this->offset);
	this->offset += size * sizeof(T);
	return ptr;
}

void Arena::alloc_block()
{
	this->data = (u8*)malloc(this->block_size);
	this->offset = 0;
	memset(this->data, 0, this->block_size);

	Arena_Block* block = alloc<Arena_Block>();
	block->data = this->data;
	block->prev = this->curr;
	this->curr = block;
}
