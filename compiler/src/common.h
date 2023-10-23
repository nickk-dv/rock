#ifndef COMMON_H
#define COMMON_H

//@Hack to not crash in debug on vector checks at _ITERATOR_DEBUG_LEVEL 2
#define _ITERATOR_DEBUG_LEVEL 0
#include <unordered_map>
#include <optional>
#include <vector>
#include <chrono>
#include <string>
#include <stdlib.h>
#include <stdio.h>

typedef signed char i8;
typedef unsigned char u8;
typedef short i16;
typedef unsigned short u16;
typedef int i32;
typedef unsigned int u32;
typedef long long i64;
typedef unsigned long long u64;
typedef float f32;
typedef double f64;

template <class T>
using option = std::optional<T>;

struct StringView;
struct Timer;
struct StringStorage;
struct Arena;
struct Arena_Block;

u32 hash_fnv1a_32(const StringView& str);
u64 hash_fnv1a_64(const StringView& str);
bool match_string_view(StringView& a, StringView& b);
constexpr u64 hash_str_ascii_9(const StringView& str);
constexpr u64 hash_ascii_9(const char* str);

struct Arena
{
	u8* data;
	u64 offset;
	u64 block_size;
	Arena_Block* curr;
};

struct Arena_Block
{
	void* data;
	struct Arena_Block* prev;
};

void arena_init(Arena* arena, u64 block_size);
void arena_deinit(Arena* arena);
void arena_alloc_block(Arena* arena);

template<typename T>
T* arena_alloc(Arena* arena)
{
	if (arena->offset + sizeof(T) > arena->block_size)
		arena_alloc_block(arena);
	T* ptr = (T*)(arena->data + arena->offset);
	arena->offset += sizeof(T);
	return ptr;
}

template<typename T>
T* arena_alloc_buffer(Arena* arena, u64 size)
{
	if (arena->offset + size * sizeof(T) > arena->block_size)
		arena_alloc_block(arena);
	T* ptr = (T*)(arena->data + arena->offset);
	arena->offset += size * sizeof(T);
	return ptr;
}

struct StringView
{
	u8* data;
	u64 count;
};

StringView string_view_from_string(const std::string& string);

constexpr u64 hash_str_ascii_9(const StringView& str)
{
	u64 hash = 0;
	for (u32 i = 0; i < str.count; i++)
		hash = (hash << 7) | (u64)str.data[i];
	return hash;
}

constexpr u64 hash_ascii_9(const char* str)
{
	u64 hash = 0;
	for (u32 i = 0; i < 9 && str[i] != '\0'; i++)
		hash = (hash << 7) | (u64)((u8)str[i]);
	return hash;
}

struct Timer //@Todo change to scope based instrumentation
{
	typedef std::chrono::high_resolution_clock Clock;
	typedef std::chrono::steady_clock::time_point TimePoint;
	typedef std::chrono::nanoseconds Ns;

	void start()
	{
		t0 = Clock::now();
	}

	void end(const char* message)
	{
		TimePoint t1 = Clock::now();
		Ns ns = std::chrono::duration_cast<Ns>(t1 - t0);
		const f32 ns_to_ms = 1000000.0f;
		printf("%s ms: %f\n", message, ns.count() / ns_to_ms);
	}

	TimePoint t0;
};

struct StringStorage
{
	~StringStorage() { destroy(); }

	void init() { buffer = (char*)malloc(1024 * 16); }
	void destroy() { free(buffer); }

	void start_str() {
		str_start = cursor;
	}

	char* end_str() {
		put_char('\0');
		return buffer + str_start;
	}

	void put_char(char c) {
		buffer[cursor] = c;
		cursor += 1;
	}

private:
	char* buffer;
	u64 cursor = 0;
	u64 str_start = 0;
};

template<typename KeyType, typename ValueType, typename HashType, bool (*match_proc)(KeyType& a, KeyType& b)>
struct HashMap 
{
	HashMap() {};
	HashMap(u32 size) { init(size); }
	~HashMap() { destroy(); }

public:
	void init(u32 size) { alloc_table(size); }
	void destroy() { free(array); }

	void zero_reset() {
		memset(array, 0, sizeof(Slot) * table_size);
	}

	void alloc_table(u32 size) {
		table_size = size;
		slots_filled = 0;
		resize_threshold = table_size - table_size / 4;
		array = (Slot*)malloc(sizeof(Slot) * table_size);
		if (array != NULL) memset(array, 0, sizeof(Slot) * table_size);
	}

	void add(KeyType key, ValueType value, HashType hash) {
		u32 slot = hash % table_size;

		while (array[slot].hash != 0 && !match_proc(key, array[slot].key)) {
			slot = (slot + 1) % table_size;
		}

		if (array[slot].hash == 0) {
			array[slot] = Slot{ key, value, hash };
			slots_filled += 1;
			if (slots_filled == resize_threshold) grow();
		}
	}

	bool contains(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return true;
			slot = (slot + 1) % table_size;
		}
		return false;
	}

	option<ValueType> find(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return array[slot].value;
			slot = (slot + 1) % table_size;
		}
		return {};
	}

private:
	void grow() {
		u32 table_size_old = table_size;
		Slot* array_old = array;
		alloc_table(table_size * 2);

		for (u32 i = 0; i < table_size_old; i++) {
			Slot slot = array_old[i];
			if (slot.hash != 0) add(slot.key, slot.value, slot.hash);
		}
		free(array_old);
	}

	struct Slot
	{
		KeyType key;
		ValueType value;
		HashType hash;
	};

	Slot* array = NULL;
	u32 table_size = 0;
	u32 slots_filled = 0;
	u32 resize_threshold = 0;
};

template<typename KeyType, typename HashType, bool (*match_proc)(KeyType& a, KeyType& b)>
struct HashSet 
{
	HashSet() {};
	HashSet(u32 size) { init(size); }
	~HashSet() { destroy(); }

public:
	void init(u32 size) { alloc_table(size); }
	void destroy() { free(array); }

	void zero_reset() {
		memset(array, 0, sizeof(Slot) * table_size);
	}
	
	void alloc_table(u32 size) {
		table_size = size;
		slots_filled = 0;
		resize_threshold = table_size - table_size / 4;
		array = (Slot*)malloc(sizeof(Slot) * table_size);
		if (array != NULL) memset(array, 0, sizeof(Slot) * table_size);
	}

	void add(KeyType key, HashType hash) {
		u32 slot = hash % table_size;

		while (array[slot].hash != 0 && !match_proc(key, array[slot].key)) {
			slot = (slot + 1) % table_size;
		}

		if (array[slot].hash == 0) {
			array[slot] = Slot{ key, hash };
			slots_filled += 1;
			if (slots_filled == resize_threshold) grow();
		}
	}

	bool contains(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return true;
			slot = (slot + 1) % table_size;
		}
		return false;
	}

	option<KeyType> find_key(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return array[slot].key;
			slot = (slot + 1) % table_size;
		}
		return {};
	}

private:
	void grow() {
		u32 table_size_old = table_size;
		Slot* array_old = array;
		alloc_table(table_size * 2);

		for (u32 i = 0; i < table_size_old; i++) {
			Slot slot = array_old[i];
			if (slot.hash != 0) add(slot.key, slot.hash);
		}
		free(array_old);
	}

	struct Slot 
	{
		KeyType key;
		HashType hash;
	};

	Slot* array = NULL;
	u32 table_size = 0;
	u32 slots_filled = 0;
	u32 resize_threshold = 0;
};

#endif
