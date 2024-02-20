use super::ast::*;
use super::intern::*;
use super::span::*;
use super::token::*;
use super::token_list::TokenList;
use super::visit;
use crate::err::error::*;
use crate::err::report;
use crate::mem::*;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

//@empty error tokens produce invalid span diagnostic

/// Persistant data across a compilation
pub struct CompCtx {
    files: Vec<File>,
    intern: InternPool,
}

pub struct File {
    pub path: PathBuf,
    pub source: String,
}

#[derive(Clone, Copy)]
pub struct FileID(pub u32);

impl CompCtx {
    pub fn new() -> CompCtx {
        CompCtx {
            files: Vec::new(),
            intern: InternPool::new(),
        }
    }

    pub fn file(&self, file_id: FileID) -> &File {
        if let Some(file) = self.files.get(file_id.raw_index()) {
            return file;
        }
        //@internal error
        panic!("getting file using an invalid ID {}", file_id.raw())
    }

    pub fn intern(&self) -> &InternPool {
        &self.intern
    }

    pub fn intern_mut(&mut self) -> &mut InternPool {
        &mut self.intern
    }

    pub fn add_file(&mut self, path: PathBuf, source: String) -> FileID {
        let id = self.files.len();
        self.files.push(File { path, source });
        FileID(id as u32)
    }
}

impl FileID {
    fn raw(&self) -> u32 {
        self.0
    }

    fn raw_index(&self) -> usize {
        self.0 as usize
    }
}

struct Timer {
    start_time: Instant,
}

impl Timer {
    fn new() -> Self {
        Timer {
            start_time: Instant::now(),
        }
    }

    fn elapsed_ms(self, message: &'static str) {
        let elapsed = self.start_time.elapsed();
        let sec_ms = elapsed.as_secs_f64() * 1000.0;
        let ms = sec_ms + f64::from(elapsed.subsec_nanos()) / 1_000_000.0;
        eprintln!("{}: {:.3} ms", message, ms);
    }
}

pub fn parse() -> Result<(CompCtx, Ast), ()> {
    let timer = Timer::new();
    let mut ctx = CompCtx::new();
    let mut ast = Ast {
        arena: Arena::new(),
        modules: Vec::new(),
    };
    timer.elapsed_ms("parse arena created");

    let timer = Timer::new();
    let files = collect_files(&mut ctx);
    timer.elapsed_ms("collect files");

    // 0.610 ms, 50520 arena bytes
    // 0.450 ms, 50616 arena bytes

    let timer = Timer::new();
    let handle = &mut std::io::BufWriter::new(std::io::stderr());
    for id in files {
        let lexer2 = super::lexer::Lexer::new(&ctx.file(id).source);
        let tokens = lexer2.lex();
        let mut parser = Parser {
            cursor: 0,
            tokens: &tokens,
            arena: &mut ast.arena,
            source: &ctx.file(id).source,
            char_id: 0,
            string_id: 0,
        };

        let res = parser.parse_module(id);
        ast.modules.push(res.0);
        if let Some(error) = res.1 {
            report::report(handle, &error, &ctx);
        } else {
            //@cloning entire source text, since &File + &mut InternPool violates borrow checker
            let mut interner = ModuleInterner {
                file: ctx.file(id).source.clone(),
                intern_pool: ctx.intern_mut(),
                tokens,
            };
            visit::visit_module_with(&mut interner, res.0);
        }
    }
    let _ = handle.flush();
    timer.elapsed_ms("parsed all files");

    report::err_status((ctx, ast))
}

#[must_use]
fn collect_files(ctx: &mut CompCtx) -> Vec<FileID> {
    //relative 'root' path of the project being compiled
    //@hardcoded to 'test' for faster testing
    let mut dir_paths = Vec::new();
    dir_paths.push(PathBuf::from("test"));

    let handle = &mut std::io::BufWriter::new(std::io::stderr());
    let mut filepaths = Vec::new();

    while let Some(dir_path) = dir_paths.pop() {
        match std::fs::read_dir(&dir_path) {
            Ok(dir) => {
                for entry in dir.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(extension) = path.extension() {
                            if extension == "lang" {
                                filepaths.push(path);
                            }
                        }
                    } else if path.is_dir() {
                        dir_paths.push(path);
                    }
                }
            }
            Err(err) => {
                report::report(
                    handle,
                    &Error::file_io(FileIOError::DirRead)
                        .info(err.to_string())
                        .info(format!("path: {:?}", dir_path))
                        .into(),
                    &ctx,
                );
            }
        }
    }

    let mut files = Vec::new();

    for path in filepaths {
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                let id = ctx.add_file(path, source);
                files.push(id);
            }
            Err(err) => {
                report::report(
                    handle,
                    &Error::file_io(FileIOError::FileRead)
                        .info(err.to_string())
                        .info(format!("path: {:?}", path))
                        .into(),
                    &ctx,
                );
            }
        };
    }

    files
}

struct ModuleInterner<'a> {
    file: String,
    intern_pool: &'a mut InternPool,
    tokens: TokenList,
}

impl<'a> visit::MutVisit for ModuleInterner<'a> {
    fn visit_ident(&mut self, ident: &mut Ident) {
        let string = ident.span.slice(&self.file);
        ident.id = self.intern_pool.intern(string);
    }

    fn visit_expr(&mut self, mut expr: P<Expr>) {
        if let ExprKind::LitString { ref mut id } = expr.kind {
            let string = self.tokens.string(id.0 as usize);
            *id = self.intern_pool.intern(string);
        }
    }
}

//@sequential ids of chars / strings
// might not be always correct when dealing with bin op precedence
struct Parser<'ast> {
    cursor: usize,
    tokens: &'ast TokenList,
    arena: &'ast mut Arena,
    source: &'ast str,
    char_id: u32,
    string_id: u32,
}

impl<'ast> Parser<'ast> {
    fn parse_module(&mut self, file_id: FileID) -> (P<Module>, Option<Error>) {
        let mut module = self.alloc::<Module>();
        let mut decls = ListBuilder::new();
        module.file_id = file_id;

        while self.peek() != Token::Eof {
            match self.parse_decl() {
                Ok(decl) => {
                    decls.add(&mut self.arena, decl);
                }
                Err(err) => {
                    let got_token = (self.peek(), self.peek_span());
                    return (module, Some(Error::parse(err, file_id, got_token)));
                }
            }
        }

        module.decls = decls.take();
        (module, None)
    }

    fn parse_ident(&mut self, context: ParseContext) -> Result<Ident, ParseError> {
        if self.peek() == Token::Ident {
            let span = self.peek_span();
            self.eat();
            return Ok(Ident {
                span,
                id: INTERN_DUMMY_ID,
            });
        }
        Err(ParseError::Ident(context))
    }

    fn parse_vis(&mut self) -> Vis {
        if self.try_eat(Token::KwPub) {
            Vis::Public
        } else {
            Vis::Private
        }
    }

    fn parse_mut(&mut self) -> Mut {
        if self.try_eat(Token::KwMut) {
            Mut::Mutable
        } else {
            Mut::Immutable
        }
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut ty = Type {
            ptr: PtrLevel::new(),
            kind: TypeKind::Basic(BasicType::Unit),
        };
        while self.try_eat(Token::Star) {
            let mutt = self.parse_mut();
            if let Err(..) = ty.ptr.add_level(mutt) {
                //@overflown ptr indirection span cannot be captured by current err system
                // silently ignoring this error
            }
        }
        if let Some(basic) = self.try_eat_basic_type() {
            ty.kind = TypeKind::Basic(basic);
            return Ok(ty);
        }
        ty.kind = match self.peek() {
            Token::OpenParen => {
                self.eat();
                self.expect(Token::CloseParen, ParseContext::UnitType)?;
                TypeKind::Basic(BasicType::Unit)
            }
            Token::Ident | Token::KwSuper | Token::KwPackage => {
                TypeKind::Custom(self.parse_path()?)
            }
            Token::OpenBracket => match self.peek_next() {
                Token::KwMut | Token::CloseBracket => {
                    self.eat();
                    let mut array_slice = self.alloc::<ArraySlice>();
                    array_slice.mutt = self.parse_mut();
                    self.expect(Token::CloseBracket, ParseContext::ArraySlice)?;
                    array_slice.ty = self.parse_type()?;
                    TypeKind::ArraySlice(array_slice)
                }
                _ => {
                    self.eat();
                    let mut array_static = self.alloc::<ArrayStatic>();
                    array_static.size = ConstExpr(self.parse_expr()?);
                    self.expect(Token::CloseBracket, ParseContext::ArrayStatic)?;
                    array_static.ty = self.parse_type()?;
                    TypeKind::ArrayStatic(array_static)
                }
            },
            _ => return Err(ParseError::TypeMatch),
        };
        Ok(ty)
    }

    fn parse_path(&mut self) -> Result<P<Path>, ParseError> {
        let mut path = self.alloc::<Path>();
        let mut names = ListBuilder::new();

        path.span_start = self.peek_span_start();
        path.kind = match self.peek() {
            Token::KwSuper => {
                self.eat();
                PathKind::Super
            }
            Token::KwPackage => {
                self.eat();
                PathKind::Package
            }
            _ => {
                let name = self.parse_ident(ParseContext::ModulePath)?; //@take specific ctx instead?
                names.add(&mut self.arena, name);
                PathKind::None
            }
        };
        while self.peek() == Token::Dot && self.peek_next() != Token::OpenBlock {
            self.eat();
            let name = self.parse_ident(ParseContext::ModulePath)?; //@take specific ctx instead?
            names.add(&mut self.arena, name);
        }

        path.names = names.take();
        Ok(path)
    }

    fn parse_decl(&mut self) -> Result<Decl, ParseError> {
        match self.peek() {
            Token::KwImport => Ok(Decl::Import(self.parse_import_decl()?)),
            Token::Ident | Token::KwPub => {
                let vis = self.parse_vis();
                let name = self.parse_ident(ParseContext::Decl)?;
                if self.peek() == Token::Colon {
                    return Ok(Decl::Global(self.parse_global_decl(vis, name)?));
                }
                self.expect(Token::ColonColon, ParseContext::Decl)?;
                match self.peek() {
                    Token::KwMod => Ok(Decl::Module(self.parse_module_decl(vis, name)?)),
                    Token::OpenParen => Ok(Decl::Proc(self.parse_proc_decl(vis, name)?)),
                    Token::KwEnum => Ok(Decl::Enum(self.parse_enum_decl(vis, name)?)),
                    Token::KwUnion => Ok(Decl::Union(self.parse_union_decl(vis, name)?)),
                    Token::KwStruct => Ok(Decl::Struct(self.parse_struct_decl(vis, name)?)),
                    _ => Err(ParseError::DeclMatchKw),
                }
            }
            _ => Err(ParseError::DeclMatch),
        }
    }

    fn parse_module_decl(&mut self, vis: Vis, name: Ident) -> Result<P<ModuleDecl>, ParseError> {
        self.eat(); // `mod`
        self.expect(Token::Semicolon, ParseContext::ModDecl)?;
        let mut module_decl = self.alloc::<ModuleDecl>();
        module_decl.vis = vis;
        module_decl.name = name;
        Ok(module_decl)
    }

    fn parse_import_decl(&mut self) -> Result<P<ImportDecl>, ParseError> {
        self.eat(); // `import`
        let mut import_decl = self.alloc::<ImportDecl>();
        let mut symbols = ListBuilder::new();
        import_decl.path = self.parse_path()?;
        self.expect(Token::Dot, ParseContext::ImportDecl)?;
        self.expect(Token::OpenBlock, ParseContext::ImportDecl)?;
        if !self.try_eat(Token::CloseBlock) {
            loop {
                let symbol = self.parse_import_symbol()?;
                symbols.add(&mut self.arena, symbol);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::CloseBlock, ParseContext::ImportDecl)?;
        }
        import_decl.symbols = symbols.take();
        Ok(import_decl)
    }

    fn parse_import_symbol(&mut self) -> Result<ImportSymbol, ParseError> {
        let mut symbol = ImportSymbol {
            name: self.parse_ident(ParseContext::ImportDecl)?, // @ctx
            alias: None,
        };
        if self.try_eat(Token::KwAs) {
            symbol.alias = Some(self.parse_ident(ParseContext::ImportDecl)?); // @ctx
        }
        Ok(symbol)
    }

    fn parse_global_decl(&mut self, vis: Vis, name: Ident) -> Result<P<GlobalDecl>, ParseError> {
        self.eat(); // `:`
        let mut global_decl = self.alloc::<GlobalDecl>();
        global_decl.vis = vis;
        global_decl.name = name;

        if self.try_eat(Token::Equals) {
            global_decl.ty = None;
            global_decl.value = ConstExpr(self.parse_expr()?);
        } else {
            global_decl.ty = Some(self.parse_type()?);
            self.expect(Token::Equals, ParseContext::GlobalDecl)?;
            global_decl.value = ConstExpr(self.parse_expr()?);
        }
        self.expect(Token::Semicolon, ParseContext::GlobalDecl)?;
        Ok(global_decl)
    }

    fn parse_proc_decl(&mut self, vis: Vis, name: Ident) -> Result<P<ProcDecl>, ParseError> {
        self.eat(); // `(`
        let mut proc_decl = self.alloc::<ProcDecl>();
        let mut params = ListBuilder::new();
        proc_decl.vis = vis;
        proc_decl.name = name;

        if !self.try_eat(Token::CloseParen) {
            loop {
                if self.try_eat(Token::DotDot) {
                    proc_decl.is_variadic = true;
                    break;
                }
                let param = self.parse_proc_param()?;
                params.add(&mut self.arena, param);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::CloseParen, ParseContext::ProcDecl)?;
        }

        proc_decl.return_ty = if self.try_eat(Token::ArrowThin) {
            Some(self.parse_type()?)
        } else {
            None
        };
        proc_decl.block = if !self.try_eat(Token::DirCCall) {
            Some(self.parse_block()?)
        } else {
            None
        };
        proc_decl.params = params.take();
        Ok(proc_decl)
    }

    fn parse_proc_param(&mut self) -> Result<ProcParam, ParseError> {
        let mutt = self.parse_mut();
        let name = self.parse_ident(ParseContext::ProcParam)?;
        self.expect(Token::Colon, ParseContext::ProcParam)?;
        let ty = self.parse_type()?;
        Ok(ProcParam { mutt, name, ty })
    }

    fn parse_enum_decl(&mut self, vis: Vis, name: Ident) -> Result<P<EnumDecl>, ParseError> {
        self.eat(); // `enum`
        let mut enum_decl = self.alloc::<EnumDecl>();
        let mut variants = ListBuilder::new();
        enum_decl.vis = vis;
        enum_decl.name = name;

        enum_decl.basic_ty = self.try_eat_basic_type();
        self.expect(Token::OpenBlock, ParseContext::EnumDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let variant = self.parse_enum_variant()?;
            variants.add(&mut self.arena, variant);
        }
        enum_decl.variants = variants.take();
        Ok(enum_decl)
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, ParseError> {
        let name = self.parse_ident(ParseContext::EnumVariant)?;
        let value = if self.try_eat(Token::Equals) {
            Some(ConstExpr(self.parse_expr()?))
        } else {
            None
        };
        self.expect(Token::Semicolon, ParseContext::EnumVariant)?;
        Ok(EnumVariant { name, value })
    }

    fn parse_union_decl(&mut self, vis: Vis, name: Ident) -> Result<P<UnionDecl>, ParseError> {
        self.eat(); // `union`
        let mut union_decl = self.alloc::<UnionDecl>();
        let mut members = ListBuilder::new();
        union_decl.vis = vis;
        union_decl.name = name;

        self.expect(Token::OpenBlock, ParseContext::UnionDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let member = self.parse_union_member()?;
            members.add(&mut self.arena, member);
        }
        union_decl.members = members.take();
        Ok(union_decl)
    }

    fn parse_union_member(&mut self) -> Result<UnionMember, ParseError> {
        let name = self.parse_ident(ParseContext::UnionMember)?;
        self.expect(Token::Colon, ParseContext::UnionMember)?;
        let ty = self.parse_type()?;
        self.expect(Token::Semicolon, ParseContext::UnionMember)?;
        Ok(UnionMember { name, ty })
    }

    fn parse_struct_decl(&mut self, vis: Vis, name: Ident) -> Result<P<StructDecl>, ParseError> {
        self.eat(); // `struct`
        let mut struct_decl = self.alloc::<StructDecl>();
        let mut fields = ListBuilder::new();
        struct_decl.vis = vis;
        struct_decl.name = name;

        self.expect(Token::OpenBlock, ParseContext::StructDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let field = self.parse_struct_field()?;
            fields.add(&mut self.arena, field);
        }
        struct_decl.fields = fields.take();
        Ok(struct_decl)
    }

    fn parse_struct_field(&mut self) -> Result<StructField, ParseError> {
        let vis = self.parse_vis();
        let name = self.parse_ident(ParseContext::StructField)?;
        self.expect(Token::Colon, ParseContext::StructField)?;
        let ty = self.parse_type()?;
        self.expect(Token::Semicolon, ParseContext::StructField)?;
        Ok(StructField { vis, name, ty })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span_start = self.peek_span_start();
        let kind = match self.peek() {
            Token::KwBreak => {
                self.eat();
                self.expect(Token::Semicolon, ParseContext::Break)?;
                StmtKind::Break
            }
            Token::KwContinue => {
                self.eat();
                self.expect(Token::Semicolon, ParseContext::Continue)?;
                StmtKind::Continue
            }
            Token::KwFor => {
                self.eat();
                StmtKind::ForLoop(self.parse_for()?)
            }
            Token::KwDefer => {
                self.eat();
                StmtKind::Defer(self.parse_block()?)
            }
            Token::KwReturn => {
                self.eat();
                if self.try_eat(Token::Semicolon) {
                    StmtKind::Return(None)
                } else {
                    let expr = self.parse_expr()?;
                    self.expect(Token::Semicolon, ParseContext::Return)?;
                    StmtKind::Return(Some(expr))
                }
            }
            _ => self.parse_stmt_no_keyword()?,
        };
        Ok(Stmt {
            kind,
            span: Span::new(span_start, self.peek_span_end()),
        })
    }

    fn parse_for(&mut self) -> Result<P<For>, ParseError> {
        let mut for_ = self.alloc::<For>();
        match self.peek() {
            Token::OpenBlock => {
                for_.kind = ForKind::Loop;
            }
            _ => {
                let expect_var_bind = (self.peek() == Token::KwMut)
                    || ((self.peek() == Token::Ident || self.peek() == Token::Underscore)
                        && self.peek_next() == Token::Colon);
                if expect_var_bind {
                    //@all 3 are expected
                    let var_decl = self.parse_var_decl()?;
                    let cond = self.parse_expr()?;
                    self.expect(Token::Semicolon, ParseContext::For)?;
                    let mut var_assign = self.alloc::<VarAssign>();
                    var_assign.lhs = self.parse_expr()?;
                    var_assign.op = match self.peek().as_assign_op() {
                        Some(op) => {
                            self.eat();
                            op
                        }
                        _ => return Err(ParseError::ForAssignOp),
                    };
                    var_assign.rhs = self.parse_expr()?;
                    for_.kind = ForKind::ForLoop {
                        var_decl,
                        cond,
                        var_assign,
                    };
                } else {
                    for_.kind = ForKind::While {
                        cond: self.parse_expr()?,
                    };
                }
            }
        }
        for_.block = self.parse_block()?;
        Ok(for_)
    }

    fn parse_stmt_no_keyword(&mut self) -> Result<StmtKind, ParseError> {
        let expect_var_bind = (self.peek() == Token::KwMut)
            || ((self.peek() == Token::Ident || self.peek() == Token::Underscore)
                && self.peek_next() == Token::Colon);
        if expect_var_bind {
            Ok(StmtKind::VarDecl(self.parse_var_decl()?))
        } else {
            let expr = self.parse_expr()?;
            match self.peek().as_assign_op() {
                Some(op) => {
                    self.eat();
                    let mut var_assign = self.alloc::<VarAssign>();
                    var_assign.op = op;
                    var_assign.lhs = expr;
                    var_assign.rhs = self.parse_expr()?;
                    self.expect(Token::Semicolon, ParseContext::VarAssign)?;
                    Ok(StmtKind::VarAssign(var_assign))
                }
                None => {
                    if self.try_eat(Token::Semicolon) {
                        Ok(StmtKind::ExprSemi(expr))
                    } else {
                        Ok(StmtKind::ExprTail(expr))
                    }
                }
            }
        }
    }

    fn parse_var_decl(&mut self) -> Result<P<VarDecl>, ParseError> {
        let mut var_decl = self.alloc::<VarDecl>();
        var_decl.mutt = self.parse_mut();
        var_decl.name = if self.try_eat(Token::Underscore) {
            None
        } else {
            Some(self.parse_ident(ParseContext::VarDecl)?)
        };
        self.expect(Token::Colon, ParseContext::VarDecl)?;

        if self.try_eat(Token::Equals) {
            var_decl.ty = None;
            var_decl.expr = Some(self.parse_expr()?);
        } else {
            var_decl.ty = Some(self.parse_type()?);
            var_decl.expr = if self.try_eat(Token::Equals) {
                Some(self.parse_expr()?)
            } else {
                None
            }
        }
        self.expect(Token::Semicolon, ParseContext::VarDecl)?;
        Ok(var_decl)
    }

    fn parse_expr(&mut self) -> Result<P<Expr>, ParseError> {
        self.parse_sub_expr(0)
    }

    fn parse_sub_expr(&mut self, min_prec: u32) -> Result<P<Expr>, ParseError> {
        let mut expr_lhs = self.parse_primary_expr()?;
        loop {
            let prec: u32;
            let binary_op: BinOp;
            if let Some(op) = self.peek().as_bin_op() {
                binary_op = op;
                prec = op.prec();
                if prec < min_prec {
                    break;
                }
                self.eat();
            } else {
                break;
            }
            let mut expr = self.alloc::<Expr>();
            let op = binary_op;
            let lhs = expr_lhs;
            let rhs = self.parse_sub_expr(prec + 1)?;
            expr.span = Span::new(lhs.span.start, rhs.span.end);
            expr.kind = ExprKind::BinaryExpr { op, lhs, rhs };
            expr_lhs = expr;
        }
        Ok(expr_lhs)
    }

    fn parse_primary_expr(&mut self) -> Result<P<Expr>, ParseError> {
        let span_start = self.peek_span_start();

        if self.try_eat(Token::OpenParen) {
            if self.try_eat(Token::CloseParen) {
                let mut expr = self.alloc::<Expr>();
                expr.kind = ExprKind::Unit;
                expr.span = Span::new(span_start, self.peek_span_end());
                return self.parse_tail_expr(expr);
            }
            let expr = self.parse_sub_expr(0)?;
            self.expect(Token::CloseParen, ParseContext::Expr)?;
            return self.parse_tail_expr(expr);
        }

        let mut expr = self.alloc::<Expr>();

        if let Some(unary_op) = self.try_eat_un_op() {
            let op = unary_op;
            let rhs = self.parse_primary_expr()?;

            expr.span = Span::new(span_start, self.peek_span_end());
            expr.kind = ExprKind::UnaryExpr { op, rhs };
            return Ok(expr);
        }

        expr.kind = match self.peek() {
            Token::Underscore => {
                self.eat();
                ExprKind::Discard
            }
            Token::KwIf => ExprKind::If {
                if_: self.parse_if()?,
            },
            Token::KwNull
            | Token::KwTrue
            | Token::KwFalse
            | Token::IntLit
            | Token::FloatLit
            | Token::CharLit
            | Token::StringLit => self.parse_lit()?,
            Token::OpenBlock => ExprKind::Block {
                stmts: self.parse_block_stmts()?,
            },
            Token::KwMatch => {
                self.eat();
                let on_expr = self.parse_expr()?;
                let mut arms = ListBuilder::new();
                self.expect(Token::OpenBlock, ParseContext::Match)?;
                while !self.try_eat(Token::CloseBlock) {
                    let arm = self.parse_match_arm()?;
                    arms.add(&mut self.arena, arm);
                }
                ExprKind::Match {
                    on_expr,
                    arms: arms.take(),
                }
            }
            Token::KwSizeof => {
                self.eat();
                self.expect(Token::OpenParen, ParseContext::Sizeof)?;
                let mut pty = self.alloc::<Type>();
                *pty = self.parse_type()?;
                self.expect(Token::CloseParen, ParseContext::Sizeof)?;
                ExprKind::Sizeof { ty: pty }
            }
            Token::OpenBracket => {
                self.eat();
                if self.try_eat(Token::CloseBracket) {
                    ExprKind::ArrayInit {
                        input: ListBuilder::new().take(),
                    }
                } else {
                    let expr = self.parse_expr()?;
                    if self.try_eat(Token::Semicolon) {
                        let size = ConstExpr(self.parse_expr()?);
                        self.expect(Token::CloseBracket, ParseContext::ArrayInit)?;
                        ExprKind::ArrayRepeat { expr, size }
                    } else {
                        let mut input = ListBuilder::new();
                        input.add(&mut self.arena, expr);
                        if !self.try_eat(Token::CloseBracket) {
                            self.expect(Token::Comma, ParseContext::ArrayInit)?;
                            loop {
                                let expr = self.parse_expr()?;
                                input.add(&mut self.arena, expr);
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::CloseBracket, ParseContext::ArrayInit)?;
                        }
                        ExprKind::ArrayInit {
                            input: input.take(),
                        }
                    }
                }
            }
            _ => {
                let path = self.parse_path()?;

                match (self.peek(), self.peek_next()) {
                    (Token::OpenParen, ..) => {
                        self.eat(); // `(`
                        let input =
                            self.parse_expr_list(Token::CloseParen, ParseContext::ProcCall)?;
                        ExprKind::ProcCall { path, input }
                    }
                    (Token::Dot, Token::OpenBlock) => {
                        self.eat(); // `.`
                        self.eat(); // `{`
                        let mut input = ListBuilder::new();
                        if !self.try_eat(Token::CloseBlock) {
                            loop {
                                let name = self.parse_ident(ParseContext::StructInit)?;
                                let expr = match self.peek() {
                                    Token::Colon => {
                                        self.eat();
                                        Some(self.parse_expr()?)
                                    }
                                    Token::Comma | Token::CloseBlock => None,
                                    _ => return Err(ParseError::FieldInit),
                                };
                                input.add(&mut self.arena, FieldInit { name, expr });
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::CloseBlock, ParseContext::StructInit)?;
                        }
                        ExprKind::StructInit {
                            path,
                            input: input.take(),
                        }
                    }
                    _ => ExprKind::Item { path },
                }
            }
        };
        expr.span = Span::new(span_start, self.peek_span_end());
        return self.parse_tail_expr(expr);
    }

    fn parse_tail_expr(&mut self, expr: P<Expr>) -> Result<P<Expr>, ParseError> {
        let mut target = expr;
        let mut last_cast = false;
        let span_start = expr.span.start;
        loop {
            match self.peek() {
                Token::Dot => {
                    if last_cast {
                        return Ok(target);
                    }
                    self.eat();
                    let mut expr_field = self.alloc::<Expr>();
                    let name = self.parse_ident(ParseContext::Expr)?; //@ctx expr field
                    expr_field.kind = ExprKind::Field { target, name };
                    expr_field.span = Span::new(span_start, self.peek_span_end());
                    target = expr_field;
                }
                Token::OpenBracket => {
                    if last_cast {
                        return Ok(target);
                    }
                    self.eat();
                    let mut expr_index = self.alloc::<Expr>();
                    let index = self.parse_expr()?;
                    self.expect(Token::CloseBracket, ParseContext::Expr)?; //@ctx expr index
                    expr_index.kind = ExprKind::Index { target, index };
                    expr_index.span = Span::new(span_start, self.peek_span_end());
                    target = expr_index;
                }
                Token::KwAs => {
                    self.eat();
                    let mut expr_cast = self.alloc::<Expr>();
                    let mut pty = self.alloc::<Type>();
                    *pty = self.parse_type()?;
                    expr_cast.kind = ExprKind::Cast { target, ty: pty };
                    expr_cast.span = Span::new(span_start, self.peek_span_end());
                    target = expr_cast;
                    last_cast = true;
                }
                _ => return Ok(target),
            }
        }
    }

    fn parse_if(&mut self) -> Result<P<If>, ParseError> {
        let if_ = self.parse_if_branch()?;
        let mut if_prev = if_;
        while self.try_eat(Token::KwElse) {
            match self.peek() {
                Token::KwIf => {
                    let else_if = self.parse_if_branch()?;
                    if_prev.else_ = Some(Else::If { else_if });
                    if_prev = else_if;
                }
                Token::OpenBlock => {
                    let block = self.parse_block()?;
                    if_prev.else_ = Some(Else::Block { block });
                }
                _ => return Err(ParseError::ElseMatch),
            }
        }
        Ok(if_)
    }

    fn parse_if_branch(&mut self) -> Result<P<If>, ParseError> {
        self.eat();
        let mut if_ = self.alloc::<If>();
        if_.cond = self.parse_expr()?;
        if_.block = self.parse_block()?;
        if_.else_ = None;
        Ok(if_)
    }

    fn parse_block(&mut self) -> Result<P<Expr>, ParseError> {
        let span_start = self.peek_span_start();
        let mut expr = self.alloc::<Expr>();
        expr.kind = ExprKind::Block {
            stmts: self.parse_block_stmts()?,
        };
        expr.span = Span::new(span_start, self.peek_span_end());
        Ok(expr)
    }

    fn parse_block_stmts(&mut self) -> Result<List<Stmt>, ParseError> {
        let mut stmts = ListBuilder::new();
        self.expect(Token::OpenBlock, ParseContext::Block)?;
        while !self.try_eat(Token::CloseBlock) {
            let stmt = self.parse_stmt()?;
            stmts.add(&mut self.arena, stmt);
        }
        Ok(stmts.take())
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let pat = self.parse_expr()?;
        self.expect(Token::ArrowWide, ParseContext::MatchArm)?;
        let expr = self.parse_expr()?;
        Ok(MatchArm { pat, expr })
    }

    fn parse_lit(&mut self) -> Result<ExprKind, ParseError> {
        match self.peek() {
            Token::KwNull => {
                self.eat();
                Ok(ExprKind::LitNull)
            }
            Token::KwTrue => {
                self.eat();
                Ok(ExprKind::LitBool { val: true })
            }
            Token::KwFalse => {
                self.eat();
                Ok(ExprKind::LitBool { val: false })
            }
            Token::IntLit => {
                let span = self.peek_span();
                self.eat();
                let str = span.slice(self.source);
                let v = match str.parse::<u64>() {
                    Ok(value) => value,
                    Err(error) => {
                        panic!("parse int error: {}", error.to_string());
                    }
                };
                if let Some(basic) = self.peek().as_basic_type() {
                    match basic {
                        BasicType::S8
                        | BasicType::S16
                        | BasicType::S32
                        | BasicType::S64
                        | BasicType::Ssize
                        | BasicType::U8
                        | BasicType::U16
                        | BasicType::U32
                        | BasicType::U64
                        | BasicType::Usize => {
                            self.eat();
                            Ok(ExprKind::LitUint {
                                val: v,
                                ty: Some(basic),
                            })
                        }
                        BasicType::F32 | BasicType::F64 => {
                            self.eat();
                            //@some values cant be represented
                            Ok(ExprKind::LitFloat {
                                val: v as f64,
                                ty: Some(basic),
                            })
                        }
                        _ => Err(ParseError::LiteralInteger),
                    }
                } else {
                    Ok(ExprKind::LitUint { val: v, ty: None })
                }
            }
            Token::FloatLit => {
                let span = self.peek_span();
                self.eat();
                let str = span.slice(self.source);
                let v = match str.parse::<f64>() {
                    Ok(value) => value,
                    Err(error) => {
                        panic!("parse float error: {}", error.to_string());
                    }
                };
                if let Some(basic) = self.peek().as_basic_type() {
                    match basic {
                        BasicType::F32 | BasicType::F64 => {
                            self.eat();
                            Ok(ExprKind::LitFloat {
                                val: v,
                                ty: Some(basic),
                            })
                        }
                        _ => Err(ParseError::LiteralFloat),
                    }
                } else {
                    Ok(ExprKind::LitFloat { val: v, ty: None })
                }
            }
            Token::CharLit => {
                self.eat();
                let v = self.tokens.char(self.char_id as usize);
                self.char_id += 1;
                Ok(ExprKind::LitChar { val: v })
            }
            Token::StringLit => {
                self.eat();
                let id = self.string_id;
                self.string_id += 1;
                Ok(ExprKind::LitString { id: InternID(id) })
            }
            _ => Err(ParseError::LiteralMatch),
        }
    }

    fn parse_expr_list(
        &mut self,
        end: Token,
        context: ParseContext,
    ) -> Result<List<P<Expr>>, ParseError> {
        let mut expr_list = ListBuilder::new();
        if !self.try_eat(end) {
            loop {
                let expr = self.parse_expr()?;
                expr_list.add(&mut self.arena, expr);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(end, context)?;
        }
        Ok(expr_list.take())
    }

    fn alloc<T: Copy>(&mut self) -> P<T> {
        self.arena.alloc::<T>()
    }

    fn peek(&self) -> Token {
        self.tokens.token(self.cursor)
    }

    fn peek_next(&self) -> Token {
        self.tokens.token(self.cursor + 1)
    }

    fn peek_span(&self) -> Span {
        self.tokens.span(self.cursor)
    }

    fn peek_span_start(&self) -> u32 {
        self.tokens.span(self.cursor).start
    }

    fn peek_span_end(&self) -> u32 {
        self.tokens.span(self.cursor - 1).end
    }

    fn eat(&mut self) {
        self.cursor += 1;
    }

    fn try_eat(&mut self, token: Token) -> bool {
        if token == self.peek() {
            self.eat();
            return true;
        }
        false
    }

    fn try_eat_un_op(&mut self) -> Option<UnOp> {
        match self.peek().as_un_op() {
            Some(mut op) => {
                self.eat();
                if let UnOp::Addr(ref mut mutt) = op {
                    *mutt = self.parse_mut();
                }
                Some(op)
            }
            None => None,
        }
    }

    fn try_eat_basic_type(&mut self) -> Option<BasicType> {
        match self.peek().as_basic_type() {
            Some(op) => {
                self.eat();
                Some(op)
            }
            None => None,
        }
    }

    fn expect(&mut self, token: Token, context: ParseContext) -> Result<(), ParseError> {
        if token == self.peek() {
            self.eat();
            return Ok(());
        }
        Err(ParseError::ExpectToken(context, token))
    }
}

impl BinOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinOp::LogicAnd | BinOp::LogicOr => 1,
            BinOp::CmpLt
            | BinOp::CmpLtEq
            | BinOp::CmpGt
            | BinOp::CmpGtEq
            | BinOp::CmpIsEq
            | BinOp::CmpNotEq => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 4,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => 5,
            BinOp::BitShl | BinOp::BitShr => 6,
        }
    }
}
