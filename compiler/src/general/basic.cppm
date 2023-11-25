export module basic;

import <optional>;
export template <class T> using option = std::optional<T>;
export import <vector>;
export import <string>;

export typedef signed char        i8;
export typedef signed short       i16;
export typedef signed int         i32;
export typedef signed long long   i64;
export typedef unsigned char      u8;
export typedef unsigned short     u16;
export typedef unsigned int       u32;
export typedef unsigned long long u64;
export typedef float              f32;
export typedef double             f64;

struct StringView;
export bool match_string_view(StringView& a, StringView b);

export constexpr u64 hash_cstr_unique_64(const char* cstr)
{
	u64 hash = 0;
	for (u32 i = 0; i < 9 && cstr[i] != '\0'; i++)
	{
		hash = (hash << 7) | (u64)cstr[i];
	}
	return hash;
}

export struct Span
{
	u32 start;
	u32 end;
};

export struct StringView
{
	u8* data;
	u64 count;

	u32 hash_fnv1a_32();
	u64 hash_unique_64();
};

export StringView string_view_from_string(const std::string& string)
{
	return StringView{ (u8*)string.data(), string.size() };
}

module : private;

u32 StringView::hash_fnv1a_32() 
{
	constexpr u32 fnv_prime_32 = 0x01000193UL;
	constexpr u32 fnv_offset_32 = 0x811c9dc5UL;
	
	u32 hash = fnv_offset_32;
	for (u32 i = 0; i < this->count; i++) 
	{
		hash ^= this->data[i];
		hash *= fnv_prime_32;
	}
	return hash;
}

u64 StringView::hash_unique_64()
{
	u64 hash = 0;
	for (u32 i = 0; i < this->count; i++)
	{
		hash = (hash << 7) | (u64)this->data[i];
	}
	return hash;
}

bool match_string_view(StringView& a, StringView b)
{
	if (a.count != b.count) return false;
	for (u64 i = 0; i < a.count; i++)
	{
		if (a.data[i] != b.data[i]) return false;
	}
	return true;
}
