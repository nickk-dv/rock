#pragma once

typedef unsigned char u8;
typedef unsigned int u32;
typedef unsigned long long u64;

struct String
{
	~String();

	u8* data;
	size_t count;
};

struct StringView
{
	StringView(const String& str);

	u8* data;
	size_t count;
};

bool os_file_read_all(const char* file_path, String* str);

u64 string_hash_ascii_count_9(const StringView& str);

constexpr u64 string_hash_ascii_count_9(const char* str)
{
	u64 hash = 0;

	for (u32 i = 0; i < 9 && str[i] != '\0'; i++)
		hash = (hash << 7) | ((u64)((u8)str[i]) & 0x7F);

	return hash;
}
