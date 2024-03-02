use super::ast::*;
use super::intern::*;
use super::parse_error::*;
use super::span::Span;
use super::token::Token;
use super::token_list::TokenList;
use crate::check::SourceLoc;
use crate::err::error::Error;
use crate::err::error_new::*;
use crate::mem::{Arena, List, ListBuilder};

pub struct Parser<'a, 'ast> {
    cursor: usize,
    tokens: TokenList,
    arena: &'a mut Arena<'ast>,
    intern_pool: &'a mut InternPool,
    source: &'a str,
    char_id: u32,
    string_id: u32,
}

impl<'a, 'ast> Parser<'a, 'ast> {
    pub fn new(
        tokens: TokenList,
        arena: &'a mut Arena<'ast>,
        intern_pool: &'a mut InternPool,
        source: &'a str,
    ) -> Self {
        Self {
            cursor: 0,
            tokens,
            arena,
            intern_pool,
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

    pub fn module(&mut self, file_id: super::FileID) -> Result<Module<'ast>, (Error, CompError)> {
        let mut decls = ListBuilder::new();
        while self.peek() != Token::Eof {
            match self.decl() {
                Ok(decl) => decls.add(&mut self.arena, decl),
                Err(error) => {
                    let got_token = (self.peek(), self.peek_span());
                    let parse_error_data = ParseErrorData::new(error, file_id, got_token);

                    //@no marker on the span "unexpected token"
                    let mut error_ctx = "expected: ".to_string();
                    for (i, token) in parse_error_data.expected.iter().enumerate() {
                        error_ctx.push_str("`");
                        error_ctx.push_str(token.as_str());
                        error_ctx.push_str("`");
                        if i < parse_error_data.expected.len() - 1 {
                            error_ctx.push_str(", ");
                        }
                    }
                    let message = format!(
                        "Parse Error: in {}\n{}",
                        parse_error_data.ctx.as_str(),
                        error_ctx
                    );
                    let error = Error::parse(parse_error_data);
                    let comp_error = CompError::new(
                        SourceLoc::new(got_token.1, file_id),
                        Message::String(message),
                    );
                    return Err((error, comp_error));
                }
            }
        }
        Ok(Module::<'ast> {
            file_id,
            decls: decls.take(),
        })
    }

    fn decl(&mut self) -> Result<Decl<'ast>, ParseError> {
        let vis = self.vis();
        match self.peek() {
            Token::KwUse => Ok(Decl::Use(self.use_decl()?)),
            Token::KwMod => Ok(Decl::Mod(self.mod_decl(vis)?)),
            Token::KwProc => Ok(Decl::Proc(self.proc_decl(vis)?)),
            Token::KwEnum => Ok(Decl::Enum(self.enum_decl(vis)?)),
            Token::KwUnion => Ok(Decl::Union(self.union_decl(vis)?)),
            Token::KwStruct => Ok(Decl::Struct(self.struct_decl(vis)?)),
            Token::KwConst => Ok(Decl::Const(self.const_decl(vis)?)),
            Token::KwGlobal => Ok(Decl::Global(self.global_decl(vis)?)),
            _ => Err(ParseError::DeclMatch),
        }
    }

    fn use_decl(&mut self) -> Result<&'ast UseDecl<'ast>, ParseError> {
        self.eat(); // `use`
        let path = self.path()?;
        self.expect(Token::Dot, ParseCtx::UseDecl)?;
        self.expect(Token::OpenBlock, ParseCtx::UseDecl)?;
        let mut symbols = ListBuilder::new();
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
        Ok(self.arena.alloc_ref_new(UseDecl {
            path,
            symbols: symbols.take(),
        }))
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

    fn mod_decl(&mut self, vis: Vis) -> Result<&'ast ModDecl, ParseError> {
        self.eat(); // `mod`
        let name = self.ident(ParseCtx::ModDecl)?;
        self.expect(Token::Semicolon, ParseCtx::ModDecl)?;
        Ok(self.arena.alloc_ref_new(ModDecl { vis, name }))
    }

    fn proc_decl(&mut self, vis: Vis) -> Result<&'ast ProcDecl<'ast>, ParseError> {
        self.eat(); // `proc`
        let name = self.ident(ParseCtx::ProcDecl)?;
        let mut params = ListBuilder::new();
        let mut is_variadic = false;
        self.expect(Token::OpenParen, ParseCtx::ProcDecl)?;
        if !self.try_eat(Token::CloseParen) {
            loop {
                if self.try_eat(Token::DotDot) {
                    is_variadic = true;
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
        let return_ty = if self.try_eat(Token::ArrowThin) {
            Some(self.ty()?)
        } else {
            None
        };
        let block = if self.try_eat(Token::DirCCall) {
            None
        } else {
            Some(self.block()?)
        };
        Ok(self.arena.alloc_ref_new(ProcDecl {
            vis,
            name,
            params: params.take(),
            is_variadic,
            return_ty,
            block,
        }))
    }

    fn proc_param(&mut self) -> Result<ProcParam<'ast>, ParseError> {
        let mutt = self.mutt();
        let name = self.ident(ParseCtx::ProcParam)?;
        self.expect(Token::Colon, ParseCtx::ProcParam)?;
        let ty = self.ty()?;
        Ok(ProcParam { mutt, name, ty })
    }

    fn enum_decl(&mut self, vis: Vis) -> Result<&'ast EnumDecl<'ast>, ParseError> {
        self.eat(); // `enum`
        let name = self.ident(ParseCtx::EnumDecl)?;
        let mut variants = ListBuilder::new();
        self.expect(Token::OpenBlock, ParseCtx::EnumDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let variant = self.enum_variant()?;
            variants.add(&mut self.arena, variant);
        }
        let enum_decl = EnumDecl {
            vis,
            name,
            variants: variants.take(),
        };
        Ok(self.arena.alloc_ref_new(enum_decl))
    }

    fn enum_variant(&mut self) -> Result<EnumVariant<'ast>, ParseError> {
        let name = self.ident(ParseCtx::EnumVariant)?;
        let value = if self.try_eat(Token::Equals) {
            Some(ConstExpr(self.expr()?))
        } else {
            None
        };
        self.expect(Token::Semicolon, ParseCtx::EnumVariant)?;
        Ok(EnumVariant { name, value })
    }

    fn union_decl(&mut self, vis: Vis) -> Result<&'ast UnionDecl<'ast>, ParseError> {
        self.eat(); // `union`
        let name = self.ident(ParseCtx::UnionDecl)?;
        self.expect(Token::OpenBlock, ParseCtx::UnionDecl)?;
        let mut members = ListBuilder::new();
        while !self.try_eat(Token::CloseBlock) {
            let member = self.union_member()?;
            members.add(&mut self.arena, member);
        }
        Ok(self.arena.alloc_ref_new(UnionDecl {
            vis,
            name,
            members: members.take(),
        }))
    }

    fn union_member(&mut self) -> Result<UnionMember<'ast>, ParseError> {
        let name = self.ident(ParseCtx::UnionMember)?;
        self.expect(Token::Colon, ParseCtx::UnionMember)?;
        let ty = self.ty()?;
        self.expect(Token::Semicolon, ParseCtx::UnionMember)?;
        Ok(UnionMember { name, ty })
    }

    fn struct_decl(&mut self, vis: Vis) -> Result<&'ast StructDecl<'ast>, ParseError> {
        self.eat(); // `struct`
        let name = self.ident(ParseCtx::StructDecl)?;
        let mut fields = ListBuilder::new();
        self.expect(Token::OpenBlock, ParseCtx::StructDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            let field = self.struct_field()?;
            fields.add(&mut self.arena, field);
        }
        Ok(self.arena.alloc_ref_new(StructDecl {
            vis,
            name,
            fields: fields.take(),
        }))
    }

    fn struct_field(&mut self) -> Result<StructField<'ast>, ParseError> {
        let vis = self.vis();
        let name = self.ident(ParseCtx::StructField)?;
        self.expect(Token::Colon, ParseCtx::StructField)?;
        let ty = self.ty()?;
        self.expect(Token::Semicolon, ParseCtx::StructField)?;
        Ok(StructField { vis, name, ty })
    }

    fn const_decl(&mut self, vis: Vis) -> Result<&'ast ConstDecl<'ast>, ParseError> {
        self.eat(); // `const`
        let name = self.ident(ParseCtx::ConstDecl)?;
        self.expect(Token::Colon, ParseCtx::ConstDecl)?;
        let ty = if self.try_eat(Token::Equals) {
            None
        } else {
            let ty = Some(self.ty()?);
            self.expect(Token::Equals, ParseCtx::ConstDecl)?;
            ty
        };
        let value = ConstExpr(self.expr()?);
        self.expect(Token::Semicolon, ParseCtx::ConstDecl)?;
        Ok(self.arena.alloc_ref_new(ConstDecl {
            vis,
            name,
            ty,
            value,
        }))
    }

    fn global_decl(&mut self, vis: Vis) -> Result<&'ast GlobalDecl<'ast>, ParseError> {
        self.eat(); // `global`
        let name = self.ident(ParseCtx::GlobalDecl)?;
        self.expect(Token::Colon, ParseCtx::GlobalDecl)?;
        let ty = if self.try_eat(Token::Equals) {
            None
        } else {
            let ty = Some(self.ty()?);
            self.expect(Token::Equals, ParseCtx::GlobalDecl)?;
            ty
        };
        let value = ConstExpr(self.expr()?);
        self.expect(Token::Semicolon, ParseCtx::GlobalDecl)?;
        Ok(self.arena.alloc_ref_new(GlobalDecl {
            vis,
            name,
            ty,
            value,
        }))
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
            let string = span.slice(&self.source);
            return Ok(Ident {
                span,
                id: self.intern_pool.intern(string),
            });
        }
        Err(ParseError::ExpectIdent(ctx))
    }

    fn path(&mut self) -> Result<&'ast Path, ParseError> {
        let mut names = ListBuilder::new();
        let span_start = self.peek_span_start();
        let kind = match self.peek() {
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
        Ok(self.arena.alloc_ref_new(Path {
            kind,
            names: names.take(),
            span_start,
        }))
    }

    fn ty(&mut self) -> Result<Type<'ast>, ParseError> {
        if let Some(basic) = self.peek().as_basic_type() {
            self.eat(); // `basic_type`
            return Ok(Type::Basic(basic));
        }
        match self.peek() {
            Token::Star => {
                self.eat(); // '*'
                let mutt = self.mutt();
                let ty = self.ty()?;
                let ty_ref = self.arena.alloc_ref_new(ty);
                Ok(Type::Reference(ty_ref, mutt))
            }
            Token::OpenParen => {
                self.eat(); // `(`
                self.expect(Token::CloseParen, ParseCtx::UnitType)?;
                Ok(Type::Basic(BasicType::Unit))
            }
            Token::Ident | Token::KwSuper | Token::KwPackage => Ok(Type::Custom(self.path()?)),
            Token::OpenBracket => match self.peek_next() {
                Token::KwMut | Token::CloseBracket => {
                    self.eat(); // `[`
                    let mutt = self.mutt();
                    self.expect(Token::CloseBracket, ParseCtx::ArraySlice)?;
                    let ty = self.ty()?;
                    Ok(Type::ArraySlice(
                        self.arena.alloc_ref_new(ArraySlice { mutt, ty }),
                    ))
                }
                _ => {
                    self.eat(); // `[`
                    let size = ConstExpr(self.expr()?);
                    self.expect(Token::CloseBracket, ParseCtx::ArrayStatic)?;
                    let ty = self.ty()?;
                    Ok(Type::ArrayStatic(
                        self.arena.alloc_ref_new(ArrayStatic { size, ty }),
                    ))
                }
            },
            _ => Err(ParseError::TypeMatch),
        }
    }

    fn stmt(&mut self) -> Result<Stmt<'ast>, ParseError> {
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

    fn for_(&mut self) -> Result<&'ast For<'ast>, ParseError> {
        let kind = match self.peek() {
            Token::OpenBlock => ForKind::Loop,
            _ => {
                let expect_var_bind = (self.peek() == Token::KwMut)
                    || ((self.peek() == Token::Ident || self.peek() == Token::Underscore)
                        && self.peek_next() == Token::Colon);
                if expect_var_bind {
                    //@all 3 are expected
                    let var_decl = self.var_decl()?;
                    let cond = self.expr()?;
                    self.expect(Token::Semicolon, ParseCtx::ForLoop)?;
                    let lhs = self.expr()?;
                    let op = match self.peek().as_assign_op() {
                        Some(op) => {
                            self.eat();
                            op
                        }
                        _ => return Err(ParseError::ForAssignOp),
                    };
                    let rhs = self.expr()?;
                    let var_assign = self.arena.alloc_ref_new(VarAssign { op, lhs, rhs });
                    ForKind::ForLoop {
                        var_decl,
                        cond,
                        var_assign,
                    }
                } else {
                    ForKind::While { cond: self.expr()? }
                }
            }
        };
        let block = self.block()?;
        Ok(self.arena.alloc_ref_new(For { kind, block }))
    }

    fn stmt_no_keyword(&mut self) -> Result<StmtKind<'ast>, ParseError> {
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
                    let op = op;
                    let lhs = expr;
                    let rhs = self.expr()?;
                    self.expect(Token::Semicolon, ParseCtx::VarAssign)?;
                    Ok(StmtKind::VarAssign(self.arena.alloc_ref_new(VarAssign {
                        op,
                        lhs,
                        rhs,
                    })))
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

    fn var_decl(&mut self) -> Result<&'ast VarDecl<'ast>, ParseError> {
        let mutt = self.mutt();
        let name = self.ident(ParseCtx::VarDecl)?;
        self.expect(Token::Colon, ParseCtx::VarDecl)?;
        let ty;
        let expr;
        if self.try_eat(Token::Equals) {
            ty = None;
            expr = Some(self.expr()?);
        } else {
            ty = Some(self.ty()?);
            expr = if self.try_eat(Token::Equals) {
                Some(self.expr()?)
            } else {
                None
            }
        }
        self.expect(Token::Semicolon, ParseCtx::VarDecl)?;
        Ok(self.arena.alloc_ref_new(VarDecl {
            mutt,
            name,
            ty,
            expr,
        }))
    }

    fn expr(&mut self) -> Result<&'ast Expr<'ast>, ParseError> {
        self.sub_expr(0)
    }

    fn sub_expr(&mut self, min_prec: u32) -> Result<&'ast Expr<'ast>, ParseError> {
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
            let op = binary_op;
            let lhs = expr_lhs;
            let rhs = self.sub_expr(prec + 1)?;
            expr_lhs = self.arena.alloc_ref_new(Expr {
                kind: ExprKind::BinaryExpr { op, lhs, rhs },
                span: Span::new(lhs.span.start, rhs.span.end),
            });
        }
        Ok(expr_lhs)
    }

    fn primary_expr(&mut self) -> Result<&'ast Expr<'ast>, ParseError> {
        let span_start = self.peek_span_start();

        if self.try_eat(Token::OpenParen) {
            if self.try_eat(Token::CloseParen) {
                let expr = self.arena.alloc_ref_new(Expr {
                    kind: ExprKind::Unit,
                    span: Span::new(span_start, self.peek_span_end()),
                });
                return self.tail_expr(expr);
            }
            let expr = self.sub_expr(0)?;
            self.expect(Token::CloseParen, ParseCtx::Expr)?;
            return self.tail_expr(expr);
        }

        if let Some(un_op) = self.peek().as_un_op() {
            self.eat();
            let kind = ExprKind::UnaryExpr {
                op: un_op,
                rhs: self.primary_expr()?,
            };
            return Ok(self.arena.alloc_ref_new(Expr {
                kind,
                span: Span::new(span_start, self.peek_span_end()),
            }));
        }

        let kind = match self.peek() {
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
                let ty = self.ty()?;
                self.expect(Token::CloseParen, ParseCtx::Sizeof)?;
                ExprKind::Sizeof { ty }
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
        let expr = self.arena.alloc_ref_new(Expr {
            kind,
            span: Span::new(span_start, self.peek_span_end()),
        });
        self.tail_expr(expr)
    }

    fn tail_expr(&mut self, expr: &'ast Expr) -> Result<&'ast Expr<'ast>, ParseError> {
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
                    let name = self.ident(ParseCtx::ExprField)?;
                    target = self.arena.alloc_ref_new(Expr {
                        kind: ExprKind::Field { target, name },
                        span: Span::new(span_start, self.peek_span_end()),
                    });
                }
                Token::OpenBracket => {
                    if last_cast {
                        return Ok(target);
                    }
                    self.eat();
                    let index = self.expr()?;
                    self.expect(Token::CloseBracket, ParseCtx::ExprIndex)?;
                    target = self.arena.alloc_ref_new(Expr {
                        kind: ExprKind::Index { target, index },
                        span: Span::new(span_start, self.peek_span_end()),
                    });
                }
                Token::KwAs => {
                    self.eat();
                    let ty = self.ty()?;
                    let ty_ref = self.arena.alloc_ref_new(ty);
                    target = self.arena.alloc_ref_new(Expr {
                        kind: ExprKind::Cast { target, ty: ty_ref },
                        span: Span::new(span_start, self.peek_span_end()),
                    });
                    last_cast = true;
                }
                _ => return Ok(target),
            }
        }
    }

    fn if_(&mut self) -> Result<&'ast If<'ast>, ParseError> {
        self.eat();
        let if_ = If {
            cond: self.expr()?,
            block: self.block()?,
            else_: self.else_branch()?,
        };
        Ok(self.arena.alloc_ref_new(if_))
    }

    fn else_branch(&mut self) -> Result<Option<Else<'ast>>, ParseError> {
        if self.try_eat(Token::KwElse) {
            match self.peek() {
                Token::KwIf => Ok(Some(Else::If {
                    else_if: self.if_()?,
                })),
                Token::OpenBlock => Ok(Some(Else::Block {
                    block: self.block()?,
                })),
                _ => return Err(ParseError::ElseMatch),
            }
        } else {
            Ok(None)
        }
    }

    fn block(&mut self) -> Result<&'ast Expr<'ast>, ParseError> {
        let span_start = self.peek_span_start();
        let stmts = self.block_stmts()?;
        Ok(self.arena.alloc_ref_new(Expr {
            kind: ExprKind::Block { stmts },
            span: Span::new(span_start, self.peek_span_end()),
        }))
    }

    fn block_stmts(&mut self) -> Result<List<Stmt<'ast>>, ParseError> {
        let mut stmts = ListBuilder::new();
        self.expect(Token::OpenBlock, ParseCtx::Block)?;
        while !self.try_eat(Token::CloseBlock) {
            let stmt = self.stmt()?;
            stmts.add(&mut self.arena, stmt);
        }
        Ok(stmts.take())
    }

    fn match_arm(&mut self) -> Result<MatchArm<'ast>, ParseError> {
        let pat = self.expr()?;
        self.expect(Token::ArrowWide, ParseCtx::MatchArm)?;
        let expr = self.expr()?;
        Ok(MatchArm { pat, expr })
    }

    fn lit(&mut self) -> Result<ExprKind<'ast>, ParseError> {
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
                let string = self.tokens.get_string(self.string_id as usize);
                self.string_id += 1;
                Ok(ExprKind::LitString {
                    id: self.intern_pool.intern(string),
                })
            }
            _ => Err(ParseError::LitMatch),
        }
    }

    fn expr_list(
        &mut self,
        end: Token,
        ctx: ParseCtx,
    ) -> Result<List<&'ast Expr<'ast>>, ParseError> {
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
