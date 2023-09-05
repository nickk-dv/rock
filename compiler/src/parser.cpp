#include "parser.h"

#include "common.h"

#include <time.h> //@Performance timing

void Parser::parse(const Lexer& lexer)
{
	clock_t start_clock = clock(); //@Performance timing

	parseTreeNodes.resize(lexer.tokens.size());

	//parse

	clock_t time = clock() - start_clock; //@Performance
	float time_ms = (float)time / CLOCKS_PER_SEC * 1000.0f;

	printf("Parsing done.\n");
	printf("Parser: Time           (Ms): %f \n", time_ms);
	printf("Parser: ParseNodeCount: %llu \n", parseTreeNodes.size());
	printf("Parser: MemoryRequired (Mb): %f \n", double(sizeof(ParseTreeNode) * parseTreeNodes.size()) / (1024.0 * 1024.0));
	printf("Parser: MemoryUsed     (Mb): %f \n", double(sizeof(ParseTreeNode) * parseTreeNodes.capacity()) / (1024.0 * 1024.0));
}
