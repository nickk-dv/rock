#pragma once

#include <vector>
#include <stack>

#include "lexer.h"

struct ParseTreeNode
{
	u64 tokenId = 0;
	u64 tokenIdLeft = 0;
	u64 tokenIdRight = 0;
};

struct Parser
{
	void parse(const Lexer& lexer);

	std::stack<u64> tokenIdStack;
	std::vector<ParseTreeNode> parseTreeNodes;
};
