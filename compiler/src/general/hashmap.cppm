export module hashmap;

import basic;

export template<typename KeyType, typename ValueType, typename HashType, bool (*match_proc)(KeyType& a, KeyType& b)>
struct HashMap 
{
	HashMap() {};
	HashMap(u32 size) { init(size); }
	~HashMap() { destroy(); }

public:
	void init(u32 size) { alloc_table(size); }
	void destroy() { free(array); }

	void zero_reset() {
		memset(array, 0, sizeof(Slot) * table_size);
	}

	void alloc_table(u32 size) {
		table_size = size;
		slots_filled = 0;
		resize_threshold = table_size - table_size / 4;
		array = (Slot*)malloc(sizeof(Slot) * table_size);
		if (array != NULL) memset(array, 0, sizeof(Slot) * table_size);
	}

	void add(KeyType key, ValueType value, HashType hash) {
		u32 slot = hash % table_size;

		while (array[slot].hash != 0 && !match_proc(key, array[slot].key)) {
			slot = (slot + 1) % table_size;
		}

		if (array[slot].hash == 0) {
			array[slot] = Slot{ key, value, hash };
			slots_filled += 1;
			if (slots_filled == resize_threshold) grow();
		}
	}

	bool contains(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return true;
			slot = (slot + 1) % table_size;
		}
		return false;
	}

	option<ValueType> find(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return array[slot].value;
			slot = (slot + 1) % table_size;
		}
		return {};
	}

private:
	void grow() {
		u32 table_size_old = table_size;
		Slot* array_old = array;
		alloc_table(table_size * 2);

		for (u32 i = 0; i < table_size_old; i++) {
			Slot slot = array_old[i];
			if (slot.hash != 0) add(slot.key, slot.value, slot.hash);
		}
		free(array_old);
	}

	struct Slot
	{
		KeyType key;
		ValueType value;
		HashType hash;
	};

	Slot* array = NULL;
	u32 table_size = 0;
	u32 slots_filled = 0;
	u32 resize_threshold = 0;
};

export template<typename KeyType, typename HashType, bool (*match_proc)(KeyType& a, KeyType& b)>
struct HashSet 
{
	HashSet() {};
	HashSet(u32 size) { init(size); }
	~HashSet() { destroy(); }

public:
	void init(u32 size) { alloc_table(size); }
	void destroy() { free(array); }

	void zero_reset() {
		memset(array, 0, sizeof(Slot) * table_size);
	}
	
	void alloc_table(u32 size) {
		table_size = size;
		slots_filled = 0;
		resize_threshold = table_size - table_size / 4;
		array = (Slot*)malloc(sizeof(Slot) * table_size);
		if (array != NULL) memset(array, 0, sizeof(Slot) * table_size);
	}

	void add(KeyType key, HashType hash) {
		u32 slot = hash % table_size;

		while (array[slot].hash != 0 && !match_proc(key, array[slot].key)) {
			slot = (slot + 1) % table_size;
		}

		if (array[slot].hash == 0) {
			array[slot] = Slot{ key, hash };
			slots_filled += 1;
			if (slots_filled == resize_threshold) grow();
		}
	}

	bool contains(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return true;
			slot = (slot + 1) % table_size;
		}
		return false;
	}

	option<KeyType> find_key(KeyType key, HashType hash) {
		u32 slot = hash % table_size;
		while (array[slot].hash != 0) {
			if (match_proc(key, array[slot].key))
				return array[slot].key;
			slot = (slot + 1) % table_size;
		}
		return {};
	}

private:
	void grow() {
		u32 table_size_old = table_size;
		Slot* array_old = array;
		alloc_table(table_size * 2);

		for (u32 i = 0; i < table_size_old; i++) {
			Slot slot = array_old[i];
			if (slot.hash != 0) add(slot.key, slot.hash);
		}
		free(array_old);
	}

	struct Slot 
	{
		KeyType key;
		HashType hash;
	};

	Slot* array = NULL;
	u32 table_size = 0;
	u32 slots_filled = 0;
	u32 resize_threshold = 0;
};
