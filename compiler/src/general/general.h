#ifndef GENERAL_H
#define GENERAL_H

#define _ITERATOR_DEBUG_LEVEL 0
#include <optional>
#include <vector>
#include <string>

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

struct StringView
{
	u8* data;
	u64 count;
};

u32 hash_fnv1a_32(const StringView& str);
u64 hash_fnv1a_64(const StringView& str);
bool match_string_view(StringView& a, StringView& b);
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

struct StringStorage //@Todo rework / move to parser
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

#endif
