use super::*;
use crate::mem::*;
use std::path::PathBuf;

pub struct Parser {
    arena: Arena,
    peek_index: usize,
    tokens: Vec<TokenSpan>,
    sources: Vec<String>,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(1024 * 1024 * 4),
            peek_index: 0,
            tokens: Vec::new(),
            sources: Vec::new(),
        }
    }

    pub fn parse_package(&mut self) -> Result<P<Package>, ()> {
        let mut path = PathBuf::new();
        path.push("bin/src");
        path.push("main.txt");

        match std::fs::read_to_string(&path) {
            Ok(string) => {
                let mut package = self.arena.alloc::<Package>();
                package.root = self.parse_module(string)?;
                Ok(package)
            }
            Err(err) => {
                println!("file open error: {} path: {}", err, path.display());
                Err(())
            }
        }
    }

    fn err(&self, expected_kind: Token) -> Result<(), ()> {
        println!("error expected: {}", Token::as_str(expected_kind));
        let token = self.peek_token();
        if let Some(last_source) = self.sources.last() {
            if let Some(substring) =
                last_source.get((token.span.start - 20) as usize..(token.span.end + 20) as usize)
            {
                println!("source location: {}", substring);
            }
        }
        Err(())
    }

    fn peek(&self) -> Token {
        unsafe { self.tokens.get_unchecked(self.peek_index).token }
    }

    fn peek_next(&self, offset: usize) -> Token {
        unsafe { self.tokens.get_unchecked(self.peek_index + offset).token }
    }

    fn peek_token(&self) -> TokenSpan {
        unsafe { *self.tokens.get_unchecked(self.peek_index) }
    }

    fn consume(&mut self) {
        dbg!(self.tokens[self.peek_index].token);
        println!(
            "consume: peek_index = {} token count = {}",
            self.peek_index,
            self.tokens.len()
        );
        self.peek_index += 1;
    }

    fn try_consume(&mut self, kind: Token) -> bool {
        if kind == self.peek() {
            self.consume();
            return true;
        }
        false
    }

    fn try_consume_unary_op(&mut self) -> Option<UnaryOp> {
        let unary_op = self.peek().as_unary_op();
        if unary_op.is_some() {
            self.consume();
        }
        unary_op
    }

    fn try_consume_basic_type(&mut self) -> Option<BasicType> {
        let basic_type = self.peek().as_basic_type();
        if basic_type.is_some() {
            self.consume();
        }
        basic_type
    }

    fn expect_consume(&mut self, kind: Token) -> Result<(), ()> {
        if kind == self.peek() {
            self.consume();
            return Ok(());
        }
        self.err(kind)
    }

    fn expect_consume_basic_type(&mut self) -> Result<BasicType, ()> {
        if let Some(basic_type) = self.peek().as_basic_type() {
            self.consume();
            return Ok(basic_type);
        }
        println!("expected 'basic type'");
        Err(())
    }

    fn parse_ident(&mut self) -> Result<Ident, ()> {
        let token = self.peek_token();
        if token.token == Token::Ident {
            self.consume();
            return Ok(Ident { span: token.span });
        }
        println!("error expected: identifier");
        let token = self.peek_token();
        if let Some(last_source) = self.sources.last() {
            if let Some(substring) =
                last_source.get((token.span.start - 20) as usize..(token.span.end + 20) as usize)
            {
                println!("source location: {}", substring);
            }
        }
        Err(())
    }

    fn parse_module(&mut self, source: String) -> Result<P<Module>, ()> {
        self.peek_index = 0;
        self.tokens = Lexer::new(&source).lex();
        self.sources.push(source);

        let mut module = self.arena.alloc::<Module>();
        module.source = (self.sources.len() - 1) as u32;

        while self.peek() != Token::Eof {
            let decl = self.parse_decl()?;
            module.decls.add(&mut self.arena, decl);
        }
        Ok(module)
    }

    fn parse_module_access(&mut self) -> Option<P<ModuleAccess>> {
        if self.peek() != Token::Ident && self.peek_next(1) != Token::ColonColon {
            return None;
        }
        let mut module_access = self.arena.alloc::<ModuleAccess>();
        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = unsafe { self.parse_ident().unwrap_unchecked() };
            self.consume();
            module_access.names.add(&mut self.arena, name);
        }
        Some(module_access)
    }

    fn parse_type(&mut self) -> Result<tt::Type, ()> {
        let mut tt = tt::Type {
            pointer_level: 0,
            kind: tt::TypeKind::Basic(BasicType::S8),
        };
        while self.try_consume(Token::Times) {
            tt.pointer_level += 1;
        }
        if let Some(basic_type) = self.try_consume_basic_type() {
            tt.kind = tt::TypeKind::Basic(basic_type);
            return Ok(tt);
        }
        match self.peek() {
            Token::Ident => {
                let custom = self.parse_type_custom()?;
                tt.kind = tt::TypeKind::Custom(custom);
                Ok(tt)
            }
            Token::OpenBracket => {
                let static_array = self.parse_type_static_array()?;
                tt.kind = tt::TypeKind::StaticArray(static_array);
                Ok(tt)
            }
            _ => Err(()),
        }
    }

    fn parse_type_custom(&mut self) -> Result<P<tt::Custom>, ()> {
        let mut custom = self.arena.alloc::<tt::Custom>();
        custom.module_access = self.parse_module_access();
        custom.name = self.parse_ident()?;
        Ok(custom)
    }

    fn parse_type_static_array(&mut self) -> Result<P<tt::StaticArray>, ()> {
        let mut static_array = self.arena.alloc::<tt::StaticArray>();
        self.expect_consume(Token::OpenBracket)?;
        static_array.constexpr_size = self.parse_sub_expr(0)?;
        self.expect_consume(Token::CloseBracket)?;
        static_array.element = self.parse_type()?;
        Ok(static_array)
    }

    fn parse_decl(&mut self) -> Result<decl::Decl, ()> {
        match self.peek() {
            Token::KwMod => Ok(decl::Decl::Mod(self.parse_decl_mod()?)),
            Token::KwImpl => Ok(decl::Decl::Impl(self.parse_decl_impl()?)),
            Token::KwImport => Ok(decl::Decl::Import(self.parse_decl_import()?)),
            Token::Ident | Token::KwPub => {
                if self.peek_next(1) == Token::KwMod {
                    return Ok(decl::Decl::Mod(self.parse_decl_mod()?));
                }

                let mut offset = 0;
                if self.peek() == Token::KwPub {
                    offset += 1;
                }
                if self.peek_next(offset) != Token::Ident {
                    return Err(());
                }
                if self.peek_next(offset + 1) != Token::ColonColon {
                    return Err(());
                }

                match self.peek_next(offset + 2) {
                    Token::OpenParen => Ok(decl::Decl::Proc(self.parse_decl_proc()?)),
                    Token::KwEnum => Ok(decl::Decl::Enum(self.parse_decl_enum()?)),
                    Token::KwStruct => Ok(decl::Decl::Struct(self.parse_decl_struct()?)),
                    _ => Ok(decl::Decl::Global(self.parse_decl_global()?)),
                }
            }
            _ => Err(()),
        }
    }

    fn parse_decl_mod(&mut self) -> Result<P<decl::Mod>, ()> {
        let mut mod_decl = self.arena.alloc::<decl::Mod>();
        mod_decl.is_pub = self.try_consume(Token::KwPub);
        self.expect_consume(Token::KwMod)?;
        self.parse_ident()?;
        self.expect_consume(Token::Semicolon)?;
        Ok(mod_decl)
    }

    fn parse_decl_impl(&mut self) -> Result<P<decl::Impl>, ()> {
        todo!()
    }

    fn parse_decl_import(&mut self) -> Result<P<decl::Import>, ()> {
        todo!()
    }

    fn parse_import_target(&mut self) -> Result<decl::ImportTarget, ()> {
        todo!()
    }

    fn parse_decl_proc(&mut self) -> Result<P<decl::Proc>, ()> {
        let mut proc_decl = self.arena.alloc::<decl::Proc>();
        proc_decl.is_pub = self.try_consume(Token::KwPub);
        proc_decl.name = self.parse_ident()?;
        self.expect_consume(Token::ColonColon)?;
        self.expect_consume(Token::OpenParen)?;
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
            self.expect_consume(Token::CloseParen)?;
        }
        if self.try_consume(Token::ArrowThin) {
            proc_decl.return_type = Some(self.parse_type()?);
        }
        if self.try_consume(Token::At) {
            proc_decl.is_external = true;
        } else {
            proc_decl.block = self.parse_stmt_block(false)?;
        }
        Ok(proc_decl)
    }

    fn parse_proc_param(&mut self) -> Result<decl::ProcParam, ()> {
        let is_mut = self.try_consume(Token::KwMut);

        let token = self.peek_token();
        if token.token == Token::KwSelf {
            self.consume();
            let param = decl::ProcParam {
                is_mut,
                kind: decl::ParamKind::SelfParam(Ident { span: token.span }),
            };
            return Ok(param);
        }
        let name = self.parse_ident()?;
        self.expect_consume(Token::Colon)?;
        let tt = self.parse_type()?;
        let param = decl::ProcParam {
            is_mut,
            kind: decl::ParamKind::Normal(decl::ParamNormal { name, tt }),
        };
        Ok(param)
    }

    fn parse_decl_enum(&mut self) -> Result<P<decl::Enum>, ()> {
        let mut enum_decl = self.arena.alloc::<decl::Enum>();
        enum_decl.is_pub = self.try_consume(Token::KwPub);
        enum_decl.name = self.parse_ident()?;
        self.expect_consume(Token::ColonColon)?;
        self.expect_consume(Token::KwEnum)?;
        enum_decl.basic_type = self.try_consume_basic_type();
        self.expect_consume(Token::OpenBlock)?;
        while !self.try_consume(Token::CloseBlock) {
            let variant = self.parse_enum_variant()?;
            enum_decl.variants.add(&mut self.arena, variant);
        }
        Ok(enum_decl)
    }

    fn parse_enum_variant(&mut self) -> Result<decl::EnumVariant, ()> {
        let name = self.parse_ident()?;
        self.expect_consume(Token::Assign)?;
        let constexpr = self.parse_expr()?;
        Ok(decl::EnumVariant { name, constexpr })
    }

    fn parse_decl_struct(&mut self) -> Result<P<decl::Struct>, ()> {
        let mut struct_decl = self.arena.alloc::<decl::Struct>();
        struct_decl.is_pub = self.try_consume(Token::KwPub);
        struct_decl.name = self.parse_ident()?;
        self.expect_consume(Token::ColonColon)?;
        self.expect_consume(Token::KwStruct)?;
        self.expect_consume(Token::OpenBlock)?;
        while !self.try_consume(Token::CloseBlock) {
            let field = self.parse_struct_field()?;
            struct_decl.fields.add(&mut self.arena, field);
        }
        Ok(struct_decl)
    }

    fn parse_struct_field(&mut self) -> Result<decl::StructField, ()> {
        let is_pub = self.try_consume(Token::KwPub);
        let name = self.parse_ident()?;
        self.expect_consume(Token::Colon)?;
        let tt = self.parse_type()?;
        let mut default_expr = None;
        if self.try_consume(Token::Assign) {
            default_expr = Some(self.parse_expr()?);
        } else {
            self.expect_consume(Token::Semicolon)?;
        }
        Ok(decl::StructField {
            is_pub,
            name,
            tt,
            default_expr,
        })
    }

    fn parse_decl_global(&mut self) -> Result<P<decl::Global>, ()> {
        let mut global_decl = self.arena.alloc::<decl::Global>();
        global_decl.is_pub = self.try_consume(Token::KwPub);
        global_decl.name = self.parse_ident()?;
        self.expect_consume(Token::ColonColon)?;
        global_decl.constexpr = self.parse_expr()?;
        Ok(global_decl)
    }

    fn parse_stmt(&mut self) -> Result<stmt::Stmt, ()> {
        match self.peek() {
            Token::KwIf => Ok(stmt::Stmt::If(self.parse_stmt_if()?)),
            Token::KwFor => Ok(stmt::Stmt::For(self.parse_stmt_for()?)),
            Token::OpenBlock => Ok(stmt::Stmt::Block(self.parse_stmt_block(false)?)),
            Token::KwDefer => Ok(stmt::Stmt::Defer(self.parse_stmt_defer()?)),
            Token::KwBreak => {
                self.parse_stmt_break()?;
                Ok(stmt::Stmt::Break)
            }
            Token::KwReturn => Ok(stmt::Stmt::Return(self.parse_stmt_return()?)),
            Token::KwContinue => {
                self.parse_stmt_continue()?;
                Ok(stmt::Stmt::Continue)
            }
            _ => {
                if self.peek() == Token::KwMut
                    || (self.peek() == Token::Ident && self.peek_next(1) == Token::Colon)
                {
                    return Ok(stmt::Stmt::VarDecl(self.parse_stmt_var_decl()?));
                }

                let module_access = self.parse_module_access();
                let something = self.parse_expr_something(module_access)?;

                if self.try_consume(Token::Semicolon) {
                    return Ok(stmt::Stmt::ProcCall(something));
                } else {
                    let mut var_assign = self.arena.alloc::<stmt::VarAssign>();
                    var_assign.something = something;
                    self.expect_consume(Token::Assign)?; //support assign ops
                    var_assign.expr = self.parse_expr()?;
                    return Ok(stmt::Stmt::VarAssign(var_assign));
                }
            }
        }
    }

    fn parse_stmt_if(&mut self) -> Result<P<stmt::If>, ()> {
        let mut if_stmt = self.arena.alloc::<stmt::If>();
        self.expect_consume(Token::KwIf)?;
        if_stmt.condition = self.parse_expr()?;
        if_stmt.block = self.parse_stmt_block(false)?;
        if_stmt.else_ = self.parse_else()?;
        Ok(if_stmt)
    }

    fn parse_else(&mut self) -> Result<Option<stmt::Else>, ()> {
        if !self.try_consume(Token::KwElse) {
            return Ok(None);
        }
        match self.peek() {
            Token::OpenBlock => {
                let block = self.parse_stmt_block(false)?;
                Ok(Some(stmt::Else::Block(block)))
            }
            _ => {
                let if_stmt = self.parse_stmt_if()?;
                Ok(Some(stmt::Else::If(if_stmt)))
            }
        }
    }

    fn parse_stmt_for(&mut self) -> Result<P<stmt::For>, ()> {
        todo!()
    }

    fn parse_stmt_block(&mut self, allow_short: bool) -> Result<P<stmt::Block>, ()> {
        let mut block_stmt = self.arena.alloc::<stmt::Block>();

        if allow_short && self.peek() != Token::OpenBlock {
            let stmt = self.parse_stmt()?;
            block_stmt.is_short = true;
            block_stmt.stmts.add(&mut self.arena, stmt);
            return Ok(block_stmt);
        }

        block_stmt.is_short = false;
        self.expect_consume(Token::OpenBlock)?;
        while !self.try_consume(Token::CloseBlock) {
            let stmt = self.parse_stmt()?;
            block_stmt.stmts.add(&mut self.arena, stmt);
        }
        Ok(block_stmt)
    }

    fn parse_stmt_defer(&mut self) -> Result<P<stmt::Block>, ()> {
        self.expect_consume(Token::KwDefer)?;
        Ok(self.parse_stmt_block(true)?)
    }

    fn parse_stmt_break(&mut self) -> Result<(), ()> {
        self.expect_consume(Token::KwBreak)?;
        self.expect_consume(Token::Semicolon)?;
        Ok(())
    }

    fn parse_stmt_return(&mut self) -> Result<P<stmt::Return>, ()> {
        let mut return_stmt = self.arena.alloc::<stmt::Return>();
        self.expect_consume(Token::KwReturn)?;
        return_stmt.expr = match self.peek() {
            Token::Semicolon => {
                self.consume();
                None
            }
            _ => Some(self.parse_expr()?),
        };
        Ok(return_stmt)
    }

    fn parse_stmt_continue(&mut self) -> Result<(), ()> {
        self.expect_consume(Token::KwContinue)?;
        self.expect_consume(Token::Semicolon)?;
        Ok(())
    }

    fn parse_stmt_var_decl(&mut self) -> Result<P<stmt::VarDecl>, ()> {
        let mut var_decl = self.arena.alloc::<stmt::VarDecl>();
        var_decl.is_mut = self.try_consume(Token::KwMut);
        var_decl.name = self.parse_ident()?;
        self.expect_consume(Token::Colon)?;
        let infer_type = self.try_consume(Token::Assign);
        if !infer_type {
            var_decl.tt = Some(self.parse_type()?);
            if self.try_consume(Token::Semicolon) {
                return Ok(var_decl);
            }
            self.expect_consume(Token::Assign)?;
        }
        var_decl.expr = Some(self.parse_expr()?);
        Ok(var_decl)
    }

    fn parse_stmt_var_assign(&mut self) -> Result<P<stmt::VarAssign>, ()> {
        todo!()
    }

    fn parse_stmt_proc_call(&mut self) -> Result<P<Something>, ()> {
        todo!()
    }

    fn parse_expr(&mut self) -> Result<P<expr::Expr>, ()> {
        let expr = self.parse_sub_expr(0)?;
        self.expect_consume(Token::Semicolon)?;
        Ok(expr)
    }

    fn parse_sub_expr(&mut self, min_prec: u32) -> Result<P<expr::Expr>, ()> {
        println!("parse_sub_expr");
        let mut expr_lhs = self.parse_primary_expr()?;
        loop {
            println!("parse_sub_expr loop");
            let prec: u32;
            let binary_op: BinaryOp;
            if let Some(op) = self.peek().as_binary_op() {
                binary_op = op;
                prec = op.precedence();
                if prec < min_prec {
                    break;
                }
                self.consume();
            } else {
                break;
            }
            let expr_rhs = self.parse_sub_expr(prec + 1)?;
            let mut expr_lhs_copy = self.arena.alloc::<expr::Expr>();
            *expr_lhs_copy = *expr_lhs;

            let mut bin_expr = self.arena.alloc::<expr::Binary>();
            bin_expr.op = binary_op;
            bin_expr.lhs = expr_lhs_copy;
            bin_expr.rhs = expr_rhs;

            *expr_lhs = expr::Expr::Binary(bin_expr);
        }
        Ok(expr_lhs)
    }

    fn parse_primary_expr(&mut self) -> Result<P<expr::Expr>, ()> {
        println!("parse_primary_expr");
        if self.try_consume(Token::OpenParen) {
            let expr = self.parse_sub_expr(0)?;
            self.expect_consume(Token::CloseParen)?;
            return Ok(expr);
        }

        if let Some(unary_op) = self.try_consume_unary_op() {
            let rhs = self.parse_primary_expr()?;
            let mut unary_expr = self.arena.alloc::<expr::Unary>();
            unary_expr.op = unary_op;
            unary_expr.rhs = rhs;

            let mut expr = self.arena.alloc::<expr::Expr>();
            *expr = expr::Expr::Unary(unary_expr);
            return Ok(expr);
        }

        let mut expr = self.arena.alloc::<expr::Expr>();
        match self.peek() {
            Token::KwCast => {
                *expr = expr::Expr::Cast(self.parse_expr_cast()?);
            }
            Token::KwSizeof => {
                *expr = expr::Expr::Sizeof(self.parse_expr_sizeof()?);
            }
            Token::LitInt(..) | Token::LitFloat(..) | Token::LitBool(..) | Token::LitString => {
                *expr = expr::Expr::Literal(self.parse_expr_literal()?);
            }
            Token::OpenBlock | Token::OpenBracket => {
                *expr = expr::Expr::ArrayInit(self.parse_expr_array_init()?);
            }
            _ => {
                if self.peek() == Token::Dot
                    && self.peek_next(1) != Token::OpenBlock
                    && self.peek_next(2) != Token::OpenBlock
                {
                    *expr = expr::Expr::Enum(self.parse_expr_enum()?);
                    return Ok(expr);
                }

                let module_access = self.parse_module_access();
                if (self.peek() == Token::Dot && self.peek_next(1) == Token::OpenBlock)
                    || (self.peek() == Token::Ident
                        && self.peek_next(1) == Token::Dot
                        && self.peek_next(2) == Token::OpenBlock)
                {
                    *expr = expr::Expr::StructInit(self.parse_expr_struct_init(module_access)?);
                    return Ok(expr);
                }

                *expr = expr::Expr::Something(self.parse_expr_something(module_access)?);
                return Ok(expr);
            }
        }
        Ok(expr)
    }

    fn parse_expr_list(&mut self, start: Token, end: Token) -> Result<List<P<expr::Expr>>, ()> {
        println!("parse_expr_list");
        let mut expr_list = List::new();
        self.expect_consume(start)?;
        if self.try_consume(end) {
            return Ok(expr_list);
        }
        loop {
            let expr = self.parse_sub_expr(0)?;
            expr_list.add(&mut self.arena, expr);
            if !self.try_consume(Token::Comma) {
                break;
            }
        }
        self.expect_consume(end)?;
        Ok(expr_list)
    }

    fn parse_expr_enum(&mut self) -> Result<expr::Enum, ()> {
        self.expect_consume(Token::Dot)?;
        let variant_name = self.parse_ident()?;
        Ok(expr::Enum { variant_name })
    }

    fn parse_expr_cast(&mut self) -> Result<P<expr::Cast>, ()> {
        let mut cast_expr = self.arena.alloc::<expr::Cast>();
        self.expect_consume(Token::KwCast)?;
        self.expect_consume(Token::OpenParen)?;
        cast_expr.into = self.expect_consume_basic_type()?;
        self.expect_consume(Token::Comma)?;
        cast_expr.expr = self.parse_sub_expr(0)?;
        self.expect_consume(Token::CloseParen)?;
        Ok(cast_expr)
    }

    fn parse_expr_sizeof(&mut self) -> Result<P<expr::Sizeof>, ()> {
        let mut sizeof_expr = self.arena.alloc::<expr::Sizeof>();
        self.expect_consume(Token::KwSizeof)?;
        self.expect_consume(Token::OpenParen)?;
        sizeof_expr.tt = self.parse_type()?;
        self.expect_consume(Token::CloseParen)?;
        Ok(sizeof_expr)
    }

    fn parse_expr_literal(&mut self) -> Result<P<expr::Literal>, ()> {
        let mut literal_expr = self.arena.alloc::<expr::Literal>();
        match self.peek() {
            Token::LitNull => {
                *literal_expr = expr::Literal::Null;
            }
            Token::LitInt(u) => {
                *literal_expr = expr::Literal::Uint(u);
            }
            Token::LitFloat(f) => {
                *literal_expr = expr::Literal::Float(f);
            }
            Token::LitBool(b) => {
                *literal_expr = expr::Literal::Bool(b);
            }
            Token::LitString => {
                *literal_expr = expr::Literal::String;
            }
            _ => {
                println!("expected int, float, bool or string literal in expression");
                return Err(());
            }
        }
        self.consume();
        Ok(literal_expr)
    }

    fn parse_expr_array_init(&mut self) -> Result<P<expr::ArrayInit>, ()> {
        let mut array_init = self.arena.alloc::<expr::ArrayInit>();
        if self.peek() == Token::OpenBracket {
            array_init.tt = Some(self.parse_type()?);
        }
        array_init.input = self.parse_expr_list(Token::OpenBlock, Token::CloseBlock)?;
        Ok(array_init)
    }

    fn parse_expr_struct_init(
        &mut self,
        module_access: Option<P<ModuleAccess>>,
    ) -> Result<P<expr::StructInit>, ()> {
        let mut struct_init = self.arena.alloc::<expr::StructInit>();
        struct_init.module_access = module_access;
        if self.peek() == Token::Ident {
            struct_init.struct_name = Some(self.parse_ident()?);
        }
        self.expect_consume(Token::Dot)?;
        struct_init.input = self.parse_expr_list(Token::OpenBlock, Token::CloseBlock)?;
        Ok(struct_init)
    }

    fn parse_expr_something(
        &mut self,
        module_access: Option<P<ModuleAccess>>,
    ) -> Result<P<Something>, ()> {
        let mut something = self.arena.alloc::<Something>();
        something.module_access = module_access;
        something.access = self.parse_access_first()?;
        self.parse_access(something.access)?;
        Ok(something)
    }

    fn parse_access_first(&mut self) -> Result<P<Access>, ()> {
        println!("parse_access_first");
        let mut access = self.arena.alloc::<Access>();
        let ident = self.parse_ident()?;
        if self.peek() == Token::OpenParen {
            let input = self.parse_expr_list(Token::OpenParen, Token::CloseParen)?;
            let mut access_call = self.arena.alloc::<AccessCall>();
            access_call.name = ident;
            access_call.input = input;
            access.kind = AccessKind::Call(access_call);
        } else {
            access.kind = AccessKind::Ident(ident);
        }
        Ok(access)
    }

    fn parse_access(&mut self, mut prev: P<Access>) -> Result<(), ()> {
        println!("parse access");
        match self.peek() {
            Token::Dot | Token::OpenBracket => {}
            _ => {
                return Ok(());
            }
        }
        let mut access = self.arena.alloc::<Access>();

        match self.peek() {
            Token::Dot => {
                self.consume();
                let ident = self.parse_ident()?;
                if self.peek() == Token::OpenParen {
                    let input = self.parse_expr_list(Token::OpenParen, Token::CloseParen)?;
                    let mut access_call = self.arena.alloc::<AccessCall>();
                    access_call.name = ident;
                    access_call.input = input;
                    access.kind = AccessKind::Call(access_call);
                } else {
                    access.kind = AccessKind::Ident(ident);
                }
            }
            Token::OpenBracket => {
                self.consume();
                let index_expr = self.parse_sub_expr(0)?;
                access.kind = AccessKind::Array(index_expr);
                self.expect_consume(Token::CloseBracket)?;
            }
            _ => {}
        }
        prev.next = Some(access);
        self.parse_access(access)?;
        Ok(())
    }
}

impl BinaryOp {
    pub fn precedence(&self) -> u32 {
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
            Token::PlusEq => Some(AssignOp::Plus),
            Token::MinusEq => Some(AssignOp::Minus),
            Token::TimesEq => Some(AssignOp::Times),
            Token::DivEq => Some(AssignOp::Div),
            Token::ModEq => Some(AssignOp::Mod),
            Token::BitAndEq => Some(AssignOp::BitAnd),
            Token::BitOrEq => Some(AssignOp::BitOr),
            Token::BitXorEq => Some(AssignOp::BitXor),
            Token::ShlEq => Some(AssignOp::Shl),
            Token::ShrEq => Some(AssignOp::Shr),
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
            _ => None,
        }
    }
}