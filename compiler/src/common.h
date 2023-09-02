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
