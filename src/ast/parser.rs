use super::ast::*;
use super::intern::*;
use super::lexer;
use super::span::*;
use super::token::*;
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
        arena: Arena::new(1024 * 1024),
        modules: Vec::new(),
    };
    timer.elapsed_ms("parse arena created");

    let timer = Timer::new();
    let files = collect_files(&mut ctx);
    timer.elapsed_ms("collect files");

    let timer = Timer::new();
    let handle = &mut std::io::BufWriter::new(std::io::stderr());
    for id in files {
        let lex_res = lexer::lex(&ctx.file(id).source);
        let mut parser = Parser {
            cursor: 0,
            tokens: lex_res.tokens,
            arena: &mut ast.arena,
        };

        let res = parser.parse_module(id);
        ast.modules.push(res.0.copy());
        if let Some(error) = res.1 {
            report::report(handle, &error, &ctx);
        } else {
            //@cloning entire source text, since &File + &mut InternPool violates borrow checker
            let mut interner = ModuleInterner {
                file: ctx.file(id).source.clone(),
                intern_pool: ctx.intern_mut(),
                lex_strings: lex_res.lex_strings,
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
    lex_strings: Vec<String>,
}

impl<'a> visit::MutVisit for ModuleInterner<'a> {
    fn visit_ident(&mut self, ident: &mut Ident) {
        let string = ident.span.slice(&self.file);
        ident.id = self.intern_pool.intern(string);
    }

    fn visit_expr(&mut self, mut expr: P<Expr>) {
        if let ExprKind::Lit(Lit::String(ref mut id)) = expr.kind {
            let string = unsafe { self.lex_strings.get_unchecked(id.0 as usize) };
            *id = self.intern_pool.intern(string);
        }
    }
}

struct Parser<'ast> {
    cursor: usize,
    tokens: Vec<TokenSpan>,
    arena: &'ast mut Arena,
}

impl<'ast> Parser<'ast> {
    fn parse_module(&mut self, file_id: FileID) -> (P<Module>, Option<Error>) {
        let mut module = self.alloc::<Module>();
        module.file_id = file_id;
        module.decls = List::new();

        while self.peek() != Token::Eof {
            match self.parse_decl() {
                Ok(decl) => {
                    module.decls.add(&mut self.arena, decl);
                }
                Err(err) => {
                    let got_token = TokenSpan {
                        span: self.peek_span(),
                        token: self.peek(),
                    };
                    return (module.copy(), Some(Error::parse(err, file_id, got_token)));
                }
            }
        }
        (module, None)
    }

    fn parse_ident(&mut self, context: ParseContext) -> Result<Ident, ParseError> {
        if self.peek() == Token::Ident {
            let span = self.peek_span();
            self.consume();
            return Ok(Ident {
                span,
                id: INTERN_DUMMY_ID,
            });
        }
        Err(ParseError::Ident(context))
    }

    fn parse_vis(&mut self) -> Vis {
        if self.try_consume(Token::KwPub) {
            Vis::Public
        } else {
            Vis::Private
        }
    }

    fn parse_mut(&mut self) -> Mut {
        if self.try_consume(Token::KwMut) {
            Mut::Mutable
        } else {
            Mut::Immutable
        }
    }

    fn parse_path(&mut self) -> Result<Path, ParseError> {
        let kind_span = self.peek_span();
        let kind = match self.peek() {
            Token::KwSuper => PathKind::Super,
            Token::KwPackage => PathKind::Package,
            _ => PathKind::None,
        };
        if kind != PathKind::None {
            self.consume();
            self.expect_token(Token::ColonColon, ParseContext::ModulePath)?;
        }
        let mut names = List::new();
        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = self.parse_ident(ParseContext::ModulePath)?;
            self.consume();
            names.add(&mut self.arena, name);
        }
        Ok(Path {
            kind,
            kind_span,
            names,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut ty = Type {
            ptr: PtrLevel::new(),
            kind: TypeKind::Basic(BasicType::Bool),
        };
        while self.try_consume(Token::Star) {
            let mutt = self.parse_mut();
            if let Err(..) = ty.ptr.add_level(mutt) {
                //@overflown ptr indirection span cannot be captured by current err system
                // silently ignoring this error
            }
        }
        if let Some(basic) = self.try_consume_basic_type() {
            ty.kind = TypeKind::Basic(basic);
            return Ok(ty);
        }
        ty.kind = match self.peek() {
            Token::OpenParen => {
                self.consume();
                self.expect_token(Token::CloseParen, ParseContext::UnitType)?;
                TypeKind::Unit
            }
            Token::Ident | Token::KwSuper | Token::KwPackage => {
                let mut custom_type = self.alloc::<CustomType>();
                custom_type.path = self.parse_path()?;
                custom_type.name = self.parse_ident(ParseContext::CustomType)?;
                TypeKind::Custom(custom_type)
            }
            Token::OpenBracket => match self.peek_next(1) {
                Token::KwMut | Token::CloseBracket => {
                    self.consume();
                    let mut array_slice = self.alloc::<ArraySlice>();
                    array_slice.mutt = self.parse_mut();
                    self.expect_token(Token::CloseBracket, ParseContext::ArraySlice)?;
                    array_slice.ty = self.parse_type()?;
                    TypeKind::ArraySlice(array_slice)
                }
                _ => {
                    self.consume();
                    let mut array_static = self.alloc::<ArrayStatic>();
                    array_static.size = ConstExpr(self.parse_expr()?);
                    self.expect_token(Token::CloseBracket, ParseContext::ArrayStatic)?;
                    array_static.ty = self.parse_type()?;
                    TypeKind::ArrayStatic(array_static)
                }
            },
            _ => return Err(ParseError::TypeMatch),
        };
        Ok(ty)
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
                self.expect_token(Token::ColonColon, ParseContext::Decl)?;
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
        let span_start = self.peek_span_start();
        self.consume(); // `mod`

        let mut module_decl = self.alloc::<ModuleDecl>();
        module_decl.vis = vis;
        module_decl.name = name;
        module_decl.id = None;
        self.expect_token(Token::Semicolon, ParseContext::ModDecl)?;

        module_decl.span = Span::new(span_start, self.peek_span_end());
        Ok(module_decl)
    }

    fn parse_import_decl(&mut self) -> Result<P<ImportDecl>, ParseError> {
        let span_start = self.peek_span_start();
        self.consume(); // `import`

        let mut import_decl = self.alloc::<ImportDecl>();
        import_decl.path = self.parse_path()?;
        import_decl.target = self.parse_import_target()?;
        self.expect_token(Token::Semicolon, ParseContext::ImportDecl)?;

        import_decl.span = Span::new(span_start, self.peek_span_end());
        Ok(import_decl)
    }

    fn parse_import_target(&mut self) -> Result<ImportTarget, ParseError> {
        match self.peek() {
            Token::Star => {
                self.consume();
                Ok(ImportTarget::GlobAll)
            }
            Token::Ident => {
                let name = self.parse_ident(ParseContext::ImportDecl)?;
                Ok(ImportTarget::Symbol(name))
            }
            Token::OpenBlock => {
                self.consume();
                let mut symbols = List::<Ident>::new();
                if !self.try_consume(Token::CloseBlock) {
                    loop {
                        let name = self.parse_ident(ParseContext::ImportDecl)?;
                        symbols.add(&mut self.arena, name);
                        if !self.try_consume(Token::Comma) {
                            break;
                        }
                    }
                    self.expect_token(Token::CloseBlock, ParseContext::ImportDecl)?;
                }
                Ok(ImportTarget::SymbolList(symbols))
            }
            _ => Err(ParseError::ImportTargetMatch),
        }
    }

    fn parse_global_decl(&mut self, vis: Vis, name: Ident) -> Result<P<GlobalDecl>, ParseError> {
        self.consume(); // `:`
        let mut global_decl = self.alloc::<GlobalDecl>();
        global_decl.vis = vis;
        global_decl.name = name;

        if self.try_consume(Token::Assign) {
            global_decl.ty = None;
            global_decl.value = ConstExpr(self.parse_expr()?);
        } else {
            global_decl.ty = Some(self.parse_type()?);
            self.expect_token(Token::Assign, ParseContext::GlobalDecl)?;
            global_decl.value = ConstExpr(self.parse_expr()?);
        }
        self.expect_token(Token::Semicolon, ParseContext::GlobalDecl)?;
        Ok(global_decl)
    }

    fn parse_proc_decl(&mut self, vis: Vis, name: Ident) -> Result<P<ProcDecl>, ParseError> {
        self.consume(); // `(`
        let mut proc_decl = self.alloc::<ProcDecl>();
        proc_decl.vis = vis;
        proc_decl.name = name;

        if !self.try_consume(Token::CloseParen) {
            loop {
                if self.try_consume(Token::DotDot) {
                    proc_decl.is_variadic = true;
                    break;
                }
                let param = self.parse_proc_param()?;
                proc_decl.params.add(&mut self.arena, param);
                if !self.try_consume(Token::Comma) {
                    break;
                }
            }
            self.expect_token(Token::CloseParen, ParseContext::ProcDecl)?;
        }

        proc_decl.return_ty = if self.try_consume(Token::ArrowThin) {
            Some(self.parse_type()?)
        } else {
            None
        };
        proc_decl.block = if !self.try_consume(Token::DirCCall) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(proc_decl)
    }

    fn parse_proc_param(&mut self) -> Result<ProcParam, ParseError> {
        let mutt = self.parse_mut();
        let name = self.parse_ident(ParseContext::ProcParam)?;
        self.expect_token(Token::Colon, ParseContext::ProcParam)?;
        let ty = self.parse_type()?;
        Ok(ProcParam { mutt, name, ty })
    }

    fn parse_enum_decl(&mut self, vis: Vis, name: Ident) -> Result<P<EnumDecl>, ParseError> {
        self.consume(); // `enum`
        let mut enum_decl = self.alloc::<EnumDecl>();
        enum_decl.vis = vis;
        enum_decl.name = name;

        enum_decl.basic_ty = self.try_consume_basic_type();
        self.expect_token(Token::OpenBlock, ParseContext::EnumDecl)?;
        while !self.try_consume(Token::CloseBlock) {
            let variant = self.parse_enum_variant()?;
            enum_decl.variants.add(&mut self.arena, variant);
        }
        Ok(enum_decl)
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, ParseError> {
        let name = self.parse_ident(ParseContext::EnumVariant)?;
        let value = if self.try_consume(Token::Assign) {
            Some(ConstExpr(self.parse_expr()?))
        } else {
            None
        };
        self.expect_token(Token::Semicolon, ParseContext::EnumVariant)?;
        Ok(EnumVariant { name, value })
    }

    fn parse_union_decl(&mut self, vis: Vis, name: Ident) -> Result<P<UnionDecl>, ParseError> {
        self.consume(); // `union`
        let mut union_decl = self.alloc::<UnionDecl>();
        union_decl.vis = vis;
        union_decl.name = name;

        self.expect_token(Token::OpenBlock, ParseContext::UnionDecl)?;
        while !self.try_consume(Token::CloseBlock) {
            let member = self.parse_union_member()?;
            union_decl.members.add(&mut self.arena, member);
        }
        Ok(union_decl)
    }

    fn parse_union_member(&mut self) -> Result<UnionMember, ParseError> {
        let name = self.parse_ident(ParseContext::UnionMember)?;
        self.expect_token(Token::Colon, ParseContext::UnionMember)?;
        let ty = self.parse_type()?;
        self.expect_token(Token::Semicolon, ParseContext::UnionMember)?;
        Ok(UnionMember { name, ty })
    }

    fn parse_struct_decl(&mut self, vis: Vis, name: Ident) -> Result<P<StructDecl>, ParseError> {
        self.consume(); // `struct`
        let mut struct_decl = self.alloc::<StructDecl>();
        struct_decl.vis = vis;
        struct_decl.name = name;

        self.expect_token(Token::OpenBlock, ParseContext::StructDecl)?;
        while !self.try_consume(Token::CloseBlock) {
            let field = self.parse_struct_field()?;
            struct_decl.fields.add(&mut self.arena, field);
        }
        Ok(struct_decl)
    }

    fn parse_struct_field(&mut self) -> Result<StructField, ParseError> {
        let name = self.parse_ident(ParseContext::StructField)?;
        self.expect_token(Token::Colon, ParseContext::StructField)?;
        let ty = self.parse_type()?;
        self.expect_token(Token::Semicolon, ParseContext::StructField)?;
        Ok(StructField { name, ty })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let span_start = self.peek_span_start();
        let kind = match self.peek() {
            Token::KwBreak => {
                self.consume();
                self.expect_token(Token::Semicolon, ParseContext::Break)?;
                StmtKind::Break
            }
            Token::KwContinue => {
                self.consume();
                self.expect_token(Token::Semicolon, ParseContext::Continue)?;
                StmtKind::Continue
            }
            Token::KwFor => {
                self.consume();
                StmtKind::For(self.parse_for()?)
            }
            Token::KwDefer => {
                self.consume();
                StmtKind::Defer(self.parse_block()?)
            }
            Token::KwReturn => {
                self.consume();
                let mut return_ = self.alloc::<Return>();
                return_.expr = if self.peek() != Token::Semicolon {
                    Some(self.parse_expr()?)
                } else {
                    None
                };
                self.expect_token(Token::Semicolon, ParseContext::Return)?;
                StmtKind::Return(return_)
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
        //@todo parse c like loop
        for_.block = self.parse_block()?;
        Ok(for_)
    }

    fn parse_stmt_no_keyword(&mut self) -> Result<StmtKind, ParseError> {
        let expect_var_bind = (self.peek() == Token::KwMut)
            || ((self.peek() == Token::Ident || self.peek() == Token::Underscore)
                && self.peek_next(1) == Token::Colon);
        if expect_var_bind {
            Ok(StmtKind::VarDecl(self.parse_var_decl()?))
        } else {
            let expr = self.parse_expr()?;
            match self.peek().as_assign_op() {
                Some(op) => {
                    self.consume();
                    let mut var_assign = self.alloc::<VarAssign>();
                    var_assign.op = op;
                    var_assign.lhs = expr;
                    var_assign.rhs = self.parse_expr()?;
                    self.expect_token(Token::Semicolon, ParseContext::VarAssign)?;
                    Ok(StmtKind::VarAssign(var_assign))
                }
                None => {
                    let has_semi = self.try_consume(Token::Semicolon);
                    let mut expr_stmt = self.alloc::<ExprStmt>();
                    *expr_stmt = ExprStmt { expr, has_semi };
                    Ok(StmtKind::ExprStmt(expr_stmt))
                }
            }
        }
    }

    fn parse_var_decl(&mut self) -> Result<P<VarDecl>, ParseError> {
        let mut var_decl = self.alloc::<VarDecl>();
        var_decl.mutt = self.parse_mut();
        var_decl.name = if self.try_consume(Token::Underscore) {
            None
        } else {
            Some(self.parse_ident(ParseContext::VarDecl)?)
        };
        self.expect_token(Token::Colon, ParseContext::VarDecl)?;

        if self.try_consume(Token::Assign) {
            var_decl.ty = None;
            var_decl.expr = Some(self.parse_expr()?);
        } else {
            var_decl.ty = Some(self.parse_type()?);
            var_decl.expr = if self.try_consume(Token::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            }
        }
        self.expect_token(Token::Semicolon, ParseContext::VarDecl)?;
        Ok(var_decl)
    }

    fn parse_expr(&mut self) -> Result<P<Expr>, ParseError> {
        self.parse_sub_expr(0)
    }

    fn parse_sub_expr(&mut self, min_prec: u32) -> Result<P<Expr>, ParseError> {
        let mut expr_lhs = self.parse_primary_expr()?;
        loop {
            let prec: u32;
            let binary_op: BinaryOp;
            if let Some(op) = self.peek().as_binary_op() {
                binary_op = op;
                prec = op.prec();
                if prec < min_prec {
                    break;
                }
                match binary_op {
                    BinaryOp::Deref | BinaryOp::Index => {}
                    _ => self.consume(),
                }
            } else {
                break;
            }
            let mut expr = self.alloc::<Expr>();

            let mut bin_expr = self.alloc::<BinaryExpr>();
            bin_expr.op = binary_op;
            bin_expr.lhs = expr_lhs;
            bin_expr.rhs = match binary_op {
                BinaryOp::Deref | BinaryOp::Index => self.parse_tail_expr(binary_op)?,
                _ => self.parse_sub_expr(prec + 1)?,
            };

            expr.span = Span::new(bin_expr.lhs.span.start, bin_expr.rhs.span.end);
            expr.kind = ExprKind::BinaryExpr(bin_expr);
            expr_lhs = expr;
        }
        Ok(expr_lhs)
    }

    fn parse_tail_expr(&mut self, op: BinaryOp) -> Result<P<Expr>, ParseError> {
        let mut expr = self.alloc::<Expr>();
        let span_start = self.peek_span_start();
        self.consume();

        expr.kind = match op {
            BinaryOp::Deref => {
                let name = self.parse_ident(ParseContext::Expr)?; //@context
                ExprKind::DotName(name)
            }
            BinaryOp::Index => {
                let mut index = self.alloc::<Index>();
                index.expr = self.parse_expr()?;
                self.expect_token(Token::CloseBracket, ParseContext::Expr)?; //@context
                ExprKind::Index(index)
            }
            _ => return Err(ParseError::PrimaryExprMatch), //@temp error
        };

        expr.span = Span::new(span_start, self.peek_span_end());
        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> Result<P<Expr>, ParseError> {
        let span_start = self.peek_span_start();

        if self.try_consume(Token::OpenParen) {
            if self.try_consume(Token::CloseParen) {
                let mut expr = self.alloc::<Expr>();
                expr.kind = ExprKind::Unit;
                expr.span = Span::new(span_start, self.peek_span_end());
                return Ok(expr);
            }
            let expr = self.parse_sub_expr(0)?;
            self.expect_token(Token::CloseParen, ParseContext::Expr)?;
            return Ok(expr);
        }

        let mut expr = self.alloc::<Expr>();

        if let Some(unary_op) = self.try_consume_unary_op() {
            let mut unary_expr = self.alloc::<UnaryExpr>();
            unary_expr.op = unary_op;
            unary_expr.rhs = self.parse_primary_expr()?;

            expr.span = Span::new(span_start, self.peek_span_end());
            expr.kind = ExprKind::UnaryExpr(unary_expr);
            return Ok(expr);
        }

        expr.kind = match self.peek() {
            Token::Underscore => {
                self.consume();
                ExprKind::Discard
            }
            Token::KwIf => ExprKind::If(self.parse_if()?),
            Token::LitNull
            | Token::LitBool(..)
            | Token::LitInt(..)
            | Token::LitFloat(..)
            | Token::LitChar(..)
            | Token::LitString(..) => ExprKind::Lit(self.parse_lit()?),
            Token::OpenBlock => ExprKind::Block(self.parse_block()?),
            Token::KwMatch => ExprKind::Match(self.parse_match()?),
            Token::KwCast => ExprKind::Cast(self.parse_cast()?),
            Token::KwSizeof => ExprKind::Sizeof(self.parse_sizeof()?),
            Token::OpenBracket => {
                self.consume(); // `[`
                if self.try_consume(Token::CloseBracket) {
                    let mut array_init = self.alloc::<ArrayInit>();
                    array_init.input = List::new();
                    ExprKind::ArrayInit(array_init)
                } else {
                    let expr = self.parse_expr()?;
                    if self.try_consume(Token::Semicolon) {
                        let mut array_repeat = self.alloc::<ArrayRepeat>();
                        array_repeat.expr = expr;
                        array_repeat.size = ConstExpr(self.parse_expr()?);
                        self.expect_token(Token::CloseBracket, ParseContext::ArrayInit)?;
                        ExprKind::ArrayRepeat(array_repeat)
                    } else {
                        let mut array_init = self.alloc::<ArrayInit>();
                        array_init.input = List::new();
                        array_init.input.add(&mut self.arena, expr);
                        if !self.try_consume(Token::CloseBracket) {
                            self.expect_token(Token::Comma, ParseContext::ArrayInit)?;
                            loop {
                                let expr = self.parse_expr()?;
                                array_init.input.add(&mut self.arena, expr);
                                if !self.try_consume(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect_token(Token::CloseBracket, ParseContext::ArrayInit)?;
                        }
                        ExprKind::ArrayInit(array_init)
                    }
                }
            }
            _ => {
                let path = self.parse_path()?;
                let name = self.parse_ident(ParseContext::Expr)?;

                match (self.peek(), self.peek_next(1)) {
                    (Token::OpenParen, ..) => {
                        self.consume(); // `(`
                        let mut proc_call = self.alloc::<ProcCall>();
                        proc_call.path = path;
                        proc_call.name = name;
                        proc_call.input =
                            self.parse_expr_list(Token::CloseParen, ParseContext::ProcCall)?;
                        ExprKind::ProcCall(proc_call)
                    }
                    (Token::Dot, Token::OpenBlock) => {
                        self.consume(); // `.`
                        self.consume(); // `{`
                        let mut struct_init = self.alloc::<StructInit>();
                        struct_init.path = path;
                        struct_init.name = name;
                        struct_init.input = List::<FieldInit>::new();
                        if !self.try_consume(Token::CloseBlock) {
                            loop {
                                let name = self.parse_ident(ParseContext::StructInit)?;
                                let expr = match self.peek() {
                                    Token::Colon => {
                                        self.consume();
                                        Some(self.parse_expr()?)
                                    }
                                    Token::Comma | Token::CloseBlock => None,
                                    _ => return Err(ParseError::FieldInit),
                                };
                                struct_init
                                    .input
                                    .add(&mut self.arena, FieldInit { name, expr });
                                if !self.try_consume(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect_token(Token::CloseBlock, ParseContext::StructInit)?;
                        }
                        ExprKind::StructInit(struct_init)
                    }
                    _ => {
                        let mut item = self.alloc::<Item>();
                        item.path = path;
                        item.name = name;
                        ExprKind::Item(item)
                    }
                }
            }
        };
        expr.span = Span::new(span_start, self.peek_span_end());
        Ok(expr)
    }

    fn parse_if(&mut self) -> Result<P<If>, ParseError> {
        let if_ = self.parse_if_branch()?;
        let mut if_prev = if_;
        while self.try_consume(Token::KwElse) {
            match self.peek() {
                Token::KwIf => {
                    let else_if = self.parse_if_branch()?;
                    if_prev.else_ = Some(Else::If(else_if));
                    if_prev = else_if;
                }
                Token::OpenBlock => {
                    let block = self.parse_block()?;
                    if_prev.else_ = Some(Else::Block(block));
                }
                _ => return Err(ParseError::ElseMatch),
            }
        }
        Ok(if_)
    }

    fn parse_if_branch(&mut self) -> Result<P<If>, ParseError> {
        self.consume();
        let mut if_ = self.alloc::<If>();
        if_.cond = self.parse_expr()?;
        if_.block = self.parse_block()?;
        if_.else_ = None;
        Ok(if_)
    }

    fn parse_block(&mut self) -> Result<P<Block>, ParseError> {
        let mut block = self.alloc::<Block>();
        self.expect_token(Token::OpenBlock, ParseContext::Block)?;
        while !self.try_consume(Token::CloseBlock) {
            let stmt = self.parse_stmt()?;
            block.stmts.add(&mut self.arena, stmt);
        }
        Ok(block)
    }

    fn parse_match(&mut self) -> Result<P<Match>, ParseError> {
        self.consume(); // `match`
        let mut match_ = self.alloc::<Match>();
        match_.expr = self.parse_expr()?;
        self.expect_token(Token::OpenBlock, ParseContext::Match)?;
        while !self.try_consume(Token::CloseBlock) {
            let arm = self.parse_match_arm()?;
            match_.arms.add(&mut self.arena, arm);
        }
        Ok(match_)
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let pat = self.parse_expr()?;
        self.expect_token(Token::ArrowWide, ParseContext::MatchArm)?;
        let expr = self.parse_expr()?;
        Ok(MatchArm { pat, expr })
    }

    fn parse_cast(&mut self) -> Result<P<Cast>, ParseError> {
        self.consume(); // `cast`
        let mut cast = self.alloc::<Cast>();
        self.expect_token(Token::OpenParen, ParseContext::Cast)?;
        cast.ty = self.parse_type()?;
        self.expect_token(Token::Comma, ParseContext::Cast)?;
        cast.expr = self.parse_expr()?;
        self.expect_token(Token::CloseParen, ParseContext::Cast)?;
        Ok(cast)
    }

    fn parse_sizeof(&mut self) -> Result<P<Sizeof>, ParseError> {
        self.consume(); // `sizeof`
        let mut sizeof = self.alloc::<Sizeof>();
        self.expect_token(Token::OpenParen, ParseContext::Sizeof)?;
        sizeof.ty = self.parse_type()?;
        self.expect_token(Token::CloseParen, ParseContext::Sizeof)?;
        Ok(sizeof)
    }

    fn parse_lit(&mut self) -> Result<Lit, ParseError> {
        match self.peek() {
            Token::LitNull => {
                self.consume();
                Ok(Lit::Null)
            }
            Token::LitBool(v) => {
                self.consume();
                Ok(Lit::Bool(v))
            }
            Token::LitInt(v) => {
                self.consume();
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
                            self.consume();
                            Ok(Lit::Uint(v, Some(basic)))
                        }
                        BasicType::F32 | BasicType::F64 => {
                            self.consume();
                            //@some values cant be represented
                            Ok(Lit::Float(v as f64, Some(basic)))
                        }
                        _ => Err(ParseError::LiteralInteger),
                    }
                } else {
                    Ok(Lit::Uint(v, None))
                }
            }
            Token::LitFloat(v) => {
                self.consume();
                if let Some(basic) = self.peek().as_basic_type() {
                    match basic {
                        BasicType::F32 | BasicType::F64 => {
                            self.consume();
                            Ok(Lit::Float(v, Some(basic)))
                        }
                        _ => Err(ParseError::LiteralFloat),
                    }
                } else {
                    Ok(Lit::Float(v, None))
                }
            }
            Token::LitChar(v) => {
                self.consume();
                Ok(Lit::Char(v))
            }
            Token::LitString(id) => {
                self.consume();
                Ok(Lit::String(InternID(id)))
            }
            _ => Err(ParseError::LiteralMatch),
        }
    }

    fn parse_expr_list(
        &mut self,
        end: Token,
        context: ParseContext,
    ) -> Result<List<P<Expr>>, ParseError> {
        let mut expr_list = List::new();
        if !self.try_consume(end) {
            loop {
                let expr = self.parse_expr()?;
                expr_list.add(&mut self.arena, expr);
                if !self.try_consume(Token::Comma) {
                    break;
                }
            }
            self.expect_token(end, context)?;
        }
        Ok(expr_list)
    }

    fn alloc<T: Copy>(&mut self) -> P<T> {
        self.arena.alloc::<T>()
    }

    fn peek(&self) -> Token {
        unsafe { self.tokens.get_unchecked(self.cursor).token }
    }

    fn peek_next(&self, offset: isize) -> Token {
        unsafe {
            self.tokens
                .get_unchecked(self.cursor + offset as usize)
                .token
        }
    }

    fn peek_span(&self) -> Span {
        unsafe { self.tokens.get_unchecked(self.cursor).span }
    }

    fn peek_span_start(&self) -> u32 {
        unsafe { self.tokens.get_unchecked(self.cursor).span.start }
    }

    fn peek_span_end(&self) -> u32 {
        unsafe { self.tokens.get_unchecked(self.cursor - 1).span.end }
    }

    fn consume(&mut self) {
        self.cursor += 1;
    }

    fn try_consume(&mut self, token: Token) -> bool {
        if token == self.peek() {
            self.consume();
            return true;
        }
        false
    }

    fn try_consume_unary_op(&mut self) -> Option<UnaryOp> {
        match self.peek().as_unary_op() {
            Some(mut op) => {
                self.consume();
                match &mut op {
                    UnaryOp::Addr(mutt) => {
                        *mutt = self.parse_mut();
                    }
                    _ => {}
                }
                Some(op)
            }
            None => None,
        }
    }

    fn try_consume_basic_type(&mut self) -> Option<BasicType> {
        match self.peek().as_basic_type() {
            Some(op) => {
                self.consume();
                Some(op)
            }
            None => None,
        }
    }

    fn expect_token(&mut self, token: Token, context: ParseContext) -> Result<(), ParseError> {
        if token == self.peek() {
            self.consume();
            return Ok(());
        }
        Err(ParseError::ExpectToken(context, token))
    }
}

impl BinaryOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinaryOp::LogicAnd | BinaryOp::LogicOr => 1,
            BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessEq
            | BinaryOp::GreaterEq
            | BinaryOp::IsEq
            | BinaryOp::NotEq => 2,
            BinaryOp::Plus | BinaryOp::Sub => 3,
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 4,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => 5,
            BinaryOp::Shl | BinaryOp::Shr => 6,
            BinaryOp::Deref | BinaryOp::Index => 7,
        }
    }
}

impl Token {
    fn as_unary_op(&self) -> Option<UnaryOp> {
        match self {
            Token::Minus => Some(UnaryOp::Neg),
            Token::BitNot => Some(UnaryOp::BitNot),
            Token::LogicNot => Some(UnaryOp::LogicNot),
            Token::Star => Some(UnaryOp::Addr(Mut::Immutable)),
            Token::Shl => Some(UnaryOp::Deref),
            _ => None,
        }
    }

    fn as_binary_op(&self) -> Option<BinaryOp> {
        match self {
            Token::Dot => Some(BinaryOp::Deref),
            Token::OpenBracket => Some(BinaryOp::Index),
            Token::LogicAnd => Some(BinaryOp::LogicAnd),
            Token::LogicOr => Some(BinaryOp::LogicOr),
            Token::Less => Some(BinaryOp::Less),
            Token::Greater => Some(BinaryOp::Greater),
            Token::LessEq => Some(BinaryOp::LessEq),
            Token::GreaterEq => Some(BinaryOp::GreaterEq),
            Token::IsEq => Some(BinaryOp::IsEq),
            Token::NotEq => Some(BinaryOp::NotEq),
            Token::Plus => Some(BinaryOp::Plus),
            Token::Minus => Some(BinaryOp::Sub),
            Token::Star => Some(BinaryOp::Mul),
            Token::Div => Some(BinaryOp::Div),
            Token::Mod => Some(BinaryOp::Rem),
            Token::BitAnd => Some(BinaryOp::BitAnd),
            Token::BitOr => Some(BinaryOp::BitOr),
            Token::BitXor => Some(BinaryOp::BitXor),
            Token::Shl => Some(BinaryOp::Shl),
            Token::Shr => Some(BinaryOp::Shr),
            _ => None,
        }
    }

    fn as_assign_op(&self) -> Option<AssignOp> {
        match self {
            Token::Assign => Some(AssignOp::Assign),
            Token::PlusEq => Some(AssignOp::BinaryOp(BinaryOp::Plus)),
            Token::MinusEq => Some(AssignOp::BinaryOp(BinaryOp::Sub)),
            Token::TimesEq => Some(AssignOp::BinaryOp(BinaryOp::Mul)),
            Token::DivEq => Some(AssignOp::BinaryOp(BinaryOp::Div)),
            Token::ModEq => Some(AssignOp::BinaryOp(BinaryOp::Rem)),
            Token::BitAndEq => Some(AssignOp::BinaryOp(BinaryOp::BitAnd)),
            Token::BitOrEq => Some(AssignOp::BinaryOp(BinaryOp::BitOr)),
            Token::BitXorEq => Some(AssignOp::BinaryOp(BinaryOp::BitXor)),
            Token::ShlEq => Some(AssignOp::BinaryOp(BinaryOp::Shl)),
            Token::ShrEq => Some(AssignOp::BinaryOp(BinaryOp::Shr)),
            _ => None,
        }
    }

    fn as_basic_type(&self) -> Option<BasicType> {
        match self {
            Token::KwBool => Some(BasicType::Bool),
            Token::KwS8 => Some(BasicType::S8),
            Token::KwS16 => Some(BasicType::S16),
            Token::KwS32 => Some(BasicType::S32),
            Token::KwS64 => Some(BasicType::S64),
            Token::KwSsize => Some(BasicType::Ssize),
            Token::KwU8 => Some(BasicType::U8),
            Token::KwU16 => Some(BasicType::U16),
            Token::KwU32 => Some(BasicType::U32),
            Token::KwU64 => Some(BasicType::U64),
            Token::KwUsize => Some(BasicType::Usize),
            Token::KwF32 => Some(BasicType::F32),
            Token::KwF64 => Some(BasicType::F64),
            Token::KwChar => Some(BasicType::Char),
            Token::KwRawptr => Some(BasicType::Rawptr),
            _ => None,
        }
    }
}
