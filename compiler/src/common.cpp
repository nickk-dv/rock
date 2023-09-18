#include <stdio.h>
#include <stdlib.h>
#include <chrono>

typedef unsigned char u8;
typedef unsigned int u32;
typedef unsigned long long u64;

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

	u8* data;
	size_t count;
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

class ArenaAllocator //@Incomplete no overflow protection
{
public:
	ArenaAllocator(size_t size)
	{
		m_size = size;
		m_offset = 0;
		m_buffer = malloc(size);
		if (m_buffer != NULL)
		memset(m_buffer, 0, size);
	}

	~ArenaAllocator()
	{
		free(m_buffer);
	}

	inline ArenaAllocator(const ArenaAllocator& other) = delete;
	inline ArenaAllocator operator=(const ArenaAllocator& other) = delete;

	template <typename T>
	T* alloc()
	{
		T* ptr = (T*)((u8*)m_buffer + m_offset);
		m_offset += sizeof(T);
		return ptr;
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
