#include "general.h"

u32 hash_fnv1a_32(const StringView& str)
{
#define FNV_PRIME_32 0x01000193UL
#define FNV_OFFSET_32 0x811c9dc5UL

	u32 hash = FNV_OFFSET_32;
	for (u32 i = 0; i < str.count; i++)
	{
		hash ^= str.data[i];
		hash *= FNV_PRIME_32;
	}
	return hash;
}

u64 hash_fnv1a_64(const StringView& str)
{
#define FNV_PRIME_64 0x00000100000001B3UL
#define FNV_OFFSET_64 0xcbf29ce484222325UL

	u64 hash = FNV_OFFSET_64;
	for (u32 i = 0; i < str.count; i++)
	{
		hash ^= str.data[i];
		hash *= FNV_PRIME_64;
	}
	return hash;
}

bool match_string_view(StringView& a, StringView& b)
{
	if (a.count != b.count) return false;
	for (u64 i = 0; i < a.count; i++)
		if (a.data[i] != b.data[i]) return false;
	return true;
}

StringView string_view_from_string(const std::string& string)
{
	return StringView{ (u8*)string.data(), string.size() };
}
