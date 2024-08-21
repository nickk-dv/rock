use crate::arena::Arena;
use crate::ast::*;
use crate::error::{DiagnosticCollection, ErrorComp, ResultComp};
use crate::intern::{InternID, InternPool};
use crate::session::ModuleID;
use crate::temp_buffer::TempBuffer;
use crate::text::{TextOffset, TextRange};
use crate::token::token_list::TokenList;
use crate::token::Token;

pub struct Parser<'ast, 'intern, 'src, 'state> {
    pub cursor: usize,
    tokens: TokenList,
    int_id: u32,
    char_id: u32,
    string_id: u32,
    pub module_id: ModuleID,
    pub source: &'src str,
    pub state: &'state mut ParseState<'ast, 'intern>,
}

pub struct ParseState<'ast, 'intern> {
    pub arena: Arena<'ast>,
    pub intern_name: InternPool<'intern>,
    pub intern_string: InternPool<'intern>,
    pub string_is_cstr: Vec<bool>,
    pub modules: Vec<Module<'ast>>,
    pub errors: Vec<ErrorComp>,
    pub items: TempBuffer<Item<'ast>>,
    pub attrs: TempBuffer<Attribute<'ast>>,
    pub attr_params: TempBuffer<AttributeParam>,
    pub params: TempBuffer<Param<'ast>>,
    pub variants: TempBuffer<Variant<'ast>>,
    pub fields: TempBuffer<Field<'ast>>,
    pub import_symbols: TempBuffer<ImportSymbol>,
    pub names: TempBuffer<Name>,
    pub types: TempBuffer<Type<'ast>>,
    pub stmts: TempBuffer<Stmt<'ast>>,
    pub branches: TempBuffer<Branch<'ast>>,
    pub match_arms: TempBuffer<MatchArm<'ast>>,
    pub match_arms_2: TempBuffer<MatchArm2<'ast>>,
    pub patterns: TempBuffer<Pat<'ast>>,
    pub exprs: TempBuffer<&'ast Expr<'ast>>,
    pub field_inits: TempBuffer<FieldInit<'ast>>,
}

impl<'ast, 'intern, 'src, 'state> Parser<'ast, 'intern, 'src, 'state> {
    pub fn new(
        tokens: TokenList,
        module_id: ModuleID,
        source: &'src str,
        state: &'state mut ParseState<'ast, 'intern>,
    ) -> Self {
        Self {
            cursor: 0,
            tokens,
            int_id: 0,
            char_id: 0,
            string_id: 0,
            module_id,
            source,
            state,
        }
    }

    pub fn start_range(&self) -> TextOffset {
        self.tokens.token_range(self.cursor).start()
    }

    /// `start` offset must be result of `start_range()` call  
    /// and at least one token must be consumed in between
    pub fn make_range(&self, start: TextOffset) -> TextRange {
        let end = self.tokens.token_range(self.cursor - 1).end();
        TextRange::new(start, end)
    }

    pub fn peek_range(&self) -> TextRange {
        self.tokens.token_range(self.cursor)
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
        self.tokens.token(self.cursor - 1) == t
    }

    pub fn peek(&self) -> Token {
        self.tokens.token(self.cursor)
    }

    // would be good to remove the need for forward peeking @14.04.24
    // used in path -> import -> struct_init parsing with `.{`
    pub fn peek_next(&self) -> Token {
        self.tokens.token(self.cursor + 1)
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

    pub fn get_int_lit(&mut self) -> u64 {
        let value = self.tokens.int(self.int_id as usize);
        self.int_id += 1;
        value
    }

    pub fn get_char_lit(&mut self) -> char {
        let value = self.tokens.char(self.char_id as usize);
        self.char_id += 1;
        value
    }

    pub fn get_string_lit(&mut self) -> (InternID, bool) {
        let (string, c_string) = self.tokens.string(self.string_id as usize);
        let id = self.state.intern_string.intern(string);

        if id.index() >= self.state.string_is_cstr.len() {
            self.state.string_is_cstr.push(c_string);
        } else if c_string {
            self.state.string_is_cstr[id.index()] = true;
        }

        self.string_id += 1;
        (id, c_string)
    }
}

impl<'ast, 'intern> ParseState<'ast, 'intern> {
    pub fn new(intern_name: InternPool<'intern>) -> ParseState<'ast, 'intern> {
        ParseState {
            arena: Arena::new(),
            intern_name,
            intern_string: InternPool::new(),
            string_is_cstr: Vec::with_capacity(1024),
            modules: Vec::new(),
            errors: Vec::new(),
            items: TempBuffer::new(128),
            attrs: TempBuffer::new(32),
            attr_params: TempBuffer::new(32),
            params: TempBuffer::new(32),
            variants: TempBuffer::new(32),
            fields: TempBuffer::new(32),
            import_symbols: TempBuffer::new(32),
            names: TempBuffer::new(32),
            types: TempBuffer::new(32),
            stmts: TempBuffer::new(32),
            branches: TempBuffer::new(32),
            match_arms: TempBuffer::new(32),
            match_arms_2: TempBuffer::new(32),
            patterns: TempBuffer::new(32),
            exprs: TempBuffer::new(32),
            field_inits: TempBuffer::new(32),
        }
    }

    pub fn result(self) -> ResultComp<Ast<'ast, 'intern>> {
        let ast = Ast {
            arena: self.arena,
            intern_name: self.intern_name,
            intern_string: self.intern_string,
            string_is_cstr: self.string_is_cstr,
            modules: self.modules,
        };
        if self.errors.is_empty() {
            ResultComp::Ok((ast, vec![]))
        } else {
            ResultComp::Err(DiagnosticCollection::new().join_errors(self.errors))
        }
    }
}
