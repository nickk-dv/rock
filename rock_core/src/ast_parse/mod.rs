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

pub fn parse<'ast>(session: &Session) -> Result<Ast<'_, 'ast>, Vec<ErrorComp>> {
    let mut state = ParseState::new();

    for file_id in session.file_ids() {
        let file = session.file(file_id);
        let filename = file
            .path
            .file_stem()
            .expect("filename")
            .to_str()
            .expect("utf-8");
        let name_id = state.intern.intern(filename);

        let lexer = Lexer::new(&file.source, file_id, false);
        let tokens = match lexer.lex() {
            Ok(it) => it,
            Err(errors) => {
                state.errors.extend(errors);
                continue;
            }
        };
        let parser = Parser::new(tokens, &file.source, &mut state);

        match grammar::module(parser, file_id, name_id) {
            Ok(it) => state.modules.push(it),
            Err(error) => state.errors.push(error),
        }
    }

    state.finish()
}

struct Parser<'ast, 'intern, 'src, 'state> {
    cursor: usize,
    tokens: TokenList,
    char_id: u32,
    string_id: u32,
    source: &'src str,
    state: &'state mut ParseState<'ast, 'intern>,
}

struct ParseState<'ast, 'intern> {
    arena: Arena<'ast>,
    intern: InternPool<'intern>,
    modules: Vec<Module<'ast>>,
    errors: Vec<ErrorComp>,
    items: NodeBuffer<Item<'ast>>,
    proc_params: NodeBuffer<ProcParam<'ast>>,
    enum_variants: NodeBuffer<EnumVariant<'ast>>,
    union_members: NodeBuffer<UnionMember<'ast>>,
    struct_fields: NodeBuffer<StructField<'ast>>,
    import_symbols: NodeBuffer<ImportSymbol>,
    names: NodeBuffer<Name>,
    stmts: NodeBuffer<Stmt<'ast>>,
    branches: NodeBuffer<Branch<'ast>>,
    match_arms: NodeBuffer<MatchArm<'ast>>,
    exprs: NodeBuffer<&'ast Expr<'ast>>,
    field_inits: NodeBuffer<FieldInit<'ast>>,
}

impl<'ast, 'intern, 'src, 'state> Parser<'ast, 'intern, 'src, 'state> {
    pub fn new(
        tokens: TokenList,
        source: &'src str,
        state: &'state mut ParseState<'ast, 'intern>,
    ) -> Self {
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

impl<'ast, 'intern> ParseState<'ast, 'intern> {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(),
            intern: InternPool::new(),
            modules: Vec::new(),
            errors: Vec::new(),
            items: NodeBuffer::new(),
            proc_params: NodeBuffer::new(),
            enum_variants: NodeBuffer::new(),
            union_members: NodeBuffer::new(),
            struct_fields: NodeBuffer::new(),
            import_symbols: NodeBuffer::new(),
            names: NodeBuffer::new(),
            stmts: NodeBuffer::new(),
            branches: NodeBuffer::new(),
            match_arms: NodeBuffer::new(),
            exprs: NodeBuffer::new(),
            field_inits: NodeBuffer::new(),
        }
    }

    pub fn finish(self) -> Result<Ast<'ast, 'intern>, Vec<ErrorComp>> {
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
