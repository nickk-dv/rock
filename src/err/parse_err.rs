use crate::ast::token::*;

pub struct ParseError {
    pub context: ParseContext,
    pub expected: Vec<Token>,
    pub got_token: TokenSpan,
}

impl ParseError {
    pub fn new(context: ParseContext, expected: Vec<Token>, got_token: TokenSpan) -> Self {
        Self {
            context,
            expected,
            got_token,
        }
    }
}

impl ParseError {
    pub fn print_temp(&self) {
        print!("parse error: expected: ");
        for token in self.expected.iter() {
            print!("'{}' ", Token::as_str(*token))
        }
        println!(
            "\nin {} got: {}",
            self.context.as_str(),
            Token::as_str(self.got_token.token)
        );
        println!(
            "span: {} - {}",
            self.got_token.span.start, self.got_token.span.end
        );
    }
}

#[derive(Copy, Clone)]
pub enum ParseContext {
    ModuleAccess,
    Type,
    CustomType,
    ArraySliceType,
    ArrayStaticType,
    Decl,
    ModDecl,
    ProcDecl,
    ProcParam,
    EnumDecl,
    EnumVariant,
    StructDecl,
    StructField,
    GlobalDecl,
    ImportDecl,
    Stmt,
    If,
    For,
    Block,
    Defer,
    Break,
    Switch,
    SwitchCase,
    Return,
    Continue,
    VarDecl,
    VarAssign,
    Expr,
    Var,
    Access,
    ArrayAccess,
    Enum,
    Cast,
    Sizeof,
    Literal,
    ProcCall,
    ArrayInit,
    StructInit,
}

impl ParseContext {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseContext::ModuleAccess => "module access",
            ParseContext::Type => "type",
            ParseContext::CustomType => "custom type",
            ParseContext::ArraySliceType => "array slice type",
            ParseContext::ArrayStaticType => "static array type",
            ParseContext::Decl => "declaration",
            ParseContext::ModDecl => "module declaration",
            ParseContext::ProcDecl => "procedure declaration",
            ParseContext::ProcParam => "procedure parameter",
            ParseContext::EnumDecl => "enum declaration",
            ParseContext::EnumVariant => "enum variant",
            ParseContext::StructDecl => "struct declaration",
            ParseContext::StructField => "struct field",
            ParseContext::GlobalDecl => "global declaration",
            ParseContext::ImportDecl => "import declaration",
            ParseContext::Stmt => "statement",
            ParseContext::If => "if statement",
            ParseContext::For => "for loop statement",
            ParseContext::Block => "statement block",
            ParseContext::Defer => "defer statement",
            ParseContext::Break => "break statement",
            ParseContext::Switch => "switch statement",
            ParseContext::SwitchCase => "switch cast",
            ParseContext::Return => "return statement",
            ParseContext::Continue => "continue statement",
            ParseContext::VarDecl => "variable declaration",
            ParseContext::VarAssign => "variable assignment",
            ParseContext::Expr => "expression",
            ParseContext::Var => "variable",
            ParseContext::Access => "access chain",
            ParseContext::ArrayAccess => "array access",
            ParseContext::Enum => "enum expression",
            ParseContext::Cast => "cast expression",
            ParseContext::Sizeof => "sizeof expression",
            ParseContext::Literal => "literal",
            ParseContext::ProcCall => "procedure call",
            ParseContext::ArrayInit => "array initializer",
            ParseContext::StructInit => "struct initializer",
        }
    }
}
