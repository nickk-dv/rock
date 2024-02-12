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
        arena: Arena::new(1024 * 1024),
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
        ast.modules.push(res.0.copy());
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
        if let ExprKind::Lit(Lit::String(ref mut id)) = expr.kind {
            let string = unsafe { self.tokens.string(id.0 as usize) };
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
        module.file_id = file_id;
        module.decls = List::new();

        while self.peek() != Token::Eof {
            match self.parse_decl() {
                Ok(decl) => {
                    module.decls.add(&mut self.arena, decl);
                }
                Err(err) => {
                    let got_token = (self.peek(), self.peek_span());
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
        let span_start = self.peek_span_start();
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
            names,
            span_start,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut ty = Type {
            ptr: PtrLevel::new(),
            kind: TypeKind::Basic(BasicType::Unit),
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
                TypeKind::Basic(BasicType::Unit)
            }
            Token::Ident | Token::KwSuper | Token::KwPackage => {
                TypeKind::Custom(self.parse_item_name()?)
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

    fn parse_item_name(&mut self) -> Result<P<ItemName>, ParseError> {
        let mut item_name = self.alloc::<ItemName>();
        item_name.path = self.parse_path()?;
        item_name.name = self.parse_ident(ParseContext::CustomType)?; //@take specific ctx instead?
        Ok(item_name)
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

        if self.try_consume(Token::Equals) {
            global_decl.ty = None;
            global_decl.value = ConstExpr(self.parse_expr()?);
        } else {
            global_decl.ty = Some(self.parse_type()?);
            self.expect_token(Token::Equals, ParseContext::GlobalDecl)?;
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
        let value = if self.try_consume(Token::Equals) {
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
        let vis = self.parse_vis();
        let name = self.parse_ident(ParseContext::StructField)?;
        self.expect_token(Token::Colon, ParseContext::StructField)?;
        let ty = self.parse_type()?;
        self.expect_token(Token::Semicolon, ParseContext::StructField)?;
        Ok(StructField { vis, name, ty })
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
                if self.try_consume(Token::Semicolon) {
                    StmtKind::Return(None)
                } else {
                    let expr = self.parse_expr()?;
                    self.expect_token(Token::Semicolon, ParseContext::Return)?;
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
                    if self.try_consume(Token::Semicolon) {
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
        var_decl.name = if self.try_consume(Token::Underscore) {
            None
        } else {
            Some(self.parse_ident(ParseContext::VarDecl)?)
        };
        self.expect_token(Token::Colon, ParseContext::VarDecl)?;

        if self.try_consume(Token::Equals) {
            var_decl.ty = None;
            var_decl.expr = Some(self.parse_expr()?);
        } else {
            var_decl.ty = Some(self.parse_type()?);
            var_decl.expr = if self.try_consume(Token::Equals) {
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
            let binary_op: BinOp;
            if let Some(op) = self.peek().as_bin_op() {
                binary_op = op;
                prec = op.prec();
                if prec < min_prec {
                    break;
                }
                self.consume();
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
            let op = unary_op;
            let rhs = self.parse_primary_expr()?;

            expr.span = Span::new(span_start, self.peek_span_end());
            expr.kind = ExprKind::UnaryExpr { op, rhs };
            return Ok(expr);
        }

        expr.kind = match self.peek() {
            Token::Underscore => {
                self.consume();
                ExprKind::Discard
            }
            Token::KwIf => ExprKind::If(self.parse_if()?),
            Token::KwNull
            | Token::KwTrue
            | Token::KwFalse
            | Token::IntLit
            | Token::FloatLit
            | Token::CharLit
            | Token::StringLit => ExprKind::Lit(self.parse_lit()?),
            Token::OpenBlock => ExprKind::Block(self.parse_block()?),
            Token::KwMatch => {
                self.consume();
                let expr = self.parse_expr()?;
                let mut arms = List::new();
                self.expect_token(Token::OpenBlock, ParseContext::Match)?;
                while !self.try_consume(Token::CloseBlock) {
                    let arm = self.parse_match_arm()?;
                    arms.add(&mut self.arena, arm);
                }
                ExprKind::Match { expr, arms }
            }
            Token::KwAs => ExprKind::Cast(self.parse_cast()?),
            Token::KwSizeof => {
                self.consume();
                self.expect_token(Token::OpenParen, ParseContext::Sizeof)?;
                let ty = self.parse_type()?;
                self.expect_token(Token::CloseParen, ParseContext::Sizeof)?;
                ExprKind::Sizeof { ty }
            }
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
                let item = self.parse_item_name()?;

                match (self.peek(), self.peek_next(1)) {
                    (Token::OpenParen, ..) => {
                        self.consume(); // `(`
                        let input =
                            self.parse_expr_list(Token::CloseParen, ParseContext::ProcCall)?;
                        ExprKind::ProcCall { item, input }
                    }
                    (Token::Dot, Token::OpenBlock) => {
                        self.consume(); // `.`
                        self.consume(); // `{`
                        let mut input = List::<FieldInit>::new();
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
                                input.add(&mut self.arena, FieldInit { name, expr });
                                if !self.try_consume(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect_token(Token::CloseBlock, ParseContext::StructInit)?;
                        }
                        ExprKind::StructInit { item, input }
                    }
                    _ => ExprKind::Item { item },
                }
            }
        };
        expr.span = Span::new(span_start, self.peek_span_end());

        let mut target = expr;
        loop {
            match self.peek() {
                Token::Dot => {
                    self.consume();
                    let span_start = self.peek_span_start();
                    let mut expr_field = self.alloc::<Expr>();
                    let name = self.parse_ident(ParseContext::Expr)?; //@ctx expr field
                    expr_field.kind = ExprKind::DotName { target, name };
                    expr_field.span = Span::new(span_start, self.peek_span_end());
                    target = expr_field;
                }
                Token::OpenBracket => {
                    self.consume();
                    let span_start = self.peek_span_start();
                    let mut expr_index = self.alloc::<Expr>();
                    let index = self.parse_expr()?;
                    self.expect_token(Token::CloseBracket, ParseContext::Expr)?; //@ctx expr index
                    expr_index.kind = ExprKind::Index { target, index };
                    expr_index.span = Span::new(span_start, self.peek_span_end());
                    target = expr_index;
                }
                _ => return Ok(target),
            }
        }
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
            Token::KwNull => {
                self.consume();
                Ok(Lit::Null)
            }
            Token::KwTrue => {
                self.consume();
                Ok(Lit::Bool(true))
            }
            Token::KwFalse => {
                self.consume();
                Ok(Lit::Bool(false))
            }
            Token::IntLit => {
                let span = self.peek_span();
                self.consume();
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
            Token::FloatLit => {
                let span = self.peek_span();
                self.consume();
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
                            self.consume();
                            Ok(Lit::Float(v, Some(basic)))
                        }
                        _ => Err(ParseError::LiteralFloat),
                    }
                } else {
                    Ok(Lit::Float(v, None))
                }
            }
            Token::CharLit => {
                self.consume();
                let v = self.tokens.char(self.char_id as usize);
                self.char_id += 1;
                Ok(Lit::Char(v))
            }
            Token::StringLit => {
                self.consume();
                let id = self.string_id;
                self.string_id += 1;
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
        unsafe { self.tokens.token(self.cursor) }
    }

    fn peek_next(&self, offset: isize) -> Token {
        unsafe { self.tokens.token(self.cursor + offset as usize) }
    }

    fn peek_span(&self) -> Span {
        unsafe { self.tokens.span(self.cursor) }
    }

    fn peek_span_start(&self) -> u32 {
        unsafe { self.tokens.span(self.cursor).start }
    }

    fn peek_span_end(&self) -> u32 {
        unsafe { self.tokens.span(self.cursor - 1).end }
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

    fn try_consume_unary_op(&mut self) -> Option<UnOp> {
        match self.peek().as_un_op() {
            Some(mut op) => {
                self.consume();
                //@not storing mutability
                let mutt = self.parse_mut();
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

impl BinOp {
    pub fn prec(&self) -> u32 {
        match self {
            BinOp::LogicAnd | BinOp::LogicOr => 1,
            BinOp::Less
            | BinOp::Greater
            | BinOp::LessEq
            | BinOp::GreaterEq
            | BinOp::IsEq
            | BinOp::NotEq => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 4,
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor => 5,
            BinOp::Shl | BinOp::Shr => 6,
        }
    }
}
