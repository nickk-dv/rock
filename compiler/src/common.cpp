#include <stdio.h>
#include <stdlib.h>
#include <chrono>

typedef unsigned char u8;
typedef unsigned int u32;
typedef unsigned long long u64;
struct String;
struct StringView;
struct Timer;
struct Arena;
struct StringViewHasher;

constexpr u64 string_hash_ascii_9(const StringView& str);
constexpr u64 hash_ascii_9(const char* str);
bool os_file_read_all(const char* file_path, String* str);
u64 hash_fnv1a(const StringView& str);

struct String
{
	~String()
	{
		free(data);
	}

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

	u8* data = NULL;
	size_t count = 0;
};

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

struct Arena
{
	Arena() {};
	~Arena() { drop(); }

	template<typename T>
	T* alloc()
	{
		if (m_offset + sizeof(T) > m_block_size) alloc_block();
		T* ptr = (T*)(m_data + m_offset);
		m_offset += sizeof(T);
		return ptr;
	}
	
	void init(size_t block_size)
	{
		m_block_size = block_size;
		alloc_block();
	}

	void drop()
	{
		for (u8* block : m_data_blocks) free(block);
	}

	void alloc_block()
	{
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

struct StringViewHasher
{
	size_t operator()(const StringView& str) const
	{
		return hash_fnv1a(str);
	}
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

bool os_file_read_all(const char* file_path, String* str)
{
	FILE* file;
	fopen_s(&file, file_path, "rb");
	if (!file) return false;

	fseek(file, 0, SEEK_END);
	size_t file_size = (size_t)ftell(file);
	fseek(file, 0, SEEK_SET);

	if (file_size == 0)
	{
		fclose(file);
		return false;
	}

	void* buffer =  malloc(file_size);

	if (buffer == NULL)
	{
		fclose(file);
		return false;
	}
	
	size_t read_size = fread(buffer, 1, file_size, file);
	fclose(file);

	if (read_size != file_size)
	{
		free(buffer);
		return false;
	}
	
	str->data = (u8*)buffer;
	str->count = file_size;
	
	return true;
}

u64 hash_fnv1a(const StringView& str)
{
	#define FNV_PRIME 0x00000100000001B3UL
	#define FNV_OFFSET 0xcbf29ce484222325UL

	u64 hash = FNV_OFFSET;
	for (u32 i = 0; i < str.count; i++)
	{
		hash ^= str.data[i];
		hash *= FNV_PRIME;
	}
	return hash;
}

//lookup Type from StringView
