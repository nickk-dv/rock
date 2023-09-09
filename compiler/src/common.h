#pragma once

typedef unsigned char u8;
typedef unsigned int u32;
typedef unsigned long long u64;

struct String
{
	~String();

	u8* data;
	u64 count;
};

struct StringView
{
	StringView(const String& str);

	u8* data;
	u64 count;
};

struct Timer
{
	Timer();

	float Ms();
	float Sec();
	
	long start_time;
};

//@Incomplete no overflow protection
class ArenaAllocator
{
public:
	ArenaAllocator(size_t size);
	~ArenaAllocator();

	inline ArenaAllocator(const ArenaAllocator& other) = delete;
	inline ArenaAllocator operator=(const ArenaAllocator& other) = delete;

	template <typename T>
	T* alloc()
	{
		size_t offset = m_offset;
		m_offset += sizeof(T);
		return (T*)offset;
	}

private:
	size_t m_size;
	size_t m_offset;
	void* m_buffer;
};

constexpr u64 string_hash_ascii_9(const StringView& str)
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

bool os_file_read_all(const char* file_path, String* str);
