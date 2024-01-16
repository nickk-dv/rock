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
            //@os err
            println!("error: {}", err);
            println!("path:  {:?}", dir_path);
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

    fn parse_module_access(&mut self) -> Result<ModuleAccess, ParseError> {
        let md = self.parse_module_access_modifier()?;
        let mut names = List::new();

        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = self.parse_ident(ParseContext::ModuleAccess)?;
            self.consume();
            names.add(&mut self.arena, name);
        }
        Ok(ModuleAccess {
            modifier: md.0,
            modifier_span: md.1,
            names,
        })
    }

    fn parse_module_access_required(&mut self) -> Result<ModuleAccess, ParseError> {
        let md = self.parse_module_access_modifier()?;
        let mut names = List::new();

        if md.0 == ModuleAccessModifier::None {
            let first = self.parse_ident(ParseContext::ModuleAccess)?;
            names.add(&mut self.arena, first);
            self.expect_token(Token::ColonColon, ParseContext::ModuleAccess)?;
        }

        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = self.parse_ident(ParseContext::ModuleAccess)?;
            self.consume();
            names.add(&mut self.arena, name);
        }
        Ok(ModuleAccess {
            modifier: md.0,
            modifier_span: md.1,
            names,
        })
    }

    fn parse_module_access_modifier(&mut self) -> Result<(ModuleAccessModifier, Span), ParseError> {
        let modifier = match self.peek() {
            Token::KwSuper => ModuleAccessModifier::Super,
            Token::KwPackage => ModuleAccessModifier::Package,
            _ => ModuleAccessModifier::None,
        };
        let start = self.peek_span_start();
        let mut span = Span::new(start, start);
        if modifier != ModuleAccessModifier::None {
            self.consume();
            span.end = self.peek_span_end();
            self.expect_token(Token::ColonColon, ParseContext::ModuleAccess)?;
        }
        Ok((modifier, span))
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let mut tt = Type {
            pointer_level: 0,
            kind: TypeKind::Basic(BasicType::Bool),
        };
        while self.try_consume(Token::Times) {
            tt.pointer_level += 1;
        }
        if let Some(basic_type) = self.try_consume_basic_type() {
            tt.kind = TypeKind::Basic(basic_type);
            return Ok(tt);
        }
        match self.peek() {
            Token::Ident => {
                tt.kind = TypeKind::Custom(self.parse_custom_type()?);
                Ok(tt)
            }
            Token::OpenBracket => {
                tt.kind = match self.peek_next(1) {
                    Token::CloseBracket => TypeKind::ArraySlice(self.parse_array_slice_type()?),
                    _ => TypeKind::ArrayStatic(self.parse_array_static_type()?),
                };
                Ok(tt)
            }
            _ => Err(ParseError::TypeMatch),
        }
    }

    fn parse_custom_type(&mut self) -> Result<P<CustomType>, ParseError> {
        let mut custom_type = self.alloc::<CustomType>();
        custom_type.module_access = self.parse_module_access()?;
        custom_type.name = self.parse_ident(ParseContext::CustomType)?;
        Ok(custom_type)
    }

    fn parse_array_slice_type(&mut self) -> Result<P<ArraySliceType>, ParseError> {
        let mut array_slice_type = self.alloc::<ArraySliceType>();
        self.expect_token(Token::OpenBracket, ParseContext::ArraySliceType)?;
        self.expect_token(Token::CloseBracket, ParseContext::ArraySliceType)?;
        array_slice_type.element = self.parse_type()?;
        Ok(array_slice_type)
    }

    fn parse_array_static_type(&mut self) -> Result<P<ArrayStaticType>, ParseError> {
        let mut array_static_type = self.alloc::<ArrayStaticType>();
        self.expect_token(Token::OpenBracket, ParseContext::ArrayStaticType)?;
        array_static_type.size = self.parse_expr()?;
        self.expect_token(Token::CloseBracket, ParseContext::ArrayStaticType)?;
        array_static_type.element = self.parse_type()?;
        Ok(array_static_type)
    }

    fn parse_visibility(&mut self) -> Visibility {
        if self.try_consume(Token::KwPub) {
            Visibility::Public
        } else {
            Visibility::Private
        }
    }

    fn parse_decl(&mut self) -> Result<Decl, ParseError> {
        match self.peek() {
            Token::KwImport => Ok(Decl::Import(self.parse_import_decl()?)),
            Token::Ident | Token::KwPub => {
                let vis = self.parse_visibility();
                let name = self.parse_ident(ParseContext::Decl)?;
                if self.peek() == Token::Colon {
                    return Ok(Decl::Global(self.parse_global_decl(vis, name)?));
                }
                self.expect_token(Token::ColonColon, ParseContext::Decl)?;
                match self.peek() {
                    Token::KwMod => Ok(Decl::Mod(self.parse_mod_decl(vis, name)?)),
                    Token::OpenParen => Ok(Decl::Proc(self.parse_proc_decl(vis, name)?)),
                    Token::KwEnum => Ok(Decl::Enum(self.parse_enum_decl(vis, name)?)),
                    Token::KwStruct => Ok(Decl::Struct(self.parse_struct_decl(vis, name)?)),
                    _ => Ok(Decl::Global(self.parse_global_decl(vis, name)?)),
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

    fn parse_proc_decl(&mut self, vis: Visibility, name: Ident) -> Result<P<ProcDecl>, ParseError> {
        let mut proc_decl = self.alloc::<ProcDecl>();
        proc_decl.vis = vis;
        proc_decl.name = name;
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
        let name = self.parse_ident(ParseContext::ProcParam)?;
        self.expect_token(Token::Colon, ParseContext::ProcParam)?;
        let tt = self.parse_type()?;
        Ok(ProcParam { name, tt })
    }

    fn parse_enum_decl(&mut self, vis: Visibility, name: Ident) -> Result<P<EnumDecl>, ParseError> {
        let mut enum_decl = self.alloc::<EnumDecl>();
        enum_decl.vis = vis;
        enum_decl.name = name;
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
        let expr = if self.try_consume(Token::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_token(Token::Semicolon, ParseContext::EnumVariant)?;
        Ok(EnumVariant { name, expr })
    }

    fn parse_struct_decl(
        &mut self,
        vis: Visibility,
        name: Ident,
    ) -> Result<P<StructDecl>, ParseError> {
        let mut struct_decl = self.alloc::<StructDecl>();
        struct_decl.vis = vis;
        struct_decl.name = name;
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
        let tt = self.parse_type()?;
        let default = if self.try_consume(Token::Assign) {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_token(Token::Semicolon, ParseContext::StructField)?;
        Ok(StructField { name, tt, default })
    }

    fn parse_global_decl(
        &mut self,
        vis: Visibility,
        name: Ident,
    ) -> Result<P<GlobalDecl>, ParseError> {
        let mut global_decl = self.alloc::<GlobalDecl>();
        global_decl.vis = vis;
        global_decl.name = name;
        global_decl.tt = if self.try_consume(Token::Colon) {
            let tt = self.parse_type()?;
            self.expect_token(Token::Colon, ParseContext::GlobalDecl)?;
            Some(tt)
        } else {
            None
        };
        global_decl.expr = self.parse_expr()?;
        self.expect_token(Token::Semicolon, ParseContext::GlobalDecl)?;
        Ok(global_decl)
    }

    fn parse_import_decl(&mut self) -> Result<P<ImportDecl>, ParseError> {
        let mut import_decl = self.alloc::<ImportDecl>();
        import_decl.span.start = self.peek_span_start();
        self.expect_token(Token::KwImport, ParseContext::ImportDecl)?;
        import_decl.module_access = self.parse_module_access_required()?;
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
            Token::Times => {
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
            Token::KwIf => StmtKind::If(self.parse_if()?),
            Token::KwFor => StmtKind::For(self.parse_for()?),
            Token::OpenBlock => StmtKind::Block(self.parse_block()?),
            Token::KwDefer => StmtKind::Defer(self.parse_defer()?),
            Token::KwBreak => self.parse_break()?,
            Token::KwSwitch => StmtKind::Switch(self.parse_switch()?),
            Token::KwReturn => StmtKind::Return(self.parse_return()?),
            Token::KwContinue => self.parse_continue()?,
            Token::Ident => {
                if self.peek_next(1) == Token::Colon {
                    StmtKind::VarDecl(self.parse_var_decl()?)
                } else {
                    let module_access = self.parse_module_access()?;
                    if self.peek() == Token::Ident && self.peek_next(1) == Token::OpenParen {
                        let proc_call = StmtKind::ProcCall(self.parse_proc_call(module_access)?);
                        self.expect_token(Token::Semicolon, ParseContext::ProcCall)?;
                        proc_call
                    } else {
                        StmtKind::VarAssign(self.parse_var_assign(module_access, true)?)
                    }
                }
            }
            _ => return Err(ParseError::StmtMatch),
        };
        let span_end = self.peek_span_end();
        Ok(Stmt {
            kind,
            span: Span::new(span_start, span_end),
        })
    }

    fn parse_if(&mut self) -> Result<P<If>, ParseError> {
        let mut if_ = self.alloc::<If>();
        self.expect_token(Token::KwIf, ParseContext::If)?;
        if_.condition = self.parse_expr()?;
        if_.block = self.parse_block()?;
        if_.else_ = self.parse_else()?;
        Ok(if_)
    }

    fn parse_else(&mut self) -> Result<Option<Else>, ParseError> {
        match self.peek() {
            Token::KwIf => Ok(Some(Else::If(self.parse_if()?))),
            Token::OpenBlock => Ok(Some(Else::Block(self.parse_block()?))),
            _ => Ok(None),
        }
    }

    fn parse_for(&mut self) -> Result<P<For>, ParseError> {
        let mut for_ = self.alloc::<For>();
        self.expect_token(Token::KwFor, ParseContext::For)?;

        if self.peek() == Token::OpenBlock {
            for_.var_decl = None;
            for_.var_assign = None;
            for_.block = self.parse_block()?;
            return Ok(for_);
        }

        for_.var_decl = if self.peek() == Token::Ident && self.peek_next(1) == Token::Colon {
            Some(self.parse_var_decl()?)
        } else {
            None
        };
        for_.condition = Some(self.parse_expr()?);
        for_.var_assign = if self.try_consume(Token::Semicolon) {
            let module_access = self.parse_module_access()?;
            Some(self.parse_var_assign(module_access, false)?)
        } else {
            None
        };

        for_.block = self.parse_block()?;
        Ok(for_)
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

    fn parse_defer(&mut self) -> Result<P<Block>, ParseError> {
        self.expect_token(Token::KwDefer, ParseContext::Defer)?;
        Ok(self.parse_block()?)
    }

    fn parse_break(&mut self) -> Result<StmtKind, ParseError> {
        self.expect_token(Token::KwBreak, ParseContext::Break)?;
        self.expect_token(Token::Semicolon, ParseContext::Break)?;
        Ok(StmtKind::Break)
    }

    fn parse_switch(&mut self) -> Result<P<Switch>, ParseError> {
        let mut switch = self.alloc::<Switch>();
        switch.expr = self.parse_expr()?;
        self.expect_token(Token::OpenBlock, ParseContext::Switch)?;
        while !self.try_consume(Token::CloseBlock) {
            let case = self.parse_switch_case()?;
            switch.cases.add(&mut self.arena, case);
        }
        Ok(switch)
    }

    fn parse_switch_case(&mut self) -> Result<SwitchCase, ParseError> {
        let expr = self.parse_expr()?;
        self.expect_token(Token::ArrowWide, ParseContext::SwitchCase)?;
        let block = self.parse_block()?;
        Ok(SwitchCase { expr, block })
    }

    fn parse_return(&mut self) -> Result<P<Return>, ParseError> {
        let mut return_ = self.alloc::<Return>();
        self.expect_token(Token::KwReturn, ParseContext::Return)?;
        return_.expr = if !self.try_consume(Token::Semicolon) {
            let expr = self.parse_expr()?;
            self.expect_token(Token::Semicolon, ParseContext::Return)?;
            Some(expr)
        } else {
            None
        };
        Ok(return_)
    }

    fn parse_continue(&mut self) -> Result<StmtKind, ParseError> {
        self.expect_token(Token::KwContinue, ParseContext::Continue)?;
        self.expect_token(Token::Semicolon, ParseContext::Continue)?;
        Ok(StmtKind::Continue)
    }

    fn parse_var_decl(&mut self) -> Result<P<VarDecl>, ParseError> {
        let mut var_decl = self.alloc::<VarDecl>();
        var_decl.name = self.parse_ident(ParseContext::VarDecl)?;
        self.expect_token(Token::Colon, ParseContext::VarDecl)?;
        if self.try_consume(Token::Assign) {
            var_decl.tt = None;
            var_decl.expr = Some(self.parse_expr()?);
        } else {
            var_decl.tt = Some(self.parse_type()?);
            var_decl.expr = if self.try_consume(Token::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            }
        }
        self.expect_token(Token::Semicolon, ParseContext::VarDecl)?;
        Ok(var_decl)
    }

    fn parse_var_assign(
        &mut self,
        module_access: ModuleAccess,
        require_semi: bool,
    ) -> Result<P<VarAssign>, ParseError> {
        let mut var_assign = self.alloc::<VarAssign>();
        var_assign.var = self.parse_var(module_access)?;
        var_assign.op = self.expect_assign_op(ParseContext::VarAssign)?;
        var_assign.expr = self.parse_expr()?;
        if require_semi {
            self.expect_token(Token::Semicolon, ParseContext::VarAssign)?;
        }
        Ok(var_assign)
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
                self.consume();
            } else {
                break;
            }
            let mut bin_expr = self.alloc::<BinaryExpr>();
            bin_expr.op = binary_op;
            bin_expr.lhs = expr_lhs;
            bin_expr.rhs = self.parse_sub_expr(prec + 1)?;
            expr_lhs = Expr {
                span: Span::new(bin_expr.lhs.span.start, bin_expr.rhs.span.end),
                kind: ExprKind::BinaryExpr(bin_expr),
            }
        }
        Ok(expr_lhs)
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
            let span_end = self.peek_span_end();
            return Ok(Expr {
                span: Span::new(span_start, span_end),
                kind: ExprKind::UnaryExpr(unary_expr),
            });
        }

        let kind = match self.peek() {
            Token::Dot => match self.peek_next(1) {
                Token::OpenBlock => {
                    let module_access = self.parse_module_access()?;
                    ExprKind::StructInit(self.parse_struct_init(module_access)?)
                }
                _ => ExprKind::Enum(self.parse_enum()?),
            },
            Token::KwCast => ExprKind::Cast(self.parse_cast()?),
            Token::KwSizeof => ExprKind::Sizeof(self.parse_sizeof()?),
            Token::LitNull
            | Token::LitBool(..)
            | Token::LitInt(..)
            | Token::LitFloat(..)
            | Token::LitChar(..)
            | Token::LitString => ExprKind::Literal(self.parse_literal()?),
            Token::OpenBracket | Token::OpenBlock => ExprKind::ArrayInit(self.parse_array_init()?),
            Token::Ident | Token::KwSuper | Token::KwPackage => {
                let module_access = self.parse_module_access()?;
                if self.peek() != Token::Ident {
                    return Err(ParseError::PrimaryExprIdent);
                }
                if self.peek_next(1) == Token::OpenParen {
                    ExprKind::ProcCall(self.parse_proc_call(module_access)?)
                } else if self.peek_next(1) == Token::Dot && self.peek_next(2) == Token::OpenBlock {
                    ExprKind::StructInit(self.parse_struct_init(module_access)?)
                } else {
                    ExprKind::Var(self.parse_var(module_access)?)
                }
            }
            _ => return Err(ParseError::PrimaryExprMatch),
        };
        let span_end = self.peek_span_end();
        Ok(Expr {
            kind,
            span: Span::new(span_start, span_end),
        })
    }

    fn parse_var(&mut self, module_access: ModuleAccess) -> Result<P<Var>, ParseError> {
        let mut var = self.alloc::<Var>();
        var.module_access = module_access;
        var.name = self.parse_ident(ParseContext::Var)?;
        var.access = self.parse_access_chain()?;
        Ok(var)
    }

    fn parse_access_chain(&mut self) -> Result<Option<P<Access>>, ParseError> {
        match self.peek() {
            Token::Dot | Token::OpenBracket => {}
            _ => return Ok(None),
        }
        let access = self.parse_access()?;
        let mut access_last = access;
        while self.peek() == Token::Dot || self.peek() == Token::OpenBracket {
            let access_next = self.parse_access()?;
            access_last.next = Some(access_next);
            access_last = access_next;
        }
        Ok(Some(access))
    }

    fn parse_access(&mut self) -> Result<P<Access>, ParseError> {
        let mut access = self.alloc::<Access>();
        match self.peek() {
            Token::Dot => {
                self.consume();
                access.kind = AccessKind::Field(self.parse_ident(ParseContext::Access)?);
                Ok(access)
            }
            Token::OpenBracket => {
                self.consume();
                access.kind = AccessKind::Array(self.parse_expr()?);
                self.expect_token(Token::CloseBracket, ParseContext::ArrayAccess)?;
                Ok(access)
            }
            _ => Err(ParseError::AccessMatch),
        }
    }

    fn parse_enum(&mut self) -> Result<P<Enum>, ParseError> {
        let mut enum_ = self.alloc::<Enum>();
        self.expect_token(Token::Dot, ParseContext::Enum)?;
        enum_.variant = self.parse_ident(ParseContext::Enum)?;
        Ok(enum_)
    }

    fn parse_cast(&mut self) -> Result<P<Cast>, ParseError> {
        let mut cast = self.alloc::<Cast>();
        self.expect_token(Token::KwCast, ParseContext::Cast)?;
        self.expect_token(Token::OpenParen, ParseContext::Cast)?;
        cast.tt = self.parse_type()?;
        self.expect_token(Token::Comma, ParseContext::Cast)?;
        cast.expr = self.parse_expr()?;
        self.expect_token(Token::CloseParen, ParseContext::Cast)?;
        Ok(cast)
    }

    fn parse_sizeof(&mut self) -> Result<P<Sizeof>, ParseError> {
        let mut sizeof = self.alloc::<Sizeof>();
        self.expect_token(Token::KwSizeof, ParseContext::Sizeof)?;
        self.expect_token(Token::OpenParen, ParseContext::Sizeof)?;
        sizeof.tt = self.parse_type()?;
        self.expect_token(Token::CloseParen, ParseContext::Sizeof)?;
        Ok(sizeof)
    }

    fn parse_literal(&mut self) -> Result<P<Literal>, ParseError> {
        let mut literal = self.alloc::<Literal>();
        *literal = match self.peek() {
            Token::LitNull => {
                self.consume();
                Literal::Null
            }
            Token::LitBool(v) => {
                self.consume();
                Literal::Bool(v)
            }
            Token::LitInt(v) => {
                self.consume();
                let basic_option = self.peek().as_basic_type();
                let basic = match basic_option {
                    Some(b) => {
                        if matches!(
                            b,
                            BasicType::S8
                                | BasicType::S16
                                | BasicType::S32
                                | BasicType::S64
                                | BasicType::Ssize
                                | BasicType::U8
                                | BasicType::U16
                                | BasicType::U32
                                | BasicType::U64
                                | BasicType::Usize
                        ) {
                            self.consume();
                            Some(b)
                        } else {
                            return Err(ParseError::LiteralInteger);
                        }
                    }
                    _ => None,
                };
                Literal::Uint(v, basic)
            }
            Token::LitFloat(v) => {
                self.consume();
                let basic_option = self.peek().as_basic_type();
                let basic = match basic_option {
                    Some(b) => {
                        if matches!(b, BasicType::F32 | BasicType::F64) {
                            self.consume();
                            Some(b)
                        } else {
                            return Err(ParseError::LiteralFloat);
                        }
                    }
                    _ => None,
                };
                Literal::Float(v, basic)
            }
            Token::LitChar(v) => {
                self.consume();
                Literal::Char(v)
            }
            Token::LitString => {
                self.consume();
                Literal::String
            }
            _ => return Err(ParseError::LiteralMatch),
        };
        Ok(literal)
    }

    fn parse_proc_call(&mut self, module_access: ModuleAccess) -> Result<P<ProcCall>, ParseError> {
        let mut proc_call = self.alloc::<ProcCall>();
        proc_call.module_access = module_access;
        proc_call.name = self.parse_ident(ParseContext::ProcCall)?;
        proc_call.input =
            self.parse_expr_list(Token::OpenParen, Token::CloseParen, ParseContext::ProcCall)?;
        proc_call.access = self.parse_access_chain()?;
        Ok(proc_call)
    }

    fn parse_array_init(&mut self) -> Result<P<ArrayInit>, ParseError> {
        let mut array_init = self.alloc::<ArrayInit>();
        array_init.tt = if self.peek() == Token::OpenBracket {
            Some(self.parse_type()?)
        } else {
            None
        };
        array_init.input =
            self.parse_expr_list(Token::OpenBlock, Token::CloseBlock, ParseContext::ArrayInit)?;
        Ok(array_init)
    }

    fn parse_struct_init(
        &mut self,
        module_access: ModuleAccess,
    ) -> Result<P<StructInit>, ParseError> {
        let mut struct_init = self.alloc::<StructInit>();
        struct_init.module_access = module_access;

        let has_access =
            module_access.modifier != ModuleAccessModifier::None || !module_access.names.is_empty();
        let has_name = self.peek() == Token::Ident || has_access;
        struct_init.name = if has_name {
            Some(self.parse_ident(ParseContext::StructInit)?)
        } else {
            None
        };
        self.expect_token(Token::Dot, ParseContext::StructInit)?;
        struct_init.input = self.parse_expr_list(
            Token::OpenBlock,
            Token::CloseBlock,
            ParseContext::StructInit,
        )?;
        Ok(struct_init)
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
            Some(op) => {
                self.consume();
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
            BinaryOp::LogicAnd | BinaryOp::LogicOr => 0,
            BinaryOp::Less
            | BinaryOp::Greater
            | BinaryOp::LessEq
            | BinaryOp::GreaterEq
            | BinaryOp::IsEq
            | BinaryOp::NotEq => 1,
            BinaryOp::Plus | BinaryOp::Minus => 2,
            BinaryOp::Times | BinaryOp::Div | BinaryOp::Mod => 3,
            BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => 4,
            BinaryOp::Shl | BinaryOp::Shr => 5,
        }
    }
}

impl Token {
    fn as_unary_op(&self) -> Option<UnaryOp> {
        match self {
            Token::Minus => Some(UnaryOp::Minus),
            Token::BitNot => Some(UnaryOp::BitNot),
            Token::LogicNot => Some(UnaryOp::LogicNot),
            Token::Times => Some(UnaryOp::AddressOf),
            Token::Shl => Some(UnaryOp::Dereference),
            _ => None,
        }
    }

    fn as_binary_op(&self) -> Option<BinaryOp> {
        match self {
            Token::LogicAnd => Some(BinaryOp::LogicAnd),
            Token::LogicOr => Some(BinaryOp::LogicOr),
            Token::Less => Some(BinaryOp::Less),
            Token::Greater => Some(BinaryOp::Greater),
            Token::LessEq => Some(BinaryOp::LessEq),
            Token::GreaterEq => Some(BinaryOp::GreaterEq),
            Token::IsEq => Some(BinaryOp::IsEq),
            Token::NotEq => Some(BinaryOp::NotEq),
            Token::Plus => Some(BinaryOp::Plus),
            Token::Minus => Some(BinaryOp::Minus),
            Token::Times => Some(BinaryOp::Times),
            Token::Div => Some(BinaryOp::Div),
            Token::Mod => Some(BinaryOp::Mod),
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
            Token::MinusEq => Some(AssignOp::BinaryOp(BinaryOp::Minus)),
            Token::TimesEq => Some(AssignOp::BinaryOp(BinaryOp::Times)),
            Token::DivEq => Some(AssignOp::BinaryOp(BinaryOp::Div)),
            Token::ModEq => Some(AssignOp::BinaryOp(BinaryOp::Mod)),
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
