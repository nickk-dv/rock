#include "common.h"

#include <cstdlib>
#include <cstdio>
#include <time.h>

String::~String()
{
	free(data);
}

StringView::StringView(const String& str)
{
	data = str.data;
	count = str.count;
}

Timer::Timer()
{
	start_time = clock();
}

float Timer::Ms()
{
	return Sec() * 1000.0f;
}

float Timer::Sec()
{
	long time = clock() - start_time;
	return (float)time / CLOCKS_PER_SEC;
}

ArenaAllocator::ArenaAllocator(size_t size)
{
	m_size = size;
	m_offset = 0;
	m_buffer = malloc(size);
}

ArenaAllocator::~ArenaAllocator()
{
	free(m_buffer);
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
