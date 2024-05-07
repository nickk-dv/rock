use crate::arena::Arena;
use crate::ast::*;
use crate::error::ErrorComp;
use crate::intern::InternPool;
use crate::text::{TextOffset, TextRange};
use crate::token::token_list::TokenList;
use crate::token::Token;

pub struct Parser<'ast, 'intern, 'src, 'state> {
    pub cursor: usize,
    pub tokens: TokenList,
    pub char_id: u32,
    pub string_id: u32,
    pub source: &'src str,
    pub state: &'state mut ParseState<'ast, 'intern>,
}

pub struct ParseState<'ast, 'intern> {
    pub arena: Arena<'ast>,
    pub intern: InternPool<'intern>,
    pub packages: Vec<Package<'ast>>,
    pub errors: Vec<ErrorComp>,
    pub items: NodeBuffer<Item<'ast>>,
    pub proc_params: NodeBuffer<ProcParam<'ast>>,
    pub enum_variants: NodeBuffer<EnumVariant<'ast>>,
    pub union_members: NodeBuffer<UnionMember<'ast>>,
    pub struct_fields: NodeBuffer<StructField<'ast>>,
    pub import_symbols: NodeBuffer<ImportSymbol>,
    pub names: NodeBuffer<Name>,
    pub types: NodeBuffer<Type<'ast>>,
    pub stmts: NodeBuffer<Stmt<'ast>>,
    pub branches: NodeBuffer<Branch<'ast>>,
    pub match_arms: NodeBuffer<MatchArm<'ast>>,
    pub exprs: NodeBuffer<&'ast Expr<'ast>>,
    pub field_inits: NodeBuffer<FieldInit<'ast>>,
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

    // would be good to remove the need for forward peeking @14.04.24
    // used in path -> import -> struct_init parsing with `.{`
    pub fn at_next(&self, t: Token) -> bool {
        self.peek_next() == t
    }

    pub fn peek(&self) -> Token {
        self.tokens.get_token(self.cursor)
    }

    // would be good to remove the need for forward peeking @14.04.24
    // used in path -> import -> struct_init parsing with `.{`
    pub fn peek_next(&self) -> Token {
        self.tokens.get_token(self.cursor + 1)
    }

    pub fn eat(&mut self, t: Token) -> bool {
        if self.at(t) {
            self.bump();
            return true;
        }
        false
    }

    pub fn bump(&mut self) {
        self.cursor += 1;
    }

    pub fn expect(&mut self, t: Token) -> Result<(), String> {
        if self.eat(t) {
            return Ok(());
        }
        Err(format!("expected `{}`", t.as_str()))
    }

    pub fn peek_range(&self) -> TextRange {
        self.tokens.get_range(self.cursor)
    }

    pub fn peek_range_start(&self) -> TextOffset {
        self.tokens.get_range(self.cursor).start()
    }

    pub fn peek_range_end(&self) -> TextOffset {
        self.tokens.get_range(self.cursor - 1).end()
    }
}

impl<'ast, 'intern> ParseState<'ast, 'intern> {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(),
            intern: InternPool::new(),
            packages: Vec::new(),
            errors: Vec::new(),
            items: NodeBuffer::new(),
            proc_params: NodeBuffer::new(),
            enum_variants: NodeBuffer::new(),
            union_members: NodeBuffer::new(),
            struct_fields: NodeBuffer::new(),
            import_symbols: NodeBuffer::new(),
            names: NodeBuffer::new(),
            types: NodeBuffer::new(),
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
                packages: self.packages,
            })
        } else {
            Err(self.errors)
        }
    }
}

pub struct NodeOffset(usize);
pub struct NodeBuffer<T: Copy> {
    buffer: Vec<T>,
}

impl<T: Copy> NodeBuffer<T> {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn start(&self) -> NodeOffset {
        NodeOffset(self.buffer.len())
    }

    pub fn add(&mut self, value: T) {
        self.buffer.push(value);
    }

    pub fn take<'arena>(&mut self, start: NodeOffset, arena: &mut Arena<'arena>) -> &'arena [T] {
        let slice = arena.alloc_slice(&self.buffer[start.0..]);
        self.buffer.truncate(start.0);
        slice
    }
}
