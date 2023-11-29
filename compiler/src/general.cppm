export module general;

export import <string>;
export import <vector>;
import <optional>;
export template <class T>
using option = std::optional<T>;

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

export struct Span
{
	u32 start;
	u32 end;
};

export struct StringView
{
	u8* data;
	u64 count;

	u32 hash_fnv1a_32() const;
	bool operator== (const StringView& other) const;
	bool operator!= (const StringView& other) const;
};

export StringView string_view_from_string(const std::string& string)
{
	return StringView{ (u8*)string.data(), string.size() };
}

u32 StringView::hash_fnv1a_32() const
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

bool StringView::operator== (const StringView& other) const
{
	if (count != other.count) return false;
	for (u64 i = 0; i < count; i++)
	{
		if (data[i] != other.data[i]) return false;
	}
	return true;
}

bool StringView::operator!= (const StringView& other) const
{
	return !(*this == other);
}

export struct Arena
{
private:
	struct Arena_Block
	{
		void* data;
		Arena_Block* prev;
	};

	u8* data;
	u64 offset;
	u64 block_size;
	Arena_Block* curr;

public:
	void init(u64 block_size);
	~Arena();
	template<typename T> T* alloc();
	template<typename T> T* alloc_buffer(u64 size);

private:
	void alloc_block();
};

void Arena::init(u64 block_size)
{
	this->offset = 0;
	this->block_size = block_size;
	this->curr = nullptr;
	alloc_block();
}

Arena::~Arena()
{
	Arena_Block* block = this->curr;
	while (block != nullptr)
	{
		Arena_Block* prev = block->prev;
		free(block->data);
		block = prev;
	}
}

template<typename T>
T* Arena::alloc()
{
	return alloc_buffer<T>(1);
}

template<typename T>
T* Arena::alloc_buffer(u64 size)
{
	if (this->offset + size * sizeof(T) > this->block_size) alloc_block();
	T* ptr = (T*)(this->data + this->offset);
	this->offset += size * sizeof(T);
	return ptr;
}

void Arena::alloc_block()
{
	this->data = (u8*)malloc(this->block_size);
	this->offset = 0;
	memset(this->data, 0, this->block_size);

	Arena_Block* block = alloc<Arena_Block>();
	block->data = this->data;
	block->prev = this->curr;
	this->curr = block;
}

export template<typename KeyType, typename ValueType>
struct HashMap
{
	void init(u32 table_size) { alloc_table(table_size); }
	~HashMap() { free(this->array); }
	
	void zero_reset()
	{
		this->slots_filled = 0;
		this->resize_threshold = this->size - this->size / 4;
		memset(array, 0, sizeof(Slot) * size);
	}

	void add(const KeyType& key, const ValueType& value)
	{
		u32 slot = key.hash % this->size;
		while (this->array[slot].key.hash != 0 && key != this->array[slot].key)
			slot = (slot + 1) % this->size;
		if (this->array[slot].key.hash == 0) { this->array[slot] = Slot{ key, value }; grow_size(); }
	}

	bool contains(const KeyType& key)
	{
		return find_key_slot(key).has_value();
	}

	option<ValueType> find(const KeyType& key)
	{
		option<u32> slot = find_key_slot(key);
		if (!slot) return {};
		return this->array[slot.value()].value;
	}

private:
	option<u32> find_key_slot(const KeyType& key)
	{
		u32 slot = key.hash % this->size;
		while (this->array[slot].key.hash != 0)
		{
			if (key == this->array[slot].key) return slot;
			slot = (slot + 1) % this->size;
		}
		return {};
	}

	void alloc_table(u32 table_size)
	{
		this->size = table_size;
		this->slots_filled = 0;
		this->resize_threshold = this->size - this->size / 4;
		this->array = (Slot*)malloc(sizeof(Slot) * this->size);
		if (this->array != nullptr) memset(this->array, 0, sizeof(Slot) * this->size);
	}

	void grow_size()
	{
		this->slots_filled += 1;
		if (this->slots_filled < this->resize_threshold) return;

		u32 prev_size = this->size;
		Slot* prev_array = this->array;
		alloc_table(this->size * 2);

		for (u32 i = 0; i < prev_size; i++)
		{
			Slot slot = prev_array[i];
			if (slot.key.hash != 0) add(slot.key, slot.value);
		}
		free(prev_array);
	}

	struct Slot
	{
		KeyType key;
		ValueType value;
	};

	Slot* array;
	u32 size;
	u32 slots_filled;
	u32 resize_threshold;
};

export template<typename KeyType>
struct HashSet
{
	void init(u32 table_size) { alloc_table(table_size); }
	~HashSet() { free(this->array); }

	void zero_reset()
	{
		this->slots_filled = 0;
		this->resize_threshold = this->size - this->size / 4;
		memset(array, 0, sizeof(Slot) * size);
	}

	void add(const KeyType& key)
	{
		u32 slot = key.hash % this->size;
		while (this->array[slot].key.hash != 0 && key != this->array[slot].key)
			slot = (slot + 1) % this->size;
		if (this->array[slot].key.hash == 0) { this->array[slot] = Slot{ key }; grow_size(); }
	}

	bool contains(const KeyType& key)
	{
		return find_key_slot(key).has_value();
	}

	option<KeyType> find(const KeyType& key)
	{
		option<u32> slot = find_key_slot(key);
		if (!slot) return {};
		return this->array[slot.value()].key;
	}

private:
	option<u32> find_key_slot(const KeyType& key)
	{
		u32 slot = key.hash % this->size;
		while (this->array[slot].key.hash != 0)
		{
			if (key == this->array[slot].key) return slot;
			slot = (slot + 1) % this->size;
		}
		return {};
	}

	void alloc_table(u32 table_size)
	{
		this->size = table_size;
		this->slots_filled = 0;
		this->resize_threshold = this->size - this->size / 4;
		this->array = (Slot*)malloc(sizeof(Slot) * this->size);
		if (this->array != nullptr) memset(this->array, 0, sizeof(Slot) * this->size);
	}

	void grow_size()
	{
		this->slots_filled += 1;
		if (this->slots_filled < this->resize_threshold) return;

		u32 prev_size = this->size;
		Slot* prev_array = this->array;
		alloc_table(this->size * 2);

		for (u32 i = 0; i < prev_size; i++)
		{
			Slot slot = prev_array[i];
			if (slot.key.hash != 0) add(slot.key);
		}
		free(prev_array);
	}

	struct Slot
	{
		KeyType key;
	};

	Slot* array;
	u32 size;
	u32 slots_filled;
	u32 resize_threshold;
};

export template<typename T>
struct Tree_Node
{
	T value;
	Tree_Node* parent;
	Tree_Node* first_child;
	Tree_Node* next_sibling;
};

export template<typename T>
struct Tree
{
	Arena arena;
	Tree_Node<T>* root;

	Tree(u64 block_size, T root_value)
	{
		arena.init(block_size);
		root = arena.alloc<Tree_Node<T>>();
		root->value = root_value;
		root->parent = nullptr;
		root->first_child = nullptr;
		root->next_sibling = nullptr;
	}

	~Tree() { arena.deinit(); }
};

export template<typename T>
Tree_Node<T>* tree_node_add_child(Arena* arena, Tree_Node<T>* parent, T value)
{
	Tree_Node<T>* node = arena_alloc<Tree_Node<T>>(arena);
	node->value = value;
	node->parent = parent;
	node->first_child = nullptr;
	node->next_sibling = nullptr;

	if (parent->first_child == nullptr)
	{
		parent->first_child = node;
		return node;
	}

	Tree_Node<T>* sibling = parent->first_child;
	while (sibling->next_sibling != nullptr) sibling = sibling->next_sibling;
	sibling->next_sibling = node;
	return node;
}

export template<typename T, typename Match_Proc>
option<Tree_Node<T>*> tree_node_find_cycle(Tree_Node<T>* node, T match_value, Match_Proc match)
{
	while (node->parent != nullptr)
	{
		node = node->parent;
		if (match(node->value, match_value)) return node;
	}
	return {};
}

export template<typename T, typename Apply_Proc, typename Context>
void tree_node_apply_proc_in_reverse_up_to_node(Tree_Node<T>* node, Tree_Node<T>* up_to_node, Context* context, Apply_Proc apply)
{
	std::vector<Tree_Node<T>*> nodes;

	while (node != nullptr)
	{
		nodes.emplace_back(node);
		if (node == up_to_node) break;
		node = node->parent;
	}

	for (auto it = nodes.rbegin(); it != nodes.rend(); it++)
	{
		apply(context, *it);
	}
}

export template<typename T, typename Apply_Proc, typename Context>
void tree_node_apply_proc_up_to_root(Tree_Node<T>* node, Context* context, Apply_Proc apply)
{
	while (node != nullptr)
	{
		apply(context, node);
		node = node->parent;
	}
}

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
