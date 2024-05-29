use crate::arena::Arena;
use crate::ast::*;
use crate::error::{DiagnosticCollection, ErrorComp, ResultComp};
use crate::intern::{InternID, InternPool};
use crate::session::FileID;
use crate::text::{TextOffset, TextRange};
use crate::token::token_list::TokenList;
use crate::token::Token;

pub struct Parser<'ast, 'intern, 'src, 'state> {
    pub cursor: usize,
    tokens: TokenList,
    char_id: u32,
    string_id: u32,
    file_id: FileID,
    pub source: &'src str,
    pub state: &'state mut ParseState<'ast, 'intern>,
}

pub struct ParseState<'ast, 'intern> {
    pub arena: Arena<'ast>,
    pub intern_name: InternPool<'intern>,
    pub intern_string: InternPool<'intern>,
    pub string_is_cstr: Vec<bool>,
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
        file_id: FileID,
        source: &'src str,
        state: &'state mut ParseState<'ast, 'intern>,
    ) -> Self {
        Self {
            cursor: 0,
            tokens,
            char_id: 0,
            string_id: 0,
            file_id,
            source,
            state,
        }
    }

    pub fn start_range(&self) -> TextOffset {
        self.tokens.get_range(self.cursor).start()
    }

    /// `start` offset must be result of `start_range()` call  
    /// and at least one token must be consumed in between
    pub fn make_range(&self, start: TextOffset) -> TextRange {
        let end = self.tokens.get_range(self.cursor - 1).end();
        TextRange::new(start, end)
    }

    pub fn peek_range(&self) -> TextRange {
        self.tokens.get_range(self.cursor)
    }

    pub fn at(&self, t: Token) -> bool {
        self.peek() == t
    }

    // would be good to remove the need for forward peeking @14.04.24
    // used in path -> import -> struct_init parsing with `.{`
    pub fn at_next(&self, t: Token) -> bool {
        self.peek_next() == t
    }

    pub fn at_prev(&self, t: Token) -> bool {
        self.tokens.get_token(self.cursor - 1) == t
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

    pub fn get_char_lit(&mut self) -> char {
        let value = self.tokens.get_char(self.char_id as usize);
        self.char_id += 1;
        value
    }

    pub fn get_string_lit(&mut self) -> (InternID, bool) {
        let (string, c_string) = self.tokens.get_string(self.string_id as usize);
        let id = self.state.intern_string.intern(string);

        if id.index() >= self.state.string_is_cstr.len() {
            self.state.string_is_cstr.push(c_string);
        } else if c_string {
            self.state.string_is_cstr[id.index()] = true;
        }

        self.string_id += 1;
        (id, c_string)
    }

    pub fn file_id(&self) -> FileID {
        self.file_id
    }
}

impl<'ast, 'intern> ParseState<'ast, 'intern> {
    pub fn new() -> ParseState<'ast, 'intern> {
        ParseState {
            arena: Arena::new(),
            intern_name: InternPool::new(),
            intern_string: InternPool::new(),
            string_is_cstr: Vec::with_capacity(1024),
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

    pub fn result(self) -> ResultComp<Ast<'ast, 'intern>> {
        let ast = Ast {
            arena: self.arena,
            intern_name: self.intern_name,
            intern_string: self.intern_string,
            string_is_cstr: self.string_is_cstr,
            packages: self.packages,
        };
        if self.errors.is_empty() {
            ResultComp::Ok((ast, vec![]))
        } else {
            ResultComp::Err(DiagnosticCollection::new().join_errors(self.errors))
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
