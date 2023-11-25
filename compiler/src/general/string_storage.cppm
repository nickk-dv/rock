export module string_storage;

import basic;

export struct StringStorage //@Todo rework / move to parser
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
