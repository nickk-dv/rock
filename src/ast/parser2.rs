use super::ast2::*;
use super::intern::*;
use super::parse_error::*;
use super::span::Span;
use super::token::Token;
use super::token_list::TokenList;
use crate::ast::{AssignOp, BasicType, BinOp, UnOp};
use crate::check::SourceLoc;
use crate::err::error::Error;
use crate::err::error_new::*;
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

    pub fn module(&mut self, file_id: super::FileID) -> Result<Box<Module>, (Error, CompError)> {
        let mut decls = Vec::new();
        while self.peek() != Token::Eof {
            match self.decl() {
                Ok(decl) => decls.push(decl),
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
        Ok(Box::new(Module { file_id, decls }))
    }

    fn decl(&mut self) -> Result<Decl, ParseError> {
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

    fn use_decl(&mut self) -> Result<Box<UseDecl>, ParseError> {
        self.eat(); // `use`
        let path = self.path()?;
        let mut symbols = Vec::new();
        self.expect(Token::Dot, ParseCtx::UseDecl)?;
        self.expect(Token::OpenBlock, ParseCtx::UseDecl)?;
        if !self.try_eat(Token::CloseBlock) {
            loop {
                symbols.push(self.use_symbol()?);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::CloseBlock, ParseCtx::UseDecl)?;
        }
        Ok(Box::new(UseDecl { path, symbols }))
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

    fn mod_decl(&mut self, vis: Vis) -> Result<Box<ModDecl>, ParseError> {
        self.eat(); // `mod`
        let name = self.ident(ParseCtx::ModDecl)?;
        self.expect(Token::Semicolon, ParseCtx::ModDecl)?;
        Ok(Box::new(ModDecl { vis, name }))
    }

    fn proc_decl(&mut self, vis: Vis) -> Result<Box<ProcDecl>, ParseError> {
        self.eat(); // `proc`
        let name = self.ident(ParseCtx::ProcDecl)?;
        let mut params: Vec<ProcParam> = Vec::new();
        let mut is_variadic = false;
        self.expect(Token::OpenParen, ParseCtx::ProcDecl)?;
        if !self.try_eat(Token::CloseParen) {
            loop {
                if self.try_eat(Token::DotDot) {
                    is_variadic = true;
                    break;
                }
                params.push(self.proc_param()?);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::CloseParen, ParseCtx::ProcDecl)?;
        }
        Ok(Box::new(ProcDecl {
            vis,
            name,
            params,
            is_variadic,
            return_ty: if self.try_eat(Token::ArrowThin) {
                Some(self.ty()?)
            } else {
                None
            },
            block: if self.try_eat(Token::DirCCall) {
                None
            } else {
                Some(self.block()?)
            },
        }))
    }

    fn proc_param(&mut self) -> Result<ProcParam, ParseError> {
        let mutt = self.mutt();
        let name = self.ident(ParseCtx::ProcParam)?;
        self.expect(Token::Colon, ParseCtx::ProcParam)?;
        let ty = self.ty()?;
        Ok(ProcParam { mutt, name, ty })
    }

    fn enum_decl(&mut self, vis: Vis) -> Result<Box<EnumDecl>, ParseError> {
        self.eat(); // `enum`
        let name = self.ident(ParseCtx::EnumDecl)?;
        let mut variants = Vec::new();
        self.expect(Token::OpenBlock, ParseCtx::EnumDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            variants.push(self.enum_variant()?);
        }
        Ok(Box::new(EnumDecl {
            vis,
            name,
            variants,
        }))
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

    fn union_decl(&mut self, vis: Vis) -> Result<Box<UnionDecl>, ParseError> {
        self.eat(); // `union`
        let name = self.ident(ParseCtx::UnionDecl)?;
        let mut members = Vec::new();
        self.expect(Token::OpenBlock, ParseCtx::UnionDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            members.push(self.union_member()?);
        }
        Ok(Box::new(UnionDecl { vis, name, members }))
    }

    fn union_member(&mut self) -> Result<UnionMember, ParseError> {
        let name = self.ident(ParseCtx::UnionMember)?;
        self.expect(Token::Colon, ParseCtx::UnionMember)?;
        let ty = self.ty()?;
        self.expect(Token::Semicolon, ParseCtx::UnionMember)?;
        Ok(UnionMember { name, ty })
    }

    fn struct_decl(&mut self, vis: Vis) -> Result<Box<StructDecl>, ParseError> {
        self.eat(); // `struct`
        let name = self.ident(ParseCtx::StructDecl)?;
        let mut fields = Vec::new();
        self.expect(Token::OpenBlock, ParseCtx::StructDecl)?;
        while !self.try_eat(Token::CloseBlock) {
            fields.push(self.struct_field()?);
        }
        Ok(Box::new(StructDecl { vis, name, fields }))
    }

    fn struct_field(&mut self) -> Result<StructField, ParseError> {
        let vis = self.vis();
        let name = self.ident(ParseCtx::StructField)?;
        self.expect(Token::Colon, ParseCtx::StructField)?;
        let ty = self.ty()?;
        self.expect(Token::Semicolon, ParseCtx::StructField)?;
        Ok(StructField { vis, name, ty })
    }

    fn const_decl(&mut self, vis: Vis) -> Result<Box<ConstDecl>, ParseError> {
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
        Ok(Box::new(ConstDecl {
            vis,
            name,
            ty,
            value,
        }))
    }

    fn global_decl(&mut self, vis: Vis) -> Result<Box<GlobalDecl>, ParseError> {
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
        Ok(Box::new(GlobalDecl {
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
            return Ok(Ident {
                span,
                id: INTERN_DUMMY_ID,
            });
        }
        Err(ParseError::ExpectIdent(ctx))
    }

    fn path(&mut self) -> Result<Box<Path>, ParseError> {
        let span_start = self.peek_span_start();
        let mut names = Vec::new();
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
                names.push(self.ident(ParseCtx::Path)?);
                PathKind::None
            }
        };
        while self.peek() == Token::Dot && self.peek_next() != Token::OpenBlock {
            self.eat(); // `.`
            names.push(self.ident(ParseCtx::Path)?);
        }
        Ok(Box::new(Path {
            kind,
            names,
            span_start,
        }))
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
                    let mutt = self.mutt();
                    self.expect(Token::CloseBracket, ParseCtx::ArraySlice)?;
                    let ty = self.ty()?;
                    TypeKind::ArraySlice(Box::new(ArraySlice { mutt, ty }))
                }
                _ => {
                    self.eat(); // `[`
                    let size = ConstExpr(self.expr()?);
                    self.expect(Token::CloseBracket, ParseCtx::ArrayStatic)?;
                    let ty = self.ty()?;
                    TypeKind::ArrayStatic(Box::new(ArrayStatic { size, ty }))
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

    fn for_(&mut self) -> Result<Box<For>, ParseError> {
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
                    let var_assign = Box::new(VarAssign {
                        op,
                        lhs,
                        rhs: self.expr()?,
                    });
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
        Ok(Box::new(For {
            kind,
            block: self.block()?,
        }))
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
                    let var_assign = Box::new(VarAssign {
                        op,
                        lhs: expr,
                        rhs: self.expr()?,
                    });
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

    fn var_decl(&mut self) -> Result<Box<VarDecl>, ParseError> {
        let mutt = self.mutt();
        let name = self.ident(ParseCtx::VarDecl)?;
        let ty;
        let expr;
        self.expect(Token::Colon, ParseCtx::VarDecl)?;
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
        Ok(Box::new(VarDecl {
            mutt,
            name,
            ty,
            expr,
        }))
    }

    fn expr(&mut self) -> Result<Box<Expr>, ParseError> {
        self.sub_expr(0)
    }

    fn sub_expr(&mut self, min_prec: u32) -> Result<Box<Expr>, ParseError> {
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
            expr_lhs = Box::new(Expr {
                span: Span::new(lhs.span.start, rhs.span.end),
                kind: ExprKind::BinaryExpr { op, lhs, rhs },
            });
        }
        Ok(expr_lhs)
    }

    fn primary_expr(&mut self) -> Result<Box<Expr>, ParseError> {
        let span_start = self.peek_span_start();

        if self.try_eat(Token::OpenParen) {
            if self.try_eat(Token::CloseParen) {
                return self.tail_expr(Box::new(Expr {
                    kind: ExprKind::Unit,
                    span: Span::new(span_start, self.peek_span_end()),
                }));
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
            return Ok(Box::new(Expr {
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
                let mut arms = Vec::new();
                self.expect(Token::OpenBlock, ParseCtx::Match)?;
                while !self.try_eat(Token::CloseBlock) {
                    arms.push(self.match_arm()?);
                }
                ExprKind::Match { on_expr, arms }
            }
            Token::KwSizeof => {
                self.eat();
                self.expect(Token::OpenParen, ParseCtx::Sizeof)?;
                let ty = Box::new(self.ty()?);
                self.expect(Token::CloseParen, ParseCtx::Sizeof)?;
                ExprKind::Sizeof { ty }
            }
            Token::OpenBracket => {
                self.eat();
                if self.try_eat(Token::CloseBracket) {
                    ExprKind::ArrayInit { input: Vec::new() }
                } else {
                    let expr = self.expr()?;
                    if self.try_eat(Token::Semicolon) {
                        let size = ConstExpr(self.expr()?);
                        self.expect(Token::CloseBracket, ParseCtx::ArrayInit)?;
                        ExprKind::ArrayRepeat { expr, size }
                    } else {
                        let mut input = Vec::new();
                        input.push(expr);
                        if !self.try_eat(Token::CloseBracket) {
                            self.expect(Token::Comma, ParseCtx::ArrayInit)?;
                            loop {
                                input.push(self.expr()?);
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::CloseBracket, ParseCtx::ArrayInit)?;
                        }
                        ExprKind::ArrayInit { input }
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
                        let mut input = Vec::new();
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
                                input.push(FieldInit { name, expr });
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::CloseBlock, ParseCtx::StructInit)?;
                        }
                        ExprKind::StructInit { path, input }
                    }
                    _ => ExprKind::Item { path },
                }
            }
        };
        self.tail_expr(Box::new(Expr {
            kind,
            span: Span::new(span_start, self.peek_span_end()),
        }))
    }

    fn tail_expr(&mut self, expr: Box<Expr>) -> Result<Box<Expr>, ParseError> {
        let mut target = expr;
        let mut last_cast = false;
        let span_start = target.span.start;
        loop {
            match self.peek() {
                Token::Dot => {
                    if last_cast {
                        return Ok(target);
                    }
                    self.eat();
                    let name = self.ident(ParseCtx::ExprField)?;
                    target = Box::new(Expr {
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
                    target = Box::new(Expr {
                        kind: ExprKind::Index { target, index },
                        span: Span::new(span_start, self.peek_span_end()),
                    });
                }
                Token::KwAs => {
                    self.eat();
                    let ty = Box::new(self.ty()?);
                    target = Box::new(Expr {
                        kind: ExprKind::Cast { target, ty },
                        span: Span::new(span_start, self.peek_span_end()),
                    });
                    last_cast = true;
                }
                _ => return Ok(target),
            }
        }
    }

    fn if_(&mut self) -> Result<Box<If>, ParseError> {
        self.eat();
        let mut if_ = Box::new(If {
            cond: self.expr()?,
            block: self.block()?,
            else_: None,
        });
        if_.else_ = self.else_branch()?;
        Ok(if_)
    }

    fn else_branch(&mut self) -> Result<Option<Else>, ParseError> {
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

    fn block(&mut self) -> Result<Box<Expr>, ParseError> {
        let span_start = self.peek_span_start();
        Ok(Box::new(Expr {
            kind: ExprKind::Block {
                stmts: self.block_stmts()?,
            },
            span: Span::new(span_start, self.peek_span_end()),
        }))
    }

    fn block_stmts(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut stmts = Vec::new();
        self.expect(Token::OpenBlock, ParseCtx::Block)?;
        while !self.try_eat(Token::CloseBlock) {
            stmts.push(self.stmt()?);
        }
        Ok(stmts)
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

    fn expr_list(&mut self, end: Token, ctx: ParseCtx) -> Result<Vec<Box<Expr>>, ParseError> {
        let mut expr_list = Vec::new();
        if !self.try_eat(end) {
            loop {
                expr_list.push(self.expr()?);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(end, ctx)?;
        }
        Ok(expr_list)
    }
}
