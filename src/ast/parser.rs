use super::ast::*;
use super::lexer;
use super::span::*;
use super::token::*;
use super::visit;
use crate::err::error::*;
use crate::err::report;
use crate::mem::*;
use crate::tools::threads;
use std::path::PathBuf;
use std::time::Instant;

//@empty error tokens produce invalid span diagnostic

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
        println!("{}: {:.3} ms", message, ms);
    }
}

pub fn parse() -> Result<P<Ast>, ()> {
    let parse_timer = Timer::new();

    let mut arena = Arena::new(4096);
    let mut ast = arena.alloc::<Ast>();
    ast.arenas = Vec::new();
    ast.modules = Vec::new();
    ast.intern_pool = arena.alloc();
    *ast.intern_pool = InternPool::new();
    ast.arenas.push(arena);

    let mut filepaths = Vec::new();
    let mut dir_path = PathBuf::new();
    dir_path.push("test");
    collect_filepaths(dir_path, &mut filepaths);

    let thread_pool = threads::ThreadPool::new(parse_task, task_res);
    let output = thread_pool.execute(filepaths);

    for res in output.1 {
        ast.arenas.push(res);
    }

    for res in output.0 {
        match res {
            Ok(result) => {
                ast.modules.push(result.0);
                if let Some(ref error) = result.1 {
                    report::report(error);
                }
            }
            Err(ref error) => {
                report::report(error);
            }
        }
    }
    parse_timer.elapsed_ms("parsed all files");

    let intern_timer = Timer::new();
    let mut interner = Interner {
        module: P::null(),
        intern_pool: P::null(),
    };
    super::visit::visit_with(&mut interner, ast.copy());
    intern_timer.elapsed_ms("intern idents");

    report::err_status(ast)
}

struct Interner {
    module: P<Module>,
    intern_pool: P<InternPool>,
}

impl visit::MutVisit for Interner {
    fn visit_ast(&mut self, ast: P<Ast>) {
        self.intern_pool = ast.intern_pool.copy();
    }

    fn visit_module(&mut self, module: P<Module>) {
        self.module = module;
    }

    fn visit_ident(&mut self, ident: &mut Ident) {
        let bytes = ident.span.str(&self.module.file.source).as_bytes();
        ident.id = self.intern_pool.intern(bytes);
    }
}

//@change ext
fn collect_filepaths(dir_path: PathBuf, filepaths: &mut Vec<PathBuf>) {
    match std::fs::read_dir(&dir_path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(extension) = path.extension() {
                        if extension == "lang" {
                            filepaths.push(path);
                        }
                    }
                } else if path.is_dir() {
                    collect_filepaths(path, filepaths);
                }
            }
        }
        Err(err) => {
            report::report(
                &Error::file_io(FileIOError::DirRead)
                    .info(err.to_string())
                    .info(format!("path: {:?}", dir_path))
                    .into(),
            );
        }
    }
}

fn parse_task(
    path: PathBuf,
    _: threads::TaskID,
    arena: &mut Arena,
) -> Result<(P<Module>, Option<Error>), Error> {
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            return Err(Error::file_io(FileIOError::FileRead)
                .info(err.to_string())
                .info(format!("path: {:?}", path))
                .into());
        }
    };

    let lex_result = lexer::lex(&source);

    let mut parser = Parser {
        cursor: 0,
        tokens: lex_result.tokens,
        arena,
    };

    let file = SourceFile {
        path,
        source: source,
        line_spans: lex_result.line_spans,
    };

    let res = parser.parse_module(file);
    Ok(res)
}

fn task_res() -> Arena {
    Arena::new(1024 * 1024)
}

struct Parser<'ast> {
    cursor: usize,
    tokens: Vec<TokenSpan>,
    arena: &'ast mut Arena,
}

impl<'ast> Parser<'ast> {
    fn parse_module(&mut self, file: SourceFile) -> (P<Module>, Option<Error>) {
        let mut module = self.alloc::<Module>();
        module.file = file;
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
                    return (module.copy(), Some(Error::parse(err, module, got_token)));
                }
            }
        }
        (module, None)
    }

    fn parse_ident(&mut self, context: ParseContext) -> Result<Ident, ParseError> {
        if self.peek() == Token::Ident {
            let span = self.peek_span();
            self.consume();
            return Ok(Ident { span, id: 0 });
        }
        Err(ParseError::Ident(context))
    }

    fn parse_module_path(&mut self) -> Result<ModulePath, ParseError> {
        let kind = self.parse_module_path_kind()?;
        let mut names = List::new();

        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = self.parse_ident(ParseContext::ModulePath)?;
            self.consume();
            names.add(&mut self.arena, name);
        }

        Ok(ModulePath {
            kind: kind.0,
            kind_span: kind.1,
            names,
        })
    }

    fn parse_module_path_required(&mut self) -> Result<ModulePath, ParseError> {
        let kind = self.parse_module_path_kind()?;
        let mut names = List::new();

        if kind.0 == ModulePathKind::None {
            let first = self.parse_ident(ParseContext::ModulePath)?;
            names.add(&mut self.arena, first);
            self.expect_token(Token::ColonColon, ParseContext::ModulePath)?;
        }

        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = self.parse_ident(ParseContext::ModulePath)?;
            self.consume();
            names.add(&mut self.arena, name);
        }

        Ok(ModulePath {
            kind: kind.0,
            kind_span: kind.1,
            names,
        })
    }

    fn parse_module_path_kind(&mut self) -> Result<(ModulePathKind, Span), ParseError> {
        let kind = match self.peek() {
            Token::KwSuper => ModulePathKind::Super,
            Token::KwPackage => ModulePathKind::Package,
            _ => ModulePathKind::None,
        };
        let start = self.peek_span_start();
        let mut span = Span::new(start, start);
        if kind != ModulePathKind::None {
            self.consume();
            span.end = self.peek_span_end();
            self.expect_token(Token::ColonColon, ParseContext::ModulePath)?;
        }
        Ok((kind, span))
    }

    fn parse_generic_args(&mut self) -> Result<Option<GenericArgs>, ParseError> {
        let span_start = self.peek_span_start();
        //@ `!` IS TEMP FOR TOKEN BEFORE G.ARGS
        if !self.try_consume(Token::LogicNot) {
            return Ok(None);
        }
        self.expect_token(Token::Less, ParseContext::GenericArgs)?;
        let mut types = List::new();
        let ty = self.parse_type()?;
        types.add(&mut self.arena, ty);
        while self.try_consume(Token::Comma) {
            let ty = self.parse_type()?;
            types.add(&mut self.arena, ty);
        }
        self.expect_token(Token::Greater, ParseContext::GenericArgs)?;
        Ok(Some(GenericArgs {
            types,
            span: Span::new(span_start, self.peek_span_end()),
        }))
    }

    fn parse_generic_params(&mut self) -> Result<Option<GenericParams>, ParseError> {
        let span_start = self.peek_span_start();
        if !self.try_consume(Token::Less) {
            return Ok(None);
        }
        let mut names = List::new();
        let name = self.parse_ident(ParseContext::GenericParams)?;
        names.add(&mut self.arena, name);
        while self.try_consume(Token::Comma) {
            let name = self.parse_ident(ParseContext::GenericParams)?;
            names.add(&mut self.arena, name);
        }
        self.expect_token(Token::Greater, ParseContext::GenericParams)?;
        Ok(Some(GenericParams {
            names,
            span: Span::new(span_start, self.peek_span_end()),
        }))
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut ty = Type {
            ptr_level: PtrLevel::new(),
            kind: TypeKind::Basic(BasicType::Bool),
        };
        while self.try_consume(Token::Star) {
            let mutt = self.parse_mut();
            if let Err(..) = ty.ptr_level.add_level(mutt) {
                //@silently ignore extra pointer indir
                //current error capturing doesnt support this limitation
            }
        }
        if let Some(basic) = self.try_consume_basic_type() {
            ty.kind = TypeKind::Basic(basic);
            return Ok(ty);
        }
        match self.peek() {
            Token::Ident | Token::KwSuper | Token::KwPackage => {
                ty.kind = TypeKind::Custom(self.parse_custom_type()?);
                Ok(ty)
            }
            Token::OpenBracket => {
                ty.kind = match self.peek_next(1) {
                    Token::KwMut | Token::CloseBracket => {
                        TypeKind::ArraySlice(self.parse_array_slice()?)
                    }
                    _ => TypeKind::ArrayStatic(self.parse_array_static()?),
                };
                Ok(ty)
            }
            _ => Err(ParseError::TypeMatch),
        }
    }

    fn parse_custom_type(&mut self) -> Result<P<CustomType>, ParseError> {
        let mut custom_type = self.alloc::<CustomType>();
        custom_type.module_path = self.parse_module_path()?;
        custom_type.name = self.parse_ident(ParseContext::CustomType)?;
        custom_type.generic_args = self.parse_generic_args()?;
        Ok(custom_type)
    }

    fn parse_array_slice(&mut self) -> Result<P<ArraySlice>, ParseError> {
        let mut array_slice = self.alloc::<ArraySlice>();
        self.expect_token(Token::OpenBracket, ParseContext::ArraySlice)?;
        array_slice.mutt = self.parse_mut();
        self.expect_token(Token::CloseBracket, ParseContext::ArraySlice)?;
        array_slice.element = self.parse_type()?;
        Ok(array_slice)
    }

    fn parse_array_static(&mut self) -> Result<P<ArrayStatic>, ParseError> {
        let mut array_static = self.alloc::<ArrayStatic>();
        self.expect_token(Token::OpenBracket, ParseContext::ArrayStatic)?;
        array_static.size = ConstExpr(self.parse_expr()?);
        self.expect_token(Token::CloseBracket, ParseContext::ArrayStatic)?;
        array_static.element = self.parse_type()?;
        Ok(array_static)
    }

    fn parse_vis(&mut self) -> Visibility {
        if self.try_consume(Token::KwPub) {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn parse_mut(&mut self) -> Mutability {
        if self.try_consume(Token::KwMut) {
            Mutability::Mutable
        } else {
            Mutability::Immutable
        }
    }

    fn parse_decl(&mut self) -> Result<Decl, ParseError> {
        match self.peek() {
            Token::KwImport => Ok(Decl::Import(self.parse_import_decl()?)),
            Token::Ident | Token::KwPub => {
                let vis_span = self.peek_span();
                let vis = self.parse_vis();
                let vis_span = match vis {
                    Visibility::Public => Some(vis_span),
                    Visibility::Private => None,
                };

                let name = self.parse_ident(ParseContext::Decl)?;
                if self.peek() == Token::Colon {
                    return Ok(Decl::Global(self.parse_global_decl(vis, name)?));
                }

                //@still permitting generic params for mod decl, cant be easily solved rn
                let generic_params = self.parse_generic_params()?;
                self.expect_token(Token::ColonColon, ParseContext::Decl)?;

                match self.peek() {
                    Token::KwMod => Ok(Decl::Mod(self.parse_mod_decl(vis, name)?)),
                    Token::OpenParen => Ok(Decl::Proc(self.parse_proc_decl(
                        vis,
                        name,
                        generic_params,
                    )?)),
                    Token::KwImpl => Ok(Decl::Impl(self.parse_impl_decl(
                        vis_span,
                        name,
                        generic_params,
                    )?)),
                    Token::KwEnum => Ok(Decl::Enum(self.parse_enum_decl(
                        vis,
                        name,
                        generic_params,
                    )?)),
                    Token::KwUnion => Ok(Decl::Union(self.parse_union_decl(
                        vis,
                        name,
                        generic_params,
                    )?)),
                    Token::KwStruct => Ok(Decl::Struct(self.parse_struct_decl(
                        vis,
                        name,
                        generic_params,
                    )?)),
                    _ => Err(ParseError::DeclMatch), //@add another parse error kind
                }
            }
            _ => Err(ParseError::DeclMatch),
        }
    }

    fn parse_mod_decl(&mut self, vis: Visibility, name: Ident) -> Result<P<ModDecl>, ParseError> {
        let mut mod_decl = self.alloc::<ModDecl>();
        mod_decl.vis = vis;
        mod_decl.name = name;
        self.expect_token(Token::KwMod, ParseContext::ModDecl)?;
        self.expect_token(Token::Semicolon, ParseContext::ModDecl)?;
        Ok(mod_decl)
    }

    fn parse_proc_decl(
        &mut self,
        vis: Visibility,
        name: Ident,
        generic_params: Option<GenericParams>,
    ) -> Result<P<ProcDecl>, ParseError> {
        let mut proc_decl = self.alloc::<ProcDecl>();
        proc_decl.vis = vis;
        proc_decl.name = name;
        proc_decl.generic_params = generic_params;
        self.expect_token(Token::OpenParen, ParseContext::ProcDecl)?;
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
        proc_decl.return_type = if self.try_consume(Token::ArrowThin) {
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

    fn parse_impl_decl(
        &mut self,
        vis_span: Option<Span>,
        name: Ident,
        generic_params: Option<GenericParams>,
    ) -> Result<P<ImplDecl>, ParseError> {
        let mut impl_decl = self.alloc::<ImplDecl>();
        impl_decl.vis_span = vis_span;
        impl_decl.name = name;
        impl_decl.generic_params = generic_params;
        impl_decl.procs = List::new();
        self.expect_token(Token::KwImpl, ParseContext::ImplDecl)?;
        self.expect_token(Token::OpenBlock, ParseContext::ImplDecl)?;
        while !self.try_consume(Token::CloseBlock) {
            //@ 4 duplicated pre parsing of proc decl
            let vis = self.parse_vis();
            let name = self.parse_ident(ParseContext::ProcDecl)?;
            let generic_params = self.parse_generic_params()?;
            self.expect_token(Token::ColonColon, ParseContext::ProcDecl)?;
            let proc_decl = self.parse_proc_decl(vis, name, generic_params)?;
            impl_decl.procs.add(&mut self.arena, proc_decl);
        }
        Ok(impl_decl)
    }

    fn parse_enum_decl(
        &mut self,
        vis: Visibility,
        name: Ident,
        generic_params: Option<GenericParams>,
    ) -> Result<P<EnumDecl>, ParseError> {
        let mut enum_decl = self.alloc::<EnumDecl>();
        enum_decl.vis = vis;
        enum_decl.name = name;
        enum_decl.generic_params = generic_params;
        self.expect_token(Token::KwEnum, ParseContext::EnumDecl)?;
        enum_decl.basic_type = self.try_consume_basic_type();
        self.expect_token(Token::OpenBlock, ParseContext::EnumDecl)?;
        while !self.try_consume(Token::CloseBlock) {
            let variant = self.parse_enum_variant()?;
            enum_decl.variants.add(&mut self.arena, variant);
        }
        Ok(enum_decl)
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, ParseError> {
        let name = self.parse_ident(ParseContext::EnumVariant)?;
        let kind = match self.peek() {
            Token::Colon => {
                self.consume();
                VariantKind::Typed(self.parse_type()?)
            }
            Token::Assign => {
                self.consume();
                let constexpr = ConstExpr(self.parse_expr()?);
                VariantKind::Normal(Some(constexpr))
            }
            Token::Semicolon => VariantKind::Normal(None),
            _ => return Err(ParseError::EnumVariantMatch),
        };
        self.expect_token(Token::Semicolon, ParseContext::EnumVariant)?;
        Ok(EnumVariant { name, kind })
    }

    fn parse_union_decl(
        &mut self,
        vis: Visibility,
        name: Ident,
        generic_params: Option<GenericParams>,
    ) -> Result<P<UnionDecl>, ParseError> {
        let mut union_decl = self.alloc::<UnionDecl>();
        union_decl.vis = vis;
        union_decl.name = name;
        union_decl.generic_params = generic_params;
        self.expect_token(Token::KwUnion, ParseContext::UnionDecl)?;
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

    fn parse_struct_decl(
        &mut self,
        vis: Visibility,
        name: Ident,
        generic_params: Option<GenericParams>,
    ) -> Result<P<StructDecl>, ParseError> {
        let mut struct_decl = self.alloc::<StructDecl>();
        struct_decl.vis = vis;
        struct_decl.name = name;
        struct_decl.generic_params = generic_params;
        self.expect_token(Token::KwStruct, ParseContext::StructDecl)?;
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

    fn parse_global_decl(
        &mut self,
        vis: Visibility,
        name: Ident,
    ) -> Result<P<GlobalDecl>, ParseError> {
        let mut global_decl = self.alloc::<GlobalDecl>();
        global_decl.vis = vis;
        global_decl.name = name;
        self.expect_token(Token::Colon, ParseContext::GlobalDecl)?;
        if self.try_consume(Token::Assign) {
            global_decl.ty = None;
            global_decl.expr = ConstExpr(self.parse_expr()?);
        } else {
            global_decl.ty = Some(self.parse_type()?);
            self.expect_token(Token::Assign, ParseContext::GlobalDecl)?;
            global_decl.expr = ConstExpr(self.parse_expr()?);
        }
        self.expect_token(Token::Semicolon, ParseContext::GlobalDecl)?;
        Ok(global_decl)
    }

    fn parse_import_decl(&mut self) -> Result<P<ImportDecl>, ParseError> {
        let mut import_decl = self.alloc::<ImportDecl>();
        import_decl.span.start = self.peek_span_start();
        self.expect_token(Token::KwImport, ParseContext::ImportDecl)?;
        import_decl.module_path = self.parse_module_path_required()?;
        import_decl.target = self.parse_import_target()?;
        import_decl.span.end = self.peek_span_end();
        self.expect_token(Token::Semicolon, ParseContext::ImportDecl)?;
        Ok(import_decl)
    }

    fn parse_import_target(&mut self) -> Result<ImportTarget, ParseError> {
        match self.peek() {
            Token::Ident => {
                let name = self.parse_ident(ParseContext::ImportDecl)?;
                Ok(ImportTarget::Symbol(name))
            }
            Token::Star => {
                self.consume();
                Ok(ImportTarget::AllSymbols)
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
                return_.expr = match self.peek() {
                    Token::Semicolon => {
                        self.consume();
                        None
                    }
                    _ => {
                        let expr = self.parse_expr()?;
                        self.expect_token(Token::Semicolon, ParseContext::Return)?;
                        Some(expr)
                    }
                };
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

        for_.kind = match self.peek() {
            Token::OpenBlock => ForKind::Loop,
            _ => {
                let var_bind = self.parse_var_binding(Token::KwIn, ParseContext::For)?;
                if let Some(var_bind) = var_bind {
                    let lhs = self.parse_expr()?;
                    let range_kind = if self.try_consume(Token::DotDot) {
                        if self.try_consume(Token::Assign) {
                            Some(RangeKind::DotDotEq)
                        } else {
                            Some(RangeKind::DotDot)
                        }
                    } else {
                        None
                    };
                    match range_kind {
                        Some(kind) => {
                            let rhs = self.parse_expr()?;
                            ForKind::Range(var_bind, Range { lhs, rhs, kind })
                        }
                        None => ForKind::Iter(var_bind, lhs),
                    }
                } else {
                    ForKind::While(self.parse_expr()?)
                }
            }
        };
        for_.block = self.parse_block()?;
        Ok(for_)
    }

    fn parse_stmt_no_keyword(&mut self) -> Result<StmtKind, ParseError> {
        let var_bind = self.parse_var_binding(Token::Colon, ParseContext::VarDecl)?;
        if let Some(var_bind) = var_bind {
            Ok(StmtKind::VarDecl(self.parse_var_decl(var_bind)?))
        } else {
            let expr = self.parse_expr()?;
            match self.peek().as_assign_op() {
                None => {
                    let has_semi = self.try_consume(Token::Semicolon);
                    let mut expr_stmt = self.alloc::<ExprStmt>();
                    *expr_stmt = ExprStmt { expr, has_semi };
                    Ok(StmtKind::ExprStmt(expr_stmt))
                }
                Some(op) => {
                    self.consume();
                    let mut assignment = self.alloc::<Assignment>();
                    assignment.lhs = expr;
                    assignment.op = op;
                    assignment.rhs = self.parse_expr()?;
                    self.expect_token(Token::Semicolon, ParseContext::Stmt)?; //@context
                    Ok(StmtKind::Assignment(assignment))
                }
            }
        }
    }

    fn parse_var_binding(
        &mut self,
        with_token: Token,
        context: ParseContext,
    ) -> Result<Option<VarBinding>, ParseError> {
        let expect = (self.peek() == Token::KwMut)
            || ((self.peek() == Token::Ident || self.peek() == Token::Underscore)
                && self.peek_next(1) == with_token);
        if !expect {
            return Ok(None);
        }
        let mutt = self.parse_mut();
        let name = if self.peek() == Token::Underscore {
            None
        } else {
            Some(self.parse_ident(context)?)
        };
        self.expect_token(with_token, context)?;
        Ok(Some(VarBinding { mutt, name }))
    }

    fn parse_var_decl(&mut self, var_bind: VarBinding) -> Result<P<VarDecl>, ParseError> {
        let mut var_decl = self.alloc::<VarDecl>();
        var_decl.bind = var_bind;
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

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_sub_expr(0)
    }

    fn parse_sub_expr(&mut self, min_prec: u32) -> Result<Expr, ParseError> {
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
            let mut bin_expr = self.alloc::<BinaryExpr>();
            bin_expr.op = binary_op;
            bin_expr.lhs = expr_lhs;
            bin_expr.rhs = match binary_op {
                BinaryOp::Deref | BinaryOp::Index => self.parse_tail_expr(binary_op)?,
                _ => self.parse_sub_expr(prec + 1)?,
            };
            expr_lhs = Expr {
                span: Span::new(bin_expr.lhs.span.start, bin_expr.rhs.span.end),
                kind: ExprKind::BinaryExpr(bin_expr),
            }
        }
        Ok(expr_lhs)
    }

    fn parse_tail_expr(&mut self, tail: BinaryOp) -> Result<Expr, ParseError> {
        let span_start = self.peek_span_start();
        self.consume();

        let kind = match tail {
            BinaryOp::Deref => {
                let name = self.parse_ident(ParseContext::Expr)?; //context
                match self.peek() {
                    //@ `!` IS TEMP FOR TOKEN BEFORE G.ARGS
                    Token::OpenParen | Token::LogicNot => {
                        ExprKind::DotCall(self.parse_dot_call(name)?)
                    }
                    _ => ExprKind::DotName(name),
                }
            }
            BinaryOp::Index => {
                let mut index = self.alloc::<Index>();
                index.expr = self.parse_expr()?;
                self.expect_token(Token::CloseBracket, ParseContext::Expr)?; //context
                ExprKind::Index(index)
            }
            _ => return Err(ParseError::PrimaryExprMatch), //@temp error
        };

        return Ok(Expr {
            span: Span::new(span_start, self.peek_span_end()),
            kind,
        });
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        if self.try_consume(Token::OpenParen) {
            let expr = self.parse_sub_expr(0)?;
            self.expect_token(Token::CloseParen, ParseContext::Expr)?;
            return Ok(expr);
        }

        let span_start = self.peek_span_start();

        if let Some(unary_op) = self.try_consume_unary_op() {
            let mut unary_expr = self.alloc::<UnaryExpr>();
            unary_expr.op = unary_op;
            unary_expr.rhs = self.parse_primary_expr()?;

            return Ok(Expr {
                span: Span::new(span_start, self.peek_span_end()),
                kind: ExprKind::UnaryExpr(unary_expr),
            });
        }

        let kind = match self.peek() {
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
            | Token::LitString => ExprKind::Lit(self.parse_lit()?),
            Token::OpenBlock => ExprKind::Block(self.parse_block()?),
            Token::KwMatch => ExprKind::Match(self.parse_match()?),
            Token::KwCast => ExprKind::Cast(self.parse_cast()?),
            Token::KwSizeof => ExprKind::Sizeof(self.parse_sizeof()?),
            _ => {
                /*
                Type
                <prefix>      *<mut> *?
                basic:        basic_type
                custom:       <path>name<generic_args>
                slice:        []<type>
                slice mut:    [mut]<type>
                array static: [expr]<type>

                captured:
                global / local vars
                enum + parsed later:  . variant
                type + parsed later:  . assoc call
                type + .{ -> struct init

                array_init issues:
                typeless { expr, expr } can we interpreted like a block expr
                use [expr] [expr, expr] syntax?
                */

                // only parse CustomType?
                // depends on interfaces and impl semantics
                // can we call methods like []Type.call(*thing);
                // if interface can be defined on slices then yes.
                let ty = self.parse_type()?;

                match [self.peek(), self.peek_next(1)] {
                    [Token::OpenParen, ..] => {
                        let mut proc_call = self.alloc::<ProcCall>();
                        proc_call.ty = ty;
                        proc_call.input = self.parse_expr_list(
                            Token::OpenParen,
                            Token::CloseParen,
                            ParseContext::ProcCall,
                        )?;
                        ExprKind::ProcCall(proc_call)
                    }
                    [Token::Dot, Token::OpenBlock] => {
                        let mut struct_init = self.alloc::<StructInit>();
                        struct_init.ty = ty;
                        struct_init.input = List::<FieldInit>::new();
                        self.consume();
                        self.consume();
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
                    _ => ExprKind::Item(ty),
                }
            }
        };

        Ok(Expr {
            kind,
            span: Span::new(span_start, self.peek_span_end()),
        })
    }

    fn parse_dot_call(&mut self, name: Ident) -> Result<P<Call>, ParseError> {
        let mut call = self.alloc::<Call>();
        call.name = name;
        call.generic_args = self.parse_generic_args()?;
        //@temp context
        call.input =
            self.parse_expr_list(Token::OpenParen, Token::CloseParen, ParseContext::Expr)?;
        Ok(call)
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
        if_.condition = self.parse_expr()?;
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
        let mut match_ = self.alloc::<Match>();
        self.expect_token(Token::KwMatch, ParseContext::Match)?;
        match_.expr = self.parse_expr()?;
        self.expect_token(Token::OpenBlock, ParseContext::Match)?;
        while !self.try_consume(Token::CloseBlock) {
            let arm = self.parse_match_arm()?;
            match_.arms.add(&mut self.arena, arm);
        }
        Ok(match_)
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ParseError> {
        //@expr would be a subset of patterns, later on when we add sum types
        let pattern = self.parse_expr()?;
        self.expect_token(Token::ArrowWide, ParseContext::MatchArm)?;
        let expr = self.parse_expr()?;
        Ok(MatchArm { pattern, expr })
    }

    fn parse_cast(&mut self) -> Result<P<Cast>, ParseError> {
        let mut cast = self.alloc::<Cast>();
        self.expect_token(Token::KwCast, ParseContext::Cast)?;
        self.expect_token(Token::OpenParen, ParseContext::Cast)?;
        cast.ty = self.parse_type()?;
        self.expect_token(Token::Comma, ParseContext::Cast)?;
        cast.expr = self.parse_expr()?;
        self.expect_token(Token::CloseParen, ParseContext::Cast)?;
        Ok(cast)
    }

    fn parse_sizeof(&mut self) -> Result<P<Sizeof>, ParseError> {
        let mut sizeof = self.alloc::<Sizeof>();
        self.expect_token(Token::KwSizeof, ParseContext::Sizeof)?;
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
            Token::LitString => {
                self.consume();
                Ok(Lit::String)
            }
            _ => Err(ParseError::LiteralMatch),
        }
    }

    fn parse_array_init(&mut self) -> Result<P<ArrayInit>, ParseError> {
        let mut array_init = self.alloc::<ArrayInit>();
        array_init.ty = if self.peek() == Token::OpenBracket {
            Some(self.parse_type()?)
        } else {
            None
        };
        array_init.input =
            self.parse_expr_list(Token::OpenBlock, Token::CloseBlock, ParseContext::ArrayInit)?;
        Ok(array_init)
    }

    fn parse_expr_list(
        &mut self,
        start: Token,
        end: Token,
        context: ParseContext,
    ) -> Result<List<Expr>, ParseError> {
        let mut expr_list = List::<Expr>::new();
        self.expect_token(start, context)?;
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

    fn alloc<T>(&mut self) -> P<T> {
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

    fn expect_assign_op(&mut self, context: ParseContext) -> Result<AssignOp, ParseError> {
        match self.peek().as_assign_op() {
            Some(op) => {
                self.consume();
                Ok(op)
            }
            None => Err(ParseError::ExpectAssignOp(context)),
        }
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
            Token::Star => Some(UnaryOp::Addr(Mutability::Immutable)),
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
