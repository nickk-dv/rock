#include "token.h"

#include <unordered_map>

static const std::unordered_map<u64, TokenType> keyword_hash_to_token_type =
{
	{ hash_ascii_9("struct"),   TokenType::KEYWORD_STRUCT },
	{ hash_ascii_9("enum"),     TokenType::KEYWORD_ENUM },
	{ hash_ascii_9("if"),       TokenType::KEYWORD_IF },
	{ hash_ascii_9("else"),     TokenType::KEYWORD_ELSE },
	{ hash_ascii_9("true"),     TokenType::KEYWORD_TRUE },
	{ hash_ascii_9("false"),    TokenType::KEYWORD_FALSE },
	{ hash_ascii_9("for"),      TokenType::KEYWORD_FOR },
	{ hash_ascii_9("cast"),     TokenType::KEYWORD_CAST },
	{ hash_ascii_9("defer"),    TokenType::KEYWORD_DEFER },
	{ hash_ascii_9("break"),    TokenType::KEYWORD_BREAK },
	{ hash_ascii_9("return"),   TokenType::KEYWORD_RETURN },
	{ hash_ascii_9("switch"),   TokenType::KEYWORD_SWITCH },
	{ hash_ascii_9("continue"), TokenType::KEYWORD_CONTINUE },
	{ hash_ascii_9("sizeof"),   TokenType::KEYWORD_SIZEOF },
	{ hash_ascii_9("import"),   TokenType::KEYWORD_IMPORT },
	{ hash_ascii_9("use"),      TokenType::KEYWORD_USE },
	{ hash_ascii_9("impl"),     TokenType::KEYWORD_IMPL },
	{ hash_ascii_9("self"),     TokenType::KEYWORD_SELF },
	{ hash_ascii_9("i8"),       TokenType::TYPE_I8 },
	{ hash_ascii_9("u8"),       TokenType::TYPE_U8 },
	{ hash_ascii_9("i16"),      TokenType::TYPE_I16 },
	{ hash_ascii_9("u16"),      TokenType::TYPE_U16 },
	{ hash_ascii_9("i32"),      TokenType::TYPE_I32 },
	{ hash_ascii_9("u32"),      TokenType::TYPE_U32 },
	{ hash_ascii_9("i64"),      TokenType::TYPE_I64 },
	{ hash_ascii_9("u64"),      TokenType::TYPE_U64 },
	{ hash_ascii_9("f32"),      TokenType::TYPE_F32 },
	{ hash_ascii_9("f64"),      TokenType::TYPE_F64 },
	{ hash_ascii_9("bool"),     TokenType::TYPE_BOOL },
	{ hash_ascii_9("string"),   TokenType::TYPE_STRING },
};

TokenType token_str_to_keyword(StringView str)
{
	if (str.count > 8 || str.count < 2) return TokenType::ERROR;
	u64 hash = hash_str_ascii_9(str);
	bool is_keyword = keyword_hash_to_token_type.find(hash) != keyword_hash_to_token_type.end();
	return is_keyword ? keyword_hash_to_token_type.at(hash) : TokenType::ERROR;
}
