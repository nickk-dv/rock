
enum BinaryOp
{
	BINARY_OP_PLUS,
	BINARY_OP_MINUS,

	BINARY_OP_TIMES,
	BINARY_OP_DIV,
	BINARY_OP_MOD,

	BINARY_OP_BITSHIFT_LEFT,
	BINARY_OP_BITSHIFT_RIGHT,

	BINARY_OP_LESS,
	BINARY_OP_GREATER,
	BINARY_OP_LESS_EQUALS,
	BINARY_OP_GREATER_EQUALS,

	BINARY_OP_IS_EQUALS,
	BINARY_OP_NOT_EQUALS,

	BINARY_OP_BITWISE_AND,

	BINARY_OP_BITWISE_XOR,

	BINARY_OP_BITWISE_OR,

	BINARY_OP_LOGIC_AND,

	BINARY_OP_LOGIC_OR,

	BINARY_OP_ERROR,
};

BinaryOp ast_binary_op_from_token(TokenType type);
u32 ast_binary_op_precedence(BinaryOp op);

struct Ast_Literal;
struct Ast_Identifier;
struct Ast_Term;
struct Ast_Expression;
struct Ast_Binary_Expression;

struct Ast;
struct Ast_Struct_Declaration;
struct Ast_Enum_Declaration;
struct Ast_Procedure_Declaration;

struct Ast_Block;
struct Ast_Statement;
struct Ast_If;
struct Ast_For;
struct Ast_Break;
struct Ast_Return;
struct Ast_Continue;
struct Ast_Procedure_Call;
struct Ast_Variable_Assignment;
struct Ast_Variable_Declaration;

struct Ast_Literal
{
	Token token;
};

struct Ast_Identifier
{
	Token token;
};

struct Ast_Term
{
	enum class Tag
	{
		Literal, Identifier, ProcedureCall,
	} tag;

	union
	{
		Ast_Literal _literal;
		Ast_Identifier _ident;
		Ast_Procedure_Call* _proc_call;
	};
};

struct Ast_Expression
{
	enum class Tag
	{
		Term, BinaryExpression,
	} tag;

	union
	{
		Ast_Term* _term;
		Ast_Binary_Expression* _bin_expr;
	};
};

//@Incompelete think how unary - ! ~ might work and be parsed
struct Ast_Binary_Expression
{
	BinaryOp op;
	Ast_Expression* left;
	Ast_Expression* right;
};

struct Ast
{
	std::vector<Ast_Struct_Declaration> structs;
	std::vector<Ast_Enum_Declaration> enums;
	std::vector<Ast_Procedure_Declaration> procedures;
};

struct IdentTypePair
{
	Ast_Identifier ident;
	Ast_Identifier type;
};

struct Ast_Struct_Declaration
{
	Ast_Identifier type;
	std::vector<IdentTypePair> fields;
};

struct Ast_Enum_Declaration
{
	Ast_Identifier type;
	std::vector<IdentTypePair> variants;
};

struct Ast_Procedure_Declaration
{
	Ast_Identifier ident;
	std::vector<IdentTypePair> input_parameters;
	std::optional<Ast_Identifier> return_type;
	Ast_Block* block;
};

struct Ast_Block
{
	std::vector<Ast_Statement*> statements;
};

struct Ast_Statement
{
	enum class Tag
	{
		If, For, While, Break, Return, Continue,
		ProcedureCall, VariableAssignment, VariableDeclaration,
	} tag;

	union
	{
		Ast_If* _if;
		Ast_For* _for;
		Ast_Break* _break;
		Ast_Return* _return;
		Ast_Continue* _continue;
		Ast_Procedure_Call* _proc_call;
		Ast_Variable_Assignment* _var_assignment;
		Ast_Variable_Declaration* _var_declaration;
	};
};

struct Ast_If
{
	//@Incomplete
	//Some conditional expr
	Ast_Block* block;
	std::optional<Ast_Block*> else_block;
};

struct Ast_For
{
	//@Incomplete
	//3 expressions
	Ast_Block* block;
};

struct Ast_Break
{
	Token token;
};

struct Ast_Return
{
	Token token;
	std::optional<Ast_Expression*> expr;
};

struct Ast_Continue
{
	Token token;
};

struct Ast_Procedure_Call
{
	Ast_Identifier ident;
	std::vector<Ast_Expression*> input_expressions;
};

struct Ast_Variable_Assignment
{
	Ast_Identifier ident;
	Ast_Expression* expr;
};

struct Ast_Variable_Declaration
{
	Ast_Identifier ident;
	Ast_Identifier type;
	std::optional<Ast_Expression*> expr;
};

static const std::unordered_map<TokenType, BinaryOp> token_to_binary_op =
{
	{TOKEN_LOGIC_AND, BINARY_OP_LOGIC_AND},
	{TOKEN_LOGIC_OR, BINARY_OP_LOGIC_OR},

	{TOKEN_LESS, BINARY_OP_LESS},
	{TOKEN_GREATER, BINARY_OP_GREATER},
	{TOKEN_LESS_EQUALS, BINARY_OP_LESS_EQUALS},
	{TOKEN_GREATER_EQUALS, BINARY_OP_GREATER_EQUALS},
	{TOKEN_IS_EQUALS, BINARY_OP_IS_EQUALS},
	{TOKEN_NOT_EQUALS, BINARY_OP_NOT_EQUALS},

	{TOKEN_PLUS, BINARY_OP_PLUS},
	{TOKEN_MINUS, BINARY_OP_MINUS},

	{TOKEN_TIMES, BINARY_OP_TIMES},
	{TOKEN_DIV, BINARY_OP_DIV},
	{TOKEN_MOD, BINARY_OP_MOD},

	{TOKEN_BITWISE_AND, BINARY_OP_BITWISE_AND},
	{TOKEN_BITWISE_XOR, BINARY_OP_BITWISE_XOR},
	{TOKEN_BITWISE_OR, BINARY_OP_BITWISE_OR},

	{TOKEN_BITSHIFT_LEFT, BINARY_OP_BITSHIFT_LEFT},
	{TOKEN_BITSHIFT_RIGHT, BINARY_OP_BITSHIFT_RIGHT},
};

//@Article about improving clarity of op prec: 
//https://www.foonathan.net/2017/07/operator-precedence/
static const std::unordered_map<BinaryOp, u32> binary_op_precedence =
{
	{BINARY_OP_LOGIC_AND, 0},
	{BINARY_OP_LOGIC_OR, 0},

	{BINARY_OP_LESS, 1},
	{BINARY_OP_GREATER, 1},
	{BINARY_OP_LESS_EQUALS, 1},
	{BINARY_OP_GREATER_EQUALS, 1},
	{BINARY_OP_IS_EQUALS, 1},
	{BINARY_OP_NOT_EQUALS, 1},

	{BINARY_OP_PLUS, 2},
	{BINARY_OP_MINUS, 2},

	{BINARY_OP_TIMES, 3},
	{BINARY_OP_DIV, 3},
	{BINARY_OP_MOD, 3},

	{BINARY_OP_BITWISE_AND, 4},
	{BINARY_OP_BITWISE_XOR, 4},
	{BINARY_OP_BITWISE_OR, 4},

	{BINARY_OP_BITSHIFT_LEFT, 5},
	{BINARY_OP_BITSHIFT_RIGHT, 5},
};

BinaryOp ast_binary_op_from_token(TokenType type)
{
	if (token_to_binary_op.find(type) != token_to_binary_op.end())
		return token_to_binary_op.at(type);
	return BINARY_OP_ERROR;
}

u32 ast_binary_op_precedence(BinaryOp op)
{
	return binary_op_precedence.at(op);
}
