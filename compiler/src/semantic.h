#pragma once

#include "common.h"

enum class PrimitiveType
{
	i8,
	u8,
	i16,
	u16,
	i32,
	u32,
	i64,
	u64,
	f32,
	f64,
	Bool,
	NotPrimitive,
};

PrimitiveType get_primitive_type_of_ident(const StringView& str);

struct SemanticAnalyzer
{
	
};
