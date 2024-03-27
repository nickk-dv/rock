mod grammar;

use crate::arena::Arena;
use crate::ast::*;
use crate::error::ErrorComp;
use crate::intern::InternPool;
use crate::lexer::Lexer;
use crate::session::Session;
use crate::text::{TextOffset, TextRange};
use crate::token::token_list::TokenList;
use crate::token::Token;

pub fn parse<'ast>(session: &Session) -> Result<Ast<'ast>, Vec<ErrorComp>> {
    let mut state = ParseState::new();

    for file_id in session.file_ids() {
        let file = session.file(file_id);
        let lexer = Lexer::new(&file.source, false);
        let tokens = lexer.lex();
        let parser = Parser::new(tokens, &file.source, &mut state);

        match grammar::module(parser, file_id) {
            Ok(it) => state.modules.push(it),
            Err(error) => state.errors.push(error),
        }
    }

    state.finish()
}

struct Parser<'ast, 'src, 'state> {
    cursor: usize,
    tokens: TokenList,
    char_id: u32,
    string_id: u32,
    source: &'src str,
    state: &'state mut ParseState<'ast>,
}

struct ParseState<'ast> {
    arena: Arena<'ast>,
    intern: InternPool,
    modules: Vec<Module<'ast>>,
    errors: Vec<ErrorComp>,
    items: NodeBuffer<Item<'ast>>,
    use_symbols: NodeBuffer<UseSymbol>,
    proc_params: NodeBuffer<ProcParam<'ast>>,
    enum_variants: NodeBuffer<EnumVariant<'ast>>,
    union_members: NodeBuffer<UnionMember<'ast>>,
    struct_fields: NodeBuffer<StructField<'ast>>,
    names: NodeBuffer<Ident>,
    stmts: NodeBuffer<Stmt<'ast>>,
    branches: NodeBuffer<Branch<'ast>>,
    match_arms: NodeBuffer<MatchArm<'ast>>,
    exprs: NodeBuffer<&'ast Expr<'ast>>,
    field_inits: NodeBuffer<FieldInit<'ast>>,
}

impl<'ast, 'src, 'state> Parser<'ast, 'src, 'state> {
    pub fn new(tokens: TokenList, source: &'src str, state: &'state mut ParseState<'ast>) -> Self {
        Self {
            cursor: 0,
            tokens,
            char_id: 0,
            string_id: 0,
            source,
            state,
        }
    }

    pub fn at(&self, t: Token) -> bool {
        self.peek() == t
    }

    pub fn at_next(&self, t: Token) -> bool {
        self.peek_next() == t
    }

    fn peek(&self) -> Token {
        self.tokens.get_token(self.cursor)
    }

    fn peek_next(&self) -> Token {
        self.tokens.get_token(self.cursor + 1)
    }

    fn eat(&mut self, t: Token) -> bool {
        if self.at(t) {
            self.bump();
            return true;
        }
        false
    }

    fn bump(&mut self) {
        self.cursor += 1;
    }

    fn expect(&mut self, t: Token) -> Result<(), String> {
        if self.eat(t) {
            return Ok(());
        }
        Err(format!("expected `{}`", t.as_str()))
    }

    fn peek_range(&self) -> TextRange {
        self.tokens.get_range(self.cursor)
    }

    fn peek_range_start(&self) -> TextOffset {
        self.tokens.get_range(self.cursor).start()
    }

    fn peek_range_end(&self) -> TextOffset {
        self.tokens.get_range(self.cursor - 1).end()
    }
}

impl<'ast> ParseState<'ast> {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(),
            intern: InternPool::new(),
            modules: Vec::new(),
            errors: Vec::new(),
            items: NodeBuffer::new(),
            use_symbols: NodeBuffer::new(),
            proc_params: NodeBuffer::new(),
            enum_variants: NodeBuffer::new(),
            union_members: NodeBuffer::new(),
            struct_fields: NodeBuffer::new(),
            names: NodeBuffer::new(),
            stmts: NodeBuffer::new(),
            branches: NodeBuffer::new(),
            match_arms: NodeBuffer::new(),
            exprs: NodeBuffer::new(),
            field_inits: NodeBuffer::new(),
        }
    }

    pub fn finish(self) -> Result<Ast<'ast>, Vec<ErrorComp>> {
        if self.errors.is_empty() {
            Ok(Ast {
                arena: self.arena,
                intern: self.intern,
                modules: self.modules,
            })
        } else {
            Err(self.errors)
        }
    }
}

struct NodeOffset(usize);
struct NodeBuffer<T: Copy> {
    buffer: Vec<T>,
}

impl<T: Copy> NodeBuffer<T> {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn start(&self) -> NodeOffset {
        NodeOffset(self.buffer.len())
    }

    fn add(&mut self, value: T) {
        self.buffer.push(value);
    }

    fn take<'arena>(&mut self, start: NodeOffset, arena: &mut Arena<'arena>) -> &'arena [T] {
        let slice = arena.alloc_slice(&self.buffer[start.0..]);
        self.buffer.truncate(start.0);
        slice
    }
}
