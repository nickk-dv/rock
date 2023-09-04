#include "common.h"

#include <cstdlib>
#include <cstdio>

String::~String()
{
	free(data);
}

StringView::StringView(const String& str)
{
	data = str.data;
	count = str.count;
}

bool os_file_read_all(const char* file_path, String* str)
{
	FILE* file = fopen(file_path, "rb");
	if (!file) return false;

	fseek(file, 0, SEEK_END);
	long file_size = ftell(file);
	fseek(file, 0, SEEK_SET);

	if (file_size <= 0)
	{
		fclose(file);
		return false;
	}

	file_size = (size_t)file_size;
	void* buffer =  malloc(file_size);
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

u64 string_hash_ascii_count_9(const StringView& str)
{
	u64 hash = 0;

	for (u32 i = 0; i < str.count; i++)
		hash = (hash << 7) | ((u64)str.data[i] & 0x7F);

	return hash;
}
