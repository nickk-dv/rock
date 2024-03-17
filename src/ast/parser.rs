use super::ast::*;
use super::intern::*;
use super::parse_error::*;
use super::token::Token;
use super::token_list::TokenList;
use crate::arena::Arena;
use crate::error::{ErrorComp, SourceRange};
use crate::text::TextOffset;
use crate::text::TextRange;
use crate::vfs;

//@empty error tokens produce invalid range diagnostic
// need to handle 'missing' and `unexpected token` errors to be differently

pub struct Parser<'a, 'ast> {
    cursor: usize,
    tokens: TokenList,
    arena: &'a mut Arena<'ast>,
    intern_pool: &'a mut InternPool,
    source: &'a str,
    char_id: u32,
    string_id: u32,
    buf_decls: NodeBuffer<Decl<'ast>>,
    buf_use_symbols: NodeBuffer<UseSymbol>,
    buf_proc_params: NodeBuffer<ProcParam<'ast>>,
    buf_enum_variants: NodeBuffer<EnumVariant<'ast>>,
    buf_union_members: NodeBuffer<UnionMember<'ast>>,
    buf_struct_fields: NodeBuffer<StructField<'ast>>,
    buf_names: NodeBuffer<Ident>,
    buf_stmts: NodeBuffer<Stmt<'ast>>,
    buf_match_arms: NodeBuffer<MatchArm<'ast>>,
    buf_exprs: NodeBuffer<&'ast Expr<'ast>>,
    buf_field_inits: NodeBuffer<FieldInit<'ast>>,
}

struct NodeBufferOffset(usize);
struct NodeBuffer<T: Copy> {
    buffer: Vec<T>,
}

impl<T: Copy> NodeBuffer<T> {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    fn start(&self) -> NodeBufferOffset {
        NodeBufferOffset(self.buffer.len())
    }

    fn add(&mut self, value: T) {
        self.buffer.push(value);
    }

    fn take<'arena>(&mut self, start: NodeBufferOffset, arena: &mut Arena<'arena>) -> &'arena [T] {
        let slice = arena.alloc_slice(&self.buffer[start.0..]);
        self.buffer.truncate(start.0);
        slice
    }
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
            buf_decls: NodeBuffer::new(),
            buf_use_symbols: NodeBuffer::new(),
            buf_proc_params: NodeBuffer::new(),
            buf_enum_variants: NodeBuffer::new(),
            buf_union_members: NodeBuffer::new(),
            buf_struct_fields: NodeBuffer::new(),
            buf_names: NodeBuffer::new(),
            buf_stmts: NodeBuffer::new(),
            buf_match_arms: NodeBuffer::new(),
            buf_exprs: NodeBuffer::new(),
            buf_field_inits: NodeBuffer::new(),
        }
    }

    fn peek(&self) -> Token {
        self.tokens.get_token(self.cursor)
    }

    fn peek_next(&self) -> Token {
        self.tokens.get_token(self.cursor + 1)
    }

    fn peek_range(&self) -> TextRange {
        self.tokens.get_range(self.cursor)
    }

    fn peek_range_start(&self) -> TextOffset {
        self.tokens.get_range(self.cursor).start()
    }

    fn peek_range_end(&self) -> TextOffset {
        self.tokens.get_range(self.cursor - 1).end()
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

    pub fn module(&mut self, file_id: vfs::FileID) -> Result<Module<'ast>, ErrorComp> {
        let start = self.buf_decls.start();
        while self.peek() != Token::Eof {
            match self.decl() {
                Ok(decl) => self.buf_decls.add(decl),
                Err(error) => {
                    let got_token = (self.peek(), self.peek_range()); //@remove print debug
                    eprintln!("parse error token range: {:?}", got_token.1);
                    let parse_error_data = ParseErrorData::new(error, file_id, got_token);

                    //@no marker on the range of "unexpected token"
                    // incomplete new error system messages, revisit this
                    let mut error_ctx = "expected: ".to_string();
                    for (i, token) in parse_error_data.expected.iter().enumerate() {
                        error_ctx.push_str("`");
                        error_ctx.push_str(token.as_str());
                        error_ctx.push_str("`");
                        if i < parse_error_data.expected.len() - 1 {
                            error_ctx.push_str(", ");
                        }
                    }
                    let error = ErrorComp::error(format!(
                        "unexpected token in {}",
                        parse_error_data.ctx.as_str()
                    ))
                    .context_msg(error_ctx, SourceRange::new(got_token.1, file_id));
                    return Err(error);
                }
            }
        }
        Ok(Module {
            file_id,
            decls: self.buf_decls.take(start, self.arena),
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
        let start = self.buf_use_symbols.start();
        self.eat(); // `use`
        let path = self.path()?;
        self.expect(Token::Dot, ParseCtx::UseDecl)?;
        self.expect(Token::BlockOpen, ParseCtx::UseDecl)?;
        if !self.try_eat(Token::BlockClose) {
            loop {
                let symbol = self.use_symbol()?;
                self.buf_use_symbols.add(symbol);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::BlockClose, ParseCtx::UseDecl)?;
        }
        let symbols = self.buf_use_symbols.take(start, self.arena);
        Ok(self.arena.alloc(UseDecl { path, symbols }))
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
        Ok(self.arena.alloc(ModDecl { vis, name }))
    }

    fn proc_decl(&mut self, vis: Vis) -> Result<&'ast ProcDecl<'ast>, ParseError> {
        let start = self.buf_proc_params.start();
        self.eat(); // `proc`
        let name = self.ident(ParseCtx::ProcDecl)?;
        let mut is_variadic = false;
        self.expect(Token::ParenOpen, ParseCtx::ProcDecl)?;
        if !self.try_eat(Token::ParenClose) {
            loop {
                if self.try_eat(Token::DotDot) {
                    is_variadic = true;
                    break;
                }
                let param = self.proc_param()?;
                self.buf_proc_params.add(param);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(Token::ParenClose, ParseCtx::ProcDecl)?;
        }
        let return_ty = if self.try_eat(Token::ArrowThin) {
            Some(self.ty()?)
        } else {
            None
        };
        //@no directive parsing done yet
        //this is being reworked currently
        /*
        let block = if self.try_eat(Token::DirCCall) {
            None
        } else {
            Some(self.block()?)
        };
        */
        let block = Some(self.block()?);

        let params = self.buf_proc_params.take(start, self.arena);
        Ok(self.arena.alloc(ProcDecl {
            vis,
            name,
            params,
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
        let start = self.buf_enum_variants.start();
        self.eat(); // `enum`
        let name = self.ident(ParseCtx::EnumDecl)?;
        self.expect(Token::BlockOpen, ParseCtx::EnumDecl)?;
        while !self.try_eat(Token::BlockClose) {
            let variant = self.enum_variant()?;
            self.buf_enum_variants.add(variant);
        }
        let enum_decl = EnumDecl {
            vis,
            name,
            variants: self.buf_enum_variants.take(start, self.arena),
        };
        Ok(self.arena.alloc(enum_decl))
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
        let start = self.buf_union_members.start();
        self.eat(); // `union`
        let name = self.ident(ParseCtx::UnionDecl)?;
        self.expect(Token::BlockOpen, ParseCtx::UnionDecl)?;
        while !self.try_eat(Token::BlockClose) {
            let member = self.union_member()?;
            self.buf_union_members.add(member);
        }
        let members = self.buf_union_members.take(start, self.arena);
        Ok(self.arena.alloc(UnionDecl { vis, name, members }))
    }

    fn union_member(&mut self) -> Result<UnionMember<'ast>, ParseError> {
        let name = self.ident(ParseCtx::UnionMember)?;
        self.expect(Token::Colon, ParseCtx::UnionMember)?;
        let ty = self.ty()?;
        self.expect(Token::Semicolon, ParseCtx::UnionMember)?;
        Ok(UnionMember { name, ty })
    }

    fn struct_decl(&mut self, vis: Vis) -> Result<&'ast StructDecl<'ast>, ParseError> {
        let start = self.buf_struct_fields.start();
        self.eat(); // `struct`
        let name = self.ident(ParseCtx::StructDecl)?;
        self.expect(Token::BlockOpen, ParseCtx::StructDecl)?;
        while !self.try_eat(Token::BlockClose) {
            let field = self.struct_field()?;
            self.buf_struct_fields.add(field);
        }
        let fields = self.buf_struct_fields.take(start, self.arena);
        Ok(self.arena.alloc(StructDecl { vis, name, fields }))
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
        let ty = self.ty()?;
        self.expect(Token::Equals, ParseCtx::ConstDecl)?;
        let value = ConstExpr(self.expr()?);
        self.expect(Token::Semicolon, ParseCtx::ConstDecl)?;

        Ok(self.arena.alloc(ConstDecl {
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
        let ty = self.ty()?;
        self.expect(Token::Equals, ParseCtx::GlobalDecl)?;
        let value = ConstExpr(self.expr()?);
        self.expect(Token::Semicolon, ParseCtx::GlobalDecl)?;

        Ok(self.arena.alloc(GlobalDecl {
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
            let range = self.peek_range();
            self.eat();
            let string = &self.source[range.as_usize()];
            return Ok(Ident {
                range,
                id: self.intern_pool.intern(string),
            });
        }
        Err(ParseError::ExpectIdent(ctx))
    }

    fn path(&mut self) -> Result<&'ast Path<'ast>, ParseError> {
        let start = self.buf_names.start();
        let range_start = self.peek_range_start();
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
                self.buf_names.add(name);
                PathKind::None
            }
        };
        while self.peek() == Token::Dot && self.peek_next() != Token::BlockOpen {
            self.eat(); // `.`
            let name = self.ident(ParseCtx::Path)?;
            self.buf_names.add(name);
        }
        let names = self.buf_names.take(start, self.arena);
        Ok(self.arena.alloc(Path {
            kind,
            names,
            range_start,
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
                let ty_ref = self.arena.alloc(ty);
                Ok(Type::Reference(ty_ref, mutt))
            }
            Token::ParenOpen => {
                self.eat(); // `(`
                self.expect(Token::ParenClose, ParseCtx::UnitType)?;
                Ok(Type::Basic(BasicType::Unit))
            }
            Token::Ident | Token::KwSuper | Token::KwPackage => Ok(Type::Custom(self.path()?)),
            Token::BracketOpen => match self.peek_next() {
                Token::KwMut | Token::BracketClose => {
                    self.eat(); // `[`
                    let mutt = self.mutt();
                    self.expect(Token::BracketClose, ParseCtx::ArraySlice)?;
                    let ty = self.ty()?;
                    Ok(Type::ArraySlice(self.arena.alloc(ArraySlice { mutt, ty })))
                }
                _ => {
                    self.eat(); // `[`
                    let size = ConstExpr(self.expr()?);
                    self.expect(Token::BracketClose, ParseCtx::ArrayStatic)?;
                    let ty = self.ty()?;
                    Ok(Type::ArrayStatic(
                        self.arena.alloc(ArrayStatic { size, ty }),
                    ))
                }
            },
            _ => Err(ParseError::TypeMatch),
        }
    }

    fn stmt(&mut self) -> Result<Stmt<'ast>, ParseError> {
        let range_start = self.peek_range_start();
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
            range: TextRange::new(range_start, self.peek_range_end()),
        })
    }

    fn for_(&mut self) -> Result<&'ast For<'ast>, ParseError> {
        let kind = match self.peek() {
            Token::BlockOpen => ForKind::Loop,
            _ => {
                let expect_var_bind = (self.peek() == Token::KwMut)
                    || (self.peek() == Token::Ident && self.peek_next() == Token::Colon);
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
                    let var_assign = self.arena.alloc(VarAssign { op, lhs, rhs });
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
        Ok(self.arena.alloc(For { kind, block }))
    }

    fn stmt_no_keyword(&mut self) -> Result<StmtKind<'ast>, ParseError> {
        let expect_var_bind = (self.peek() == Token::KwMut)
            || (self.peek() == Token::Ident && self.peek_next() == Token::Colon);
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
                    Ok(StmtKind::VarAssign(self.arena.alloc(VarAssign {
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
        Ok(self.arena.alloc(VarDecl {
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
            expr_lhs = self.arena.alloc(Expr {
                kind: ExprKind::BinaryExpr { op, lhs, rhs },
                range: TextRange::new(lhs.range.start(), rhs.range.end()),
            });
        }
        Ok(expr_lhs)
    }

    fn primary_expr(&mut self) -> Result<&'ast Expr<'ast>, ParseError> {
        let range_start = self.peek_range_start();

        if self.try_eat(Token::ParenOpen) {
            if self.try_eat(Token::ParenClose) {
                let expr = self.arena.alloc(Expr {
                    kind: ExprKind::Unit,
                    range: TextRange::new(range_start, self.peek_range_end()),
                });
                return self.tail_expr(expr);
            }
            let expr = self.sub_expr(0)?;
            self.expect(Token::ParenClose, ParseCtx::Expr)?;
            return self.tail_expr(expr);
        }

        if let Some(un_op) = self.peek().as_un_op() {
            self.eat();
            let kind = ExprKind::UnaryExpr {
                op: un_op,
                rhs: self.primary_expr()?,
            };
            return Ok(self.arena.alloc(Expr {
                kind,
                range: TextRange::new(range_start, self.peek_range_end()),
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
            Token::BlockOpen => ExprKind::Block {
                stmts: self.block_stmts()?,
            },
            Token::KwMatch => {
                let start = self.buf_match_arms.start();
                self.eat();
                let on_expr = self.expr()?;
                self.expect(Token::BlockOpen, ParseCtx::Match)?;
                while !self.try_eat(Token::BlockClose) {
                    let arm = self.match_arm()?;
                    self.buf_match_arms.add(arm);
                }

                let arms = self.buf_match_arms.take(start, self.arena);
                let match_ = self.arena.alloc(Match { on_expr, arms });
                ExprKind::Match { match_ }
            }
            Token::KwSizeof => {
                self.eat();
                self.expect(Token::ParenOpen, ParseCtx::Sizeof)?;
                let ty = self.ty()?;
                self.expect(Token::ParenClose, ParseCtx::Sizeof)?;
                ExprKind::Sizeof { ty }
            }
            Token::BracketOpen => {
                self.eat();
                if self.try_eat(Token::BracketClose) {
                    ExprKind::ArrayInit { input: &[] }
                } else {
                    let expr = self.expr()?;
                    if self.try_eat(Token::Semicolon) {
                        let size = ConstExpr(self.expr()?);
                        self.expect(Token::BracketClose, ParseCtx::ArrayInit)?;
                        ExprKind::ArrayRepeat { expr, size }
                    } else {
                        let start = self.buf_exprs.start();
                        self.buf_exprs.add(expr);
                        if !self.try_eat(Token::BracketClose) {
                            self.expect(Token::Comma, ParseCtx::ArrayInit)?;
                            loop {
                                let expr = self.expr()?;
                                self.buf_exprs.add(expr);
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::BracketClose, ParseCtx::ArrayInit)?;
                        }
                        ExprKind::ArrayInit {
                            input: self.buf_exprs.take(start, self.arena),
                        }
                    }
                }
            }
            _ => {
                let path = self.path()?;

                match (self.peek(), self.peek_next()) {
                    (Token::ParenOpen, ..) => {
                        self.eat(); // `(`
                        let input = self.expr_list(Token::ParenClose, ParseCtx::ProcCall)?;
                        let proc_call = self.arena.alloc(ProcCall { path, input });
                        ExprKind::ProcCall { proc_call }
                    }
                    (Token::Dot, Token::BlockOpen) => {
                        self.eat(); // `.`
                        self.eat(); // `{`
                        let start = self.buf_field_inits.start();
                        if !self.try_eat(Token::BlockClose) {
                            loop {
                                let name = self.ident(ParseCtx::StructInit)?;
                                let expr = match self.peek() {
                                    Token::Colon => {
                                        self.eat(); // ':'
                                        Some(self.expr()?)
                                    }
                                    Token::Comma | Token::BlockClose => None,
                                    _ => return Err(ParseError::FieldInit),
                                };
                                self.buf_field_inits.add(FieldInit { name, expr });
                                if !self.try_eat(Token::Comma) {
                                    break;
                                }
                            }
                            self.expect(Token::BlockClose, ParseCtx::StructInit)?;
                        }
                        let input = self.buf_field_inits.take(start, self.arena);
                        let struct_init = self.arena.alloc(StructInit { path, input });
                        ExprKind::StructInit { struct_init }
                    }
                    _ => ExprKind::Item { path },
                }
            }
        };
        let expr = self.arena.alloc(Expr {
            kind,
            range: TextRange::new(range_start, self.peek_range_end()),
        });
        self.tail_expr(expr)
    }

    fn tail_expr(&mut self, expr: &'ast Expr) -> Result<&'ast Expr<'ast>, ParseError> {
        let mut target = expr;
        let mut last_cast = false;
        let range_start = expr.range.start();
        loop {
            match self.peek() {
                Token::Dot => {
                    if last_cast {
                        return Ok(target);
                    }
                    self.eat();
                    let name = self.ident(ParseCtx::ExprField)?;
                    target = self.arena.alloc(Expr {
                        kind: ExprKind::Field { target, name },
                        range: TextRange::new(range_start, self.peek_range_end()),
                    });
                }
                Token::BracketOpen => {
                    if last_cast {
                        return Ok(target);
                    }
                    self.eat();
                    let index = self.expr()?;
                    self.expect(Token::BracketClose, ParseCtx::ExprIndex)?;
                    target = self.arena.alloc(Expr {
                        kind: ExprKind::Index { target, index },
                        range: TextRange::new(range_start, self.peek_range_end()),
                    });
                }
                Token::KwAs => {
                    self.eat();
                    let ty = self.ty()?;
                    let ty_ref = self.arena.alloc(ty);
                    target = self.arena.alloc(Expr {
                        kind: ExprKind::Cast { target, ty: ty_ref },
                        range: TextRange::new(range_start, self.peek_range_end()),
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
        Ok(self.arena.alloc(if_))
    }

    fn else_branch(&mut self) -> Result<Option<Else<'ast>>, ParseError> {
        if self.try_eat(Token::KwElse) {
            match self.peek() {
                Token::KwIf => Ok(Some(Else::If {
                    else_if: self.if_()?,
                })),
                Token::BlockOpen => Ok(Some(Else::Block {
                    block: self.block()?,
                })),
                _ => return Err(ParseError::ElseMatch),
            }
        } else {
            Ok(None)
        }
    }

    fn block(&mut self) -> Result<&'ast Expr<'ast>, ParseError> {
        let range_start = self.peek_range_start();
        let stmts = self.block_stmts()?;
        Ok(self.arena.alloc(Expr {
            kind: ExprKind::Block { stmts },
            range: TextRange::new(range_start, self.peek_range_end()),
        }))
    }

    fn block_stmts(&mut self) -> Result<&'ast [Stmt<'ast>], ParseError> {
        let start = self.buf_stmts.start();
        self.expect(Token::BlockOpen, ParseCtx::Block)?;
        while !self.try_eat(Token::BlockClose) {
            let stmt = self.stmt()?;
            self.buf_stmts.add(stmt);
        }
        Ok(self.buf_stmts.take(start, self.arena))
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
                let range = self.peek_range();
                self.eat();
                let string = &self.source[range.as_usize()];
                let v = match string.parse::<u64>() {
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
                let range = self.peek_range();
                self.eat();
                let string = &self.source[range.as_usize()];
                let v = match string.parse::<f64>() {
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
    ) -> Result<&'ast [&'ast Expr<'ast>], ParseError> {
        let mut start = self.buf_exprs.start();
        if !self.try_eat(end) {
            loop {
                let expr: &Expr<'_> = self.expr()?;
                self.buf_exprs.add(expr);
                if !self.try_eat(Token::Comma) {
                    break;
                }
            }
            self.expect(end, ctx)?;
        }
        Ok(self.buf_exprs.take(start, self.arena))
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
