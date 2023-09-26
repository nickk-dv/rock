#ifndef COMMON_H
#define COMMON_H

#include <unordered_map>
#include <optional>
#include <vector>
#include <chrono>
#include <stdlib.h>
#include <stdio.h>

typedef unsigned char u8;
typedef unsigned int u32;
typedef unsigned long long u64;
struct String;
struct StringView;
struct Timer;
struct StringStorage;
struct Arena;

u32 hash_fnv1a_32(const StringView& str);
u64 hash_fnv1a_64(const StringView& str);
bool os_file_read_all(const char* file_path, String* str);
bool match_string_view(StringView a, StringView b);
constexpr u64 hash_str_ascii_9(const StringView& str);
constexpr u64 hash_ascii_9(const char* str);

struct Atom //@Not used
{
	u32 hash;
	char* name;
};

//bool atom_match(Atom* a, Atom* b) //@Not used
//{
//	if (a == b) return true;
//	if (a->hash != b->hash) return false;
//	return strcmp(a->name, b->name) == 0;
//}

struct String
{
	~String() { free(data); }

	u8* data;
	size_t count;
};

struct StringView
{
	StringView(const String& str)
	{
		data = str.data;
		count = str.count;
	}

	bool operator == (const StringView& other) const
	{
		if (count != other.count) return false;
		for (u64 i = 0; i < count; i++)
			if (data[i] != other.data[i]) return false;
		return true;
	}

	u8* data;
	size_t count;
};

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

struct Timer
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
		const float ns_to_ms = 1000000.0f;
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

struct Arena
{
	Arena() {};
	Arena(size_t block_size) { init(block_size); };
	~Arena() { destroy(); }

public:
	void init(size_t block_size) { m_block_size = block_size; alloc_block(); }
	void destroy() { for (u8* block : m_data_blocks) free(block); }

	template<typename T>
	T* alloc() {
		if (m_offset + sizeof(T) > m_block_size) alloc_block();
		T* ptr = (T*)(m_data + m_offset);
		m_offset += sizeof(T);
		return ptr;
	}

private:
	void alloc_block() {
		m_offset = 0;
		m_data = (u8*)malloc(m_block_size);
		if (m_data != NULL)
			memset(m_data, 0, m_block_size);
		m_data_blocks.emplace_back(m_data);
	}

	u8* m_data = NULL;
	size_t m_offset = 0;
	size_t m_block_size = 0;
	std::vector<u8*> m_data_blocks;
};

template<typename KeyType, typename ValueType, typename HashType, bool (*match_proc)(KeyType a, KeyType b)>
struct HashMap 
{
	HashMap() {};
	HashMap(u32 size) { init(size); }
	~HashMap() { destroy(); }

public:
	void init(u32 size) { alloc_table(size); }
	void destroy() { free(array); }

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

	std::optional<ValueType> find(KeyType key, HashType hash) {
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

template<typename KeyType, typename HashType, bool (*match_proc)(KeyType a, KeyType b)>
struct HashSet 
{
	HashSet() {};
	HashSet(u32 size) { init(size); }
	~HashSet() { destroy(); }

public:
	void init(u32 size) { alloc_table(size); }
	void destroy() { free(array); }

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
