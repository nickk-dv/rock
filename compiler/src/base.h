#ifndef BASE_H
#define BASE_H

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

#define Option(Type) Option_##Type
#define OptionDecl(Type) typedef struct Option_##Type { Type value; bool has_value; } Option_##Type;
#define Some(value) { value, true }
#define None() { {}, false }
#define is_some(option) option.has_value
#define is_none(option) !option.has_value

#define arena_alloc(arena, Type) (Type*)arena_void_alloc(arena, sizeof(Type))
#define array_add(array, value, Type) array_add_##Type(&array, value)

typedef struct Arena_Block
{
	void* data;
	struct Arena_Block* prev;
} Arena_Block;

typedef struct Arena
{
	void* data;
	u64 offset;
	u64 block_size;
	Arena_Block* curr;
} Arena;

void arena_init(Arena* arena, u64 block_size);
void arena_deinit(Arena* arena);
static void* arena_void_alloc(Arena* arena, u64 size);
static void arena_alloc_block(Arena* arena);

#define Array(Type) Array_##Type
#define ArrayDecl(Type) \
typedef struct Array_##Type { Type* data; u64 size; u64 capacity; } Array_##Type; \
static void array_add_##Type(Array_##Type* array, Type value);

#define ArrayImpl(Type) \
void array_add_##Type(Array_##Type* array, Type value) \
{\
	if (array->size + 1 > array->capacity)\
	{\
		if (array->capacity != 0)\
		{\
			Type* new_data = malloc(sizeof(Type) * array->capacity * 2);\
			memcpy(new_data, array->data, sizeof(Type) * array->capacity);\
			free(array->data);\
			array->data = new_data;\
			array->capacity *= 2;\
		}\
		else\
		{\
			const u32 INIT_SIZE = 8;\
			Type* new_data = malloc(sizeof(Type) * INIT_SIZE);\
			array->data = new_data;\
			array->capacity = INIT_SIZE;\
		}\
	}\
	array->data[array->size] = value;\
	array->size += 1;\
}

#define HashMap(K, V, H) HashMap_##K##V##H
#define HashMapDecl(K, V, H) \
typedef struct Slot_##K##V##H \
{ \
	K key; \
	V value; \
	H hash; \
}  Slot_##K##V##H; \
typedef struct HashMap_##K##V##H \
{ \
	Slot_##K##V##H* array; \
	u32 table_size; \
	u32 slots_filled; \
	u32 resize_threshold; \
} HashMap_##K##V##H; \

#define HashSet(K, H) HashSet_##K##H
#define HashSetDecl(K, H) \
typedef struct Slot_##K##H \
{ \
	K key; \
	H hash; \
}  Slot_##K##H; \
typedef struct HashSet_##K##H \
{ \
	Slot_##K##H* array; \
	u32 table_size; \
	u32 slots_filled; \
	u32 resize_threshold; \
} HashSet_##K##H;

#endif
