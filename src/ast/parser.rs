use super::ast::*;
use super::intern::*;
use super::parse_error::*;
use super::span::Span;
use super::token::Token;
use super::token_list::TokenList;
use crate::err::error::Error;
use crate::mem::*;

pub struct Parser<'ast> {
    cursor: usize,
    tokens: &'ast TokenList,
    arena: &'ast mut Arena,
    source: &'ast str,
    char_id: u32,
    string_id: u32,
}

impl<'ast> Parser<'ast> {
    pub fn new(tokens: &'ast TokenList, arena: &'ast mut Arena, source: &'ast str) -> Self {
        Self {
            cursor: 0,
            tokens,
            arena,
            source,
            char_id: 0,
            string_id: 0,
        }
    }

    fn peek(&self) -> Token {
        self.tokens.get_token(self.cursor)
    }

    fn peek_next(&self) -> Token {
        self.tokens.get_token(self.cursor + 1)
    }

    fn peek_span(&self) -> Span {
        self.tokens.get_span(self.cursor)
    }

    fn peek_span_start(&self) -> u32 {
        self.tokens.get_span(self.cursor).start
    }

    fn peek_span_end(&self) -> u32 {
        self.tokens.get_span(self.cursor - 1).end
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

    fn expect(&mut self, token: Token, ctx: ParseCtx) -> Result<(), ParseError> {
        if token == self.peek() {
            self.eat();
            return Ok(());
        }
        Err(ParseError::ExpectToken(ctx, token))
    }

    pub fn module(&mut self, file_id: super::FileID) -> Result<P<Module>, Error> {
        let mut module = self.arena.alloc::<Module>();
        let mut decls = ListBuilder::new();
        while self.peek() != Token::Eof {
            match self.decl() {
                Ok(decl) => decls.add(&mut self.arena, decl),
                Err(error) => {
                    let got_token = (self.peek(), self.peek_span());
                    return Err(Error::parse(ParseErrorData::new(error, file_id, got_token)));
                }
            }
        }
        module.file_id = file_id;
        module.decls = decls.take();
        Ok(module)
    }

    fn decl(&mut self) -> Result<Decl, ParseError> {
        let vis = self.vis();
        match self.peek() {
            Token::KwMod => Ok(Decl::Mod(self.mod_decl(vis)?)),
            Token::KwUse => Ok(Decl::Use(self.use_decl()?)),
            Token::KwProc => Ok(Decl::Proc(self.proc_decl(vis)?)),
            Token::KwEnum => Ok(Decl::Enum(self.enum_decl(vis)?)),
            Token::KwUnion => Ok(Decl::Union(self.union_decl(vis)?)),
            Token::KwStruct => Ok(Decl::Struct(self.struct_decl(vis)?)),
            Token::KwConst => Ok(Decl::Const(self.const_decl(vis)?)),
            Token::KwGlobal => Ok(Decl::Global(self.global_decl(vis)?)),
            _ => Err(ParseError::DeclMatch),
        }
    }

    fn mod_decl(&mut self, vis: Vis) -> Result<P<ModDecl>, ParseError> {
        self.eat(); // `mod`
        let mut mod_decl = self.arena.alloc::<ModDecl>();
        mod_decl.vis = vis;
        mod_decl.name = self.ident(ParseCtx::ModDecl)?;
        self.expect(Token::Semicolon, ParseCtx::ModDecl)?;
        Ok(mod_decl)
    }

    fn use_decl(&mut self) -> Result<P<UseDecl>, ParseError> {
        self.eat(); // `use`
        let mut use_decl = self.arena.alloc::<UseDecl>();
        let mut symbols = ListBuilder::new();
        use_decl.path = self.path()?;
        self.expect(Token::Dot, ParseCtx::UseDecl)?;
        self.expect(Token::OpenBlock, ParseCtx::UseDecl)?;
        if !self.try_eat(Token::CloseBlock) {
            loop {
                let symbol = self.use_symbol()?;
                symbols.add(&mut self.arena, symbol);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::CloseBlock, ParseCtx::UseDecl)?;
        }
        use_decl.symbols = symbols.take();
        Ok(use_decl)
    }

    fn use_symbol(&mut self) -> Result<UseSymbol, ParseError> {
        let mut symbol = UseSymbol {
            name: self.ident(ParseCtx::UseDecl)?,
            alias: None,
        };
        if self.try_eat(Token::KwAs) {
            symbol.alias = Some(self.ident(ParseCtx::UseDecl)?);
        }
        Ok(symbol)
    }

    fn proc_decl(&mut self, vis: Vis) -> Result<P<ProcDecl>, ParseError> {
        self.eat(); // `proc`
        let mut proc_decl = self.arena.alloc::<ProcDecl>();
        let mut params = ListBuilder::new();
        proc_decl.vis = vis;
        proc_decl.name = self.ident(ParseCtx::ProcDecl)?;
        self.expect(Token::OpenParen, ParseCtx::ProcDecl)?;
        if !self.try_eat(Token::CloseParen) {
            loop {
                if self.try_eat(Token::DotDot) {
                    proc_decl.is_variadic = true;
                    break;
                }
                let param = self.proc_param()?;
                params.add(&mut self.arena, param);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::CloseParen, ParseCtx::ProcDecl)?;
        }
        proc_decl.return_ty = if self.try_eat(Token::ArrowThin) {
            Some(self.ty()?)
        } else {
            None
        };
        proc_decl.block = if self.try_eat(Token::DirCCall) {
            None
        } else {
            Some(self.block()?)
        };
        proc_decl.params = params.take();
        Ok(proc_decl)
    }

    fn proc_param(&mut self) -> Result<ProcParam, ParseError> {
        let mutt = self.mutt();
        let name = self.ident(ParseCtx::ProcParam)?;
        self.expect(Token::Colon, ParseCtx::ProcParam)?;
        let ty = self.ty()?;
        Ok(ProcParam { mutt, name, ty })
    }

    fn enum_decl(&mut self, vis: Vis) -> Result<P<EnumDecl>, ParseError> {
        self.eat(); // `enum`
        let mut enum_decl = self.arena.alloc::<EnumDecl>();
        let mut variants = ListBuilder::new();
        enum_decl.vis = vis;
        enum_decl.name = self.ident(ParseCtx::EnumDecl)?;
        self.expect(Token::OpenBlock, ParseCtx::EnumDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let variant = self.enum_variant()?;
            variants.add(&mut self.arena, variant);
        }
        enum_decl.variants = variants.take();
        Ok(enum_decl)
    }

    fn enum_variant(&mut self) -> Result<EnumVariant, ParseError> {
        let name = self.ident(ParseCtx::EnumVariant)?;
        let value = if self.try_eat(Token::Equals) {
            Some(ConstExpr(self.expr()?))
        } else {
            None
        };
        self.expect(Token::Semicolon, ParseCtx::EnumVariant)?;
        Ok(EnumVariant { name, value })
    }

    fn union_decl(&mut self, vis: Vis) -> Result<P<UnionDecl>, ParseError> {
        self.eat(); // `union`
        let mut union_decl = self.arena.alloc::<UnionDecl>();
        let mut members = ListBuilder::new();
        union_decl.vis = vis;
        union_decl.name = self.ident(ParseCtx::UnionDecl)?;
        self.expect(Token::OpenBlock, ParseCtx::UnionDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let member = self.union_member()?;
            members.add(&mut self.arena, member);
        }
        union_decl.members = members.take();
        Ok(union_decl)
    }

    fn union_member(&mut self) -> Result<UnionMember, ParseError> {
        let name = self.ident(ParseCtx::UnionMember)?;
        self.expect(Token::Colon, ParseCtx::UnionMember)?;
        let ty = self.ty()?;
        self.expect(Token::Semicolon, ParseCtx::UnionMember)?;
        Ok(UnionMember { name, ty })
    }

    fn struct_decl(&mut self, vis: Vis) -> Result<P<StructDecl>, ParseError> {
        self.eat(); // `struct`
        let mut struct_decl = self.arena.alloc::<StructDecl>();
        let mut fields = ListBuilder::new();
        struct_decl.vis = vis;
        struct_decl.name = self.ident(ParseCtx::StructDecl)?;
        self.expect(Token::OpenBlock, ParseCtx::StructDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let field = self.struct_field()?;
            fields.add(&mut self.arena, field);
        }
        struct_decl.fields = fields.take();
        Ok(struct_decl)
    }

    fn struct_field(&mut self) -> Result<StructField, ParseError> {
        let vis = self.vis();
        let name = self.ident(ParseCtx::StructField)?;
        self.expect(Token::Colon, ParseCtx::StructField)?;
        let ty = self.ty()?;
        self.expect(Token::Semicolon, ParseCtx::StructField)?;
        Ok(StructField { vis, name, ty })
    }

    fn const_decl(&mut self, vis: Vis) -> Result<P<ConstDecl>, ParseError> {
        self.eat(); // `const`
        let mut const_decl = self.arena.alloc::<ConstDecl>();
        const_decl.vis = vis;
        const_decl.name = self.ident(ParseCtx::ConstDecl)?;
        self.expect(Token::Colon, ParseCtx::ConstDecl)?;
        const_decl.ty = if self.try_eat(Token::Equals) {
            None
        } else {
            let ty = Some(self.ty()?);
            self.expect(Token::Equals, ParseCtx::ConstDecl)?;
            ty
        };
        const_decl.value = ConstExpr(self.expr()?);
        self.expect(Token::Semicolon, ParseCtx::ConstDecl)?;
        Ok(const_decl)
    }

    fn global_decl(&mut self, vis: Vis) -> Result<P<GlobalDecl>, ParseError> {
        self.eat(); // `global`
        let mut global_decl = self.arena.alloc::<GlobalDecl>();
        global_decl.vis = vis;
        global_decl.name = self.ident(ParseCtx::GlobalDecl)?;
        self.expect(Token::Colon, ParseCtx::GlobalDecl)?;
        global_decl.ty = if self.try_eat(Token::Equals) {
            None
        } else {
            let ty = Some(self.ty()?);
            self.expect(Token::Equals, ParseCtx::GlobalDecl)?;
            ty
        };
        global_decl.value = ConstExpr(self.expr()?);
        self.expect(Token::Semicolon, ParseCtx::GlobalDecl)?;
        Ok(global_decl)
    }

    fn vis(&mut self) -> Vis {
        if self.try_eat(Token::KwPub) {
            return Vis::Public;
        }
        Vis::Private
    }

    fn mutt(&mut self) -> Mut {
        if self.try_eat(Token::KwMut) {
            return Mut::Mutable;
        }
        Mut::Immutable
    }

    fn ident(&mut self, ctx: ParseCtx) -> Result<Ident, ParseError> {
        if self.peek() == Token::Ident {
            let span = self.peek_span();
            self.eat();
            return Ok(Ident {
                span,
                id: INTERN_DUMMY_ID,
            });
        }
        Err(ParseError::ExpectIdent(ctx))
    }

    fn path(&mut self) -> Result<P<Path>, ParseError> {
        let mut path = self.arena.alloc::<Path>();
        let mut names = ListBuilder::new();
        path.span_start = self.peek_span_start();
        path.kind = match self.peek() {
            Token::KwSuper => {
                self.eat(); // `super`
                PathKind::Super
            }
            Token::KwPackage => {
                self.eat(); // `package`
                PathKind::Package
            }
            _ => {
                let name = self.ident(ParseCtx::Path)?;
                names.add(&mut self.arena, name);
                PathKind::None
            }
        };
        while self.peek() == Token::Dot && self.peek_next() != Token::OpenBlock {
            self.eat(); // `.`
            let name = self.ident(ParseCtx::Path)?;
            names.add(&mut self.arena, name);
        }
        path.names = names.take();
        Ok(path)
    }

    fn ty(&mut self) -> Result<Type, ParseError> {
        let mut ty = Type {
            ptr: PtrLevel::new(),
            kind: TypeKind::Basic(BasicType::Unit),
        };
        while self.try_eat(Token::Star) {
            let mutt = self.mutt();
            if let Err(..) = ty.ptr.add_level(mutt) {
                //@overflown ptr indirection span cannot be captured by current err system
                // silently ignoring this error
            }
        }
        if let Some(basic) = self.peek().as_basic_type() {
            self.eat(); // `basic_type`
            ty.kind = TypeKind::Basic(basic);
            return Ok(ty);
        }
        ty.kind = match self.peek() {
            Token::OpenParen => {
                self.eat(); // `(`
                self.expect(Token::CloseParen, ParseCtx::UnitType)?;
                TypeKind::Basic(BasicType::Unit)
            }
            Token::Ident | Token::KwSuper | Token::KwPackage => TypeKind::Custom(self.path()?),
            Token::OpenBracket => match self.peek_next() {
                Token::KwMut | Token::CloseBracket => {
                    self.eat(); // `[`
                    let mut array_slice = self.arena.alloc::<ArraySlice>();
                    array_slice.mutt = self.mutt();
                    self.expect(Token::CloseBracket, ParseCtx::ArraySlice)?;
                    array_slice.ty = self.ty()?;
                    TypeKind::ArraySlice(array_slice)
                }
                _ => {
                    self.eat(); // `[`
                    let mut array_static = self.arena.alloc::<ArrayStatic>();
                    array_static.size = ConstExpr(self.expr()?);
                    self.expect(Token::CloseBracket, ParseCtx::ArrayStatic)?;
                    array_static.ty = self.ty()?;
                    TypeKind::ArrayStatic(array_static)
                }
            },
            _ => return Err(ParseError::TypeMatch),
        };
        Ok(ty)
    }

    fn stmt(&mut self) -> Result<Stmt, ParseError> {
        let span_start = self.peek_span_start();
        let kind = match self.peek() {
            Token::KwBreak => {
                self.eat(); // `break`
                self.expect(Token::Semicolon, ParseCtx::Break)?;
                StmtKind::Break
            }
            Token::KwContinue => {
                self.eat(); // `continue`
                self.expect(Token::Semicolon, ParseCtx::Continue)?;
                StmtKind::Continue
            }
            Token::KwReturn => {
                self.eat(); // `return`
                if self.try_eat(Token::Semicolon) {
                    StmtKind::Return(None)
                } else {
                    let expr = self.expr()?;
                    self.expect(Token::Semicolon, ParseCtx::Return)?;
                    StmtKind::Return(Some(expr))
                }
            }
            Token::KwDefer => {
                self.eat(); // `defer`
                StmtKind::Defer(self.block()?)
            }
            Token::KwFor => {
                self.eat(); // `for`
                StmtKind::ForLoop(self.for_()?)
            }
            _ => self.stmt_no_keyword()?,
        };
        Ok(Stmt {
            kind,
            span: Span::new(span_start, self.peek_span_end()),
        })
    }

    fn for_(&mut self) -> Result<P<For>, ParseError> {
        let mut for_ = self.arena.alloc::<For>();
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
                    let var_decl = self.var_decl()?;
                    let cond = self.expr()?;
                    self.expect(Token::Semicolon, ParseCtx::ForLoop)?;
                    let mut var_assign = self.arena.alloc::<VarAssign>();
                    var_assign.lhs = self.expr()?;
                    var_assign.op = match self.peek().as_assign_op() {
                        Some(op) => {
                            self.eat();
                            op
                        }
                        _ => return Err(ParseError::ForAssignOp),
                    };
                    var_assign.rhs = self.expr()?;
                    for_.kind = ForKind::ForLoop {
                        var_decl,
                        cond,
                        var_assign,
                    };
                } else {
                    for_.kind = ForKind::While { cond: self.expr()? };
                }
            }
        }
        for_.block = self.block()?;
        Ok(for_)
    }

    fn stmt_no_keyword(&mut self) -> Result<StmtKind, ParseError> {
        let expect_var_bind = (self.peek() == Token::KwMut)
            || ((self.peek() == Token::Ident || self.peek() == Token::Underscore)
                && self.peek_next() == Token::Colon);
        if expect_var_bind {
            Ok(StmtKind::VarDecl(self.var_decl()?))
        } else {
            let expr = self.expr()?;
            match self.peek().as_assign_op() {
                Some(op) => {
                    self.eat();
                    let mut var_assign = self.arena.alloc::<VarAssign>();
                    var_assign.op = op;
                    var_assign.lhs = expr;
                    var_assign.rhs = self.expr()?;
                    self.expect(Token::Semicolon, ParseCtx::VarAssign)?;
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

    fn var_decl(&mut self) -> Result<P<VarDecl>, ParseError> {
        let mut var_decl = self.arena.alloc::<VarDecl>();
        var_decl.mutt = self.mutt();
        var_decl.name = self.ident(ParseCtx::VarDecl)?;
        self.expect(Token::Colon, ParseCtx::VarDecl)?;
        if self.try_eat(Token::Equals) {
            var_decl.ty = None;
            var_decl.expr = Some(self.expr()?);
        } else {
            var_decl.ty = Some(self.ty()?);
            var_decl.expr = if self.try_eat(Token::Equals) {
                Some(self.expr()?)
            } else {
                None
            }
        }
        self.expect(Token::Semicolon, ParseCtx::VarDecl)?;
        Ok(var_decl)
    }

    fn expr(&mut self) -> Result<P<Expr>, ParseError> {
        self.sub_expr(0)
    }

    fn sub_expr(&mut self, min_prec: u32) -> Result<P<Expr>, ParseError> {
        let mut expr_lhs = self.primary_expr()?;
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
            let mut expr = self.arena.alloc::<Expr>();
            let op = binary_op;
            let lhs = expr_lhs;
            let rhs = self.sub_expr(prec + 1)?;
            expr.span = Span::new(lhs.span.start, rhs.span.end);
            expr.kind = ExprKind::BinaryExpr { op, lhs, rhs };
            expr_lhs = expr;
        }
        Ok(expr_lhs)
    }

    fn primary_expr(&mut self) -> Result<P<Expr>, ParseError> {
        let span_start = self.peek_span_start();

        if self.try_eat(Token::OpenParen) {
            if self.try_eat(Token::CloseParen) {
                let mut expr = self.arena.alloc::<Expr>();
                expr.kind = ExprKind::Unit;
                expr.span = Span::new(span_start, self.peek_span_end());
                return self.tail_expr(expr);
            }
            let expr = self.sub_expr(0)?;
            self.expect(Token::CloseParen, ParseCtx::Expr)?;
            return self.tail_expr(expr);
        }

        let mut expr = self.arena.alloc::<Expr>();

        if let Some(un_op) = self.peek().as_un_op() {
            self.eat();
            expr.kind = ExprKind::UnaryExpr {
                op: un_op,
                rhs: self.primary_expr()?,
            };
            expr.span = Span::new(span_start, self.peek_span_end());
            return Ok(expr);
        }

        expr.kind = match self.peek() {
            Token::KwIf => ExprKind::If { if_: self.if_()? },
            Token::KwNull
            | Token::KwTrue
            | Token::KwFalse
            | Token::IntLit
            | Token::FloatLit
            | Token::CharLit
            | Token::StringLit => self.lit()?,
            Token::OpenBlock => ExprKind::Block {
                stmts: self.block_stmts()?,
            },
            Token::KwMatch => {
                self.eat();
                let on_expr = self.expr()?;
                let mut arms = ListBuilder::new();
                self.expect(Token::OpenBlock, ParseCtx::Match)?;
                while !self.try_eat(Token::CloseBlock) {
                    let arm = self.match_arm()?;
                    arms.add(&mut self.arena, arm);
                }
                ExprKind::Match {
                    on_expr,
                    arms: arms.take(),
                }
            }
            Token::KwSizeof => {
                self.eat();
                self.expect(Token::OpenParen, ParseCtx::Sizeof)?;
                let mut pty = self.arena.alloc::<Type>();
                *pty = self.ty()?;
                self.expect(Token::CloseParen, ParseCtx::Sizeof)?;
                ExprKind::Sizeof { ty: pty }
            }
            Token::OpenBracket => {
                self.eat();
                if self.try_eat(Token::CloseBracket) {
                    ExprKind::ArrayInit {
                        input: ListBuilder::new().take(),
                    }
                } else {
                    let expr = self.expr()?;
                    if self.try_eat(Token::Semicolon) {
                        let size = ConstExpr(self.expr()?);
                        self.expect(Token::CloseBracket, ParseCtx::ArrayInit)?;
                        ExprKind::ArrayRepeat { expr, size }
                    } else {
                        let mut input = ListBuilder::new();
                        input.add(&mut self.arena, expr);
                        if !self.try_eat(Token::CloseBracket) {
                            self.expect(Token::Comma, ParseCtx::ArrayInit)?;
                            loop {
                                let expr = self.expr()?;
                                input.add(&mut self.arena, expr);
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::CloseBracket, ParseCtx::ArrayInit)?;
                        }
                        ExprKind::ArrayInit {
                            input: input.take(),
                        }
                    }
                }
            }
            _ => {
                let path = self.path()?;

                match (self.peek(), self.peek_next()) {
                    (Token::OpenParen, ..) => {
                        self.eat(); // `(`
                        let input = self.expr_list(Token::CloseParen, ParseCtx::ProcCall)?;
                        ExprKind::ProcCall { path, input }
                    }
                    (Token::Dot, Token::OpenBlock) => {
                        self.eat(); // `.`
                        self.eat(); // `{`
                        let mut input = ListBuilder::new();
                        if !self.try_eat(Token::CloseBlock) {
                            loop {
                                let name = self.ident(ParseCtx::StructInit)?;
                                let expr = match self.peek() {
                                    Token::Colon => {
                                        self.eat(); // ':'
                                        Some(self.expr()?)
                                    }
                                    Token::Comma | Token::CloseBlock => None,
                                    _ => return Err(ParseError::FieldInit),
                                };
                                input.add(&mut self.arena, FieldInit { name, expr });
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::CloseBlock, ParseCtx::StructInit)?;
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
        return self.tail_expr(expr);
    }

    fn tail_expr(&mut self, expr: P<Expr>) -> Result<P<Expr>, ParseError> {
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
                    let mut expr_field = self.arena.alloc::<Expr>();
                    let name = self.ident(ParseCtx::ExprField)?;
                    expr_field.kind = ExprKind::Field { target, name };
                    expr_field.span = Span::new(span_start, self.peek_span_end());
                    target = expr_field;
                }
                Token::OpenBracket => {
                    if last_cast {
                        return Ok(target);
                    }
                    self.eat();
                    let mut expr_index = self.arena.alloc::<Expr>();
                    let index = self.expr()?;
                    self.expect(Token::CloseBracket, ParseCtx::ExprIndex)?;
                    expr_index.kind = ExprKind::Index { target, index };
                    expr_index.span = Span::new(span_start, self.peek_span_end());
                    target = expr_index;
                }
                Token::KwAs => {
                    self.eat();
                    let mut expr_cast = self.arena.alloc::<Expr>();
                    let mut pty = self.arena.alloc::<Type>();
                    *pty = self.ty()?;
                    expr_cast.kind = ExprKind::Cast { target, ty: pty };
                    expr_cast.span = Span::new(span_start, self.peek_span_end());
                    target = expr_cast;
                    last_cast = true;
                }
                _ => return Ok(target),
            }
        }
    }

    fn if_(&mut self) -> Result<P<If>, ParseError> {
        let if_ = self.if_branch()?;
        let mut if_prev = if_;
        while self.try_eat(Token::KwElse) {
            match self.peek() {
                Token::KwIf => {
                    let else_if = self.if_branch()?;
                    if_prev.else_ = Some(Else::If { else_if });
                    if_prev = else_if;
                }
                Token::OpenBlock => {
                    let block = self.block()?;
                    if_prev.else_ = Some(Else::Block { block });
                }
                _ => return Err(ParseError::ElseMatch),
            }
        }
        Ok(if_)
    }

    fn if_branch(&mut self) -> Result<P<If>, ParseError> {
        self.eat();
        let mut if_ = self.arena.alloc::<If>();
        if_.cond = self.expr()?;
        if_.block = self.block()?;
        if_.else_ = None;
        Ok(if_)
    }

    fn block(&mut self) -> Result<P<Expr>, ParseError> {
        let span_start = self.peek_span_start();
        let mut expr = self.arena.alloc::<Expr>();
        expr.kind = ExprKind::Block {
            stmts: self.block_stmts()?,
        };
        expr.span = Span::new(span_start, self.peek_span_end());
        Ok(expr)
    }

    fn block_stmts(&mut self) -> Result<List<Stmt>, ParseError> {
        let mut stmts = ListBuilder::new();
        self.expect(Token::OpenBlock, ParseCtx::Block)?;
        while !self.try_eat(Token::CloseBlock) {
            let stmt = self.stmt()?;
            stmts.add(&mut self.arena, stmt);
        }
        Ok(stmts.take())
    }

    fn match_arm(&mut self) -> Result<MatchArm, ParseError> {
        let pat = self.expr()?;
        self.expect(Token::ArrowWide, ParseCtx::MatchArm)?;
        let expr = self.expr()?;
        Ok(MatchArm { pat, expr })
    }

    fn lit(&mut self) -> Result<ExprKind, ParseError> {
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
                            Ok(ExprKind::LitInt {
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
                        _ => Err(ParseError::LitInt),
                    }
                } else {
                    Ok(ExprKind::LitInt { val: v, ty: None })
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
                        _ => Err(ParseError::LitFloat),
                    }
                } else {
                    Ok(ExprKind::LitFloat { val: v, ty: None })
                }
            }
            Token::CharLit => {
                self.eat();
                let v = self.tokens.get_char(self.char_id as usize);
                self.char_id += 1;
                Ok(ExprKind::LitChar { val: v })
            }
            Token::StringLit => {
                self.eat();
                let id = self.string_id;
                self.string_id += 1;
                Ok(ExprKind::LitString { id: InternID(id) })
            }
            _ => Err(ParseError::LitMatch),
        }
    }

    fn expr_list(&mut self, end: Token, ctx: ParseCtx) -> Result<List<P<Expr>>, ParseError> {
        let mut expr_list = ListBuilder::new();
        if !self.try_eat(end) {
            loop {
                let expr = self.expr()?;
                expr_list.add(&mut self.arena, expr);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(end, ctx)?;
        }
        Ok(expr_list.take())
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
