use crate::ast::decl::ProcParam;
use crate::ast::*;
use crate::lexer::*;
use crate::ptr::*;
use crate::token::*;
use std::path::PathBuf;

pub struct Parser {
    arena: Arena,
    peek_index: usize,
    tokens: Vec<Token>,
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

    fn err(&self, expected_kind: TokenKind) -> Result<(), ()> {
        println!("error expected: {}", expected_kind.as_str());
        Err(())
    }

    fn peek(&self) -> TokenKind {
        unsafe { self.tokens.get_unchecked(self.peek_index).kind }
    }

    fn peek_next(&self, offset: usize) -> TokenKind {
        unsafe { self.tokens.get_unchecked(self.peek_index + offset).kind }
    }

    fn peek_token(&self) -> Token {
        unsafe { *self.tokens.get_unchecked(self.peek_index) }
    }

    fn consume(&mut self) {
        self.peek_index += 1;
    }

    fn try_consume(&mut self, kind: TokenKind) -> bool {
        if kind == self.peek() {
            self.consume();
            return true;
        }
        false
    }

    fn try_consume_unary_op(&mut self) -> Option<UnaryOp> {
        self.peek().as_unary_op()
    }

    fn try_consume_binary_op(&mut self) -> Option<BinaryOp> {
        self.peek().as_binary_op()
    }

    fn try_consume_assign_op(&mut self) -> Option<AssignOp> {
        self.peek().as_assign_op()
    }

    fn try_consume_basic_type(&mut self) -> Option<BasicType> {
        self.peek().as_basic_type()
    }

    fn expect_consume(&mut self, kind: TokenKind) -> Result<(), ()> {
        if kind == self.peek() {
            self.consume();
            return Ok(());
        }
        self.err(kind)
    }

    fn expect_consume_basic_type(&mut self) -> Result<BasicType, ()> {
        if let Some(basic_type) = self.peek().as_basic_type() {
            return Ok(basic_type);
        }
        println!("expected 'basic type'");
        Err(())
    }

    fn parse_ident(&mut self) -> Result<Ident, ()> {
        let token = self.peek_token();
        if token.kind == TokenKind::Ident {
            self.consume();
            return Ok(Ident { span: token.span });
        }
        println!("expected 'identifier'");
        Err(())
    }

    fn parse_module(&mut self, source: String) -> Result<P<Module>, ()> {
        self.peek_index = 0;
        self.tokens = Lexer::new(&source).lex();
        self.sources.push(source);

        let mut module = self.arena.alloc::<Module>();
        module.source = (self.sources.len() - 1) as u32;

        while self.peek() != TokenKind::Eof {
            let decl = self.parse_decl()?;
            println!("decl parsed");
            module.decls.add(&mut self.arena, decl);
        }
        Ok(module)
    }

    fn parse_module_access(&mut self) -> Option<P<ModuleAccess>> {
        if self.peek() != TokenKind::Ident && self.peek_next(1) != TokenKind::ColonColon {
            return None;
        }
        let mut module_access = self.arena.alloc::<ModuleAccess>();
        while self.peek() == TokenKind::Ident && self.peek_next(1) == TokenKind::ColonColon {
            let name = unsafe { self.parse_ident().unwrap_unchecked() };
            module_access.names.add(&mut self.arena, name);
        }
        Some(module_access)
    }

    fn parse_type(&mut self) -> Result<tt::Type, ()> {
        let mut tt = tt::Type {
            pointer_level: 0,
            kind: tt::TypeKind::Basic(BasicType::S8),
        };
        while self.try_consume(TokenKind::Times) {
            tt.pointer_level += 1;
        }
        if let Some(basic_type) = self.try_consume_basic_type() {
            tt.kind = tt::TypeKind::Basic(basic_type);
            return Ok(tt);
        }
        match self.peek() {
            TokenKind::Ident => {
                let custom = self.parse_type_custom()?;
                tt.kind = tt::TypeKind::Custom(custom);
                Ok(tt)
            }
            TokenKind::OpenBracket => {
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
        self.expect_consume(TokenKind::OpenBracket)?;
        static_array.constexpr_size = self.parse_expr()?;
        self.expect_consume(TokenKind::CloseBracket)?;
        static_array.element = self.parse_type()?;
        Ok(static_array)
    }

    fn parse_decl(&mut self) -> Result<decl::Decl, ()> {
        match self.peek() {
            TokenKind::KwMod => Ok(decl::Decl::Mod(self.parse_decl_mod()?)),
            TokenKind::KwImpl => Ok(decl::Decl::Impl(self.parse_decl_impl()?)),
            TokenKind::KwImport => Ok(decl::Decl::Import(self.parse_decl_import()?)),
            TokenKind::Ident | TokenKind::KwPub => {
                if (self.peek_next(1) == TokenKind::KwMod) {
                    return Ok(decl::Decl::Mod(self.parse_decl_mod()?));
                }

                let mut offset = 0;
                if self.peek() == TokenKind::KwPub {
                    offset += 1;
                }
                if self.peek_next(offset) != TokenKind::Ident {
                    return Err(());
                }
                if self.peek_next(offset + 1) != TokenKind::ColonColon {
                    return Err(());
                }

                match self.peek_next(offset + 2) {
                    TokenKind::OpenParen => Ok(decl::Decl::Proc(self.parse_decl_proc()?)),
                    TokenKind::KwEnum => Ok(decl::Decl::Enum(self.parse_decl_enum()?)),
                    TokenKind::KwStruct => Ok(decl::Decl::Struct(self.parse_decl_struct()?)),
                    _ => Ok(decl::Decl::Global(self.parse_decl_global()?)),
                }
            }
            _ => Err(()),
        }
    }

    fn parse_decl_mod(&mut self) -> Result<P<decl::Mod>, ()> {
        let mut mod_decl = self.arena.alloc::<decl::Mod>();
        mod_decl.is_pub = self.try_consume(TokenKind::KwPub);
        self.expect_consume(TokenKind::KwMod)?;
        self.parse_ident()?;
        self.expect_consume(TokenKind::Semicolon)?;
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
        proc_decl.is_pub = self.try_consume(TokenKind::KwPub);
        proc_decl.name = self.parse_ident()?;
        self.expect_consume(TokenKind::ColonColon)?;
        self.expect_consume(TokenKind::OpenParen)?;
        if !self.try_consume(TokenKind::CloseParen) {
            loop {
                if self.try_consume(TokenKind::DotDot) {
                    proc_decl.is_variadic = true;
                    break;
                }
                let param = self.parse_proc_param()?;
                proc_decl.params.add(&mut self.arena, param);
                if !self.try_consume(TokenKind::Comma) {
                    break;
                }
            }
            self.expect_consume(TokenKind::CloseParen)?;
        }
        if self.try_consume(TokenKind::ArrowThin) {
            proc_decl.return_type = Some(self.parse_type()?);
        }
        if self.try_consume(TokenKind::At) {
            proc_decl.is_external = true;
        } else {
            proc_decl.block = self.parse_stmt_block(false)?;
        }
        Ok(proc_decl)
    }

    fn parse_proc_param(&mut self) -> Result<decl::ProcParam, ()> {
        let is_mut = self.try_consume(TokenKind::KwMut);

        let token = self.peek_token();
        if token.kind == TokenKind::KwSelf {
            self.consume();
            let param = ProcParam {
                is_mut,
                kind: decl::ParamKind::SelfParam(Ident { span: token.span }),
            };
            return Ok(param);
        }
        let name = self.parse_ident()?;
        self.expect_consume(TokenKind::Colon)?;
        let tt = self.parse_type()?;
        let param = ProcParam {
            is_mut,
            kind: decl::ParamKind::Normal(decl::ParamNormal { name, tt }),
        };
        Ok(param)
    }

    fn parse_decl_enum(&mut self) -> Result<P<decl::Enum>, ()> {
        let mut enum_decl = self.arena.alloc::<decl::Enum>();
        enum_decl.is_pub = self.try_consume(TokenKind::KwPub);
        enum_decl.name = self.parse_ident()?;
        self.expect_consume(TokenKind::ColonColon)?;
        self.expect_consume(TokenKind::KwEnum)?;
        enum_decl.basic_type = self.try_consume_basic_type();
        self.expect_consume(TokenKind::OpenBlock)?;
        while !self.try_consume(TokenKind::CloseBlock) {
            let variant = self.parse_enum_variant()?;
            enum_decl.variants.add(&mut self.arena, variant);
        }
        Ok(enum_decl)
    }

    fn parse_enum_variant(&mut self) -> Result<decl::EnumVariant, ()> {
        todo!();
    }

    fn parse_decl_struct(&mut self) -> Result<P<decl::Struct>, ()> {
        let mut struct_decl = self.arena.alloc::<decl::Struct>();
        struct_decl.is_pub = self.try_consume(TokenKind::KwPub);
        struct_decl.name = self.parse_ident()?;
        self.expect_consume(TokenKind::ColonColon)?;
        self.expect_consume(TokenKind::KwStruct)?;
        self.expect_consume(TokenKind::OpenBlock)?;
        while !self.try_consume(TokenKind::CloseBlock) {
            let field = self.parse_struct_field()?;
            struct_decl.fields.add(&mut self.arena, field);
        }
        Ok(struct_decl)
    }

    fn parse_struct_field(&mut self) -> Result<decl::StructField, ()> {
        todo!();
    }

    fn parse_decl_global(&mut self) -> Result<P<decl::Global>, ()> {
        todo!();
    }

    fn parse_stmt(&mut self) -> Result<stmt::Stmt, ()> {
        match self.peek() {
            TokenKind::KwIf => Ok(stmt::Stmt::If(self.parse_stmt_if()?)),
            TokenKind::KwFor => Ok(stmt::Stmt::For(self.parse_stmt_for()?)),
            TokenKind::OpenBlock => Ok(stmt::Stmt::Block(self.parse_stmt_block(false)?)),
            TokenKind::KwDefer => Ok(stmt::Stmt::Defer(self.parse_stmt_defer()?)),
            TokenKind::KwBreak => {
                self.parse_stmt_break()?;
                Ok(stmt::Stmt::Break)
            }
            TokenKind::KwReturn => Ok(stmt::Stmt::Return(self.parse_stmt_return()?)),
            TokenKind::KwContinue => {
                self.parse_stmt_continue()?;
                Ok(stmt::Stmt::Continue)
            }
            _ => {
                println!("invalid token while parsing a statement");
                Err(())
            }
        }
    }

    fn parse_stmt_if(&mut self) -> Result<P<stmt::If>, ()> {
        let mut if_stmt = self.arena.alloc::<stmt::If>();
        self.expect_consume(TokenKind::KwIf)?;
        if_stmt.condition = self.parse_expr()?;
        if_stmt.block = self.parse_stmt_block(false)?;
        if_stmt.else_ = self.parse_else()?;
        Ok(if_stmt)
    }

    fn parse_else(&mut self) -> Result<Option<stmt::Else>, ()> {
        if !self.try_consume(TokenKind::KwElse) {
            return Ok(None);
        }
        match self.peek() {
            TokenKind::OpenBlock => {
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

        if allow_short && self.peek() != TokenKind::OpenBlock {
            let stmt = self.parse_stmt()?;
            block_stmt.is_short = true;
            block_stmt.stmts.add(&mut self.arena, stmt);
            return Ok(block_stmt);
        }

        block_stmt.is_short = false;
        self.expect_consume(TokenKind::OpenBlock)?;
        while !self.try_consume(TokenKind::CloseBlock) {
            let stmt = self.parse_stmt()?;
            block_stmt.stmts.add(&mut self.arena, stmt);
        }
        Ok(block_stmt)
    }

    fn parse_stmt_defer(&mut self) -> Result<P<stmt::Block>, ()> {
        self.expect_consume(TokenKind::KwDefer)?;
        Ok(self.parse_stmt_block(true)?)
    }

    fn parse_stmt_break(&mut self) -> Result<(), ()> {
        self.expect_consume(TokenKind::KwBreak)?;
        self.expect_consume(TokenKind::Semicolon)?;
        Ok(())
    }

    fn parse_stmt_return(&mut self) -> Result<P<stmt::Return>, ()> {
        let mut return_stmt = self.arena.alloc::<stmt::Return>();
        self.expect_consume(TokenKind::KwReturn)?;
        return_stmt.expr = match self.peek() {
            TokenKind::Semicolon => None,
            _ => Some(self.parse_expr()?),
        };
        self.expect_consume(TokenKind::Semicolon)?;
        Ok(return_stmt)
    }

    fn parse_stmt_continue(&mut self) -> Result<(), ()> {
        self.expect_consume(TokenKind::KwContinue)?;
        self.expect_consume(TokenKind::Semicolon)?;
        Ok(())
    }

    fn parse_stmt_var_decl(&mut self) -> Result<P<stmt::VarDecl>, ()> {
        todo!()
    }

    fn parse_stmt_var_assign(&mut self) -> Result<P<stmt::VarAssign>, ()> {
        todo!()
    }

    fn parse_stmt_proc_call(&mut self) -> Result<P<Something>, ()> {
        todo!()
    }

    fn parse_expr(&mut self) -> Result<P<expr::Expr>, ()> {
        todo!()
    }

    fn parse_expr_enum(&mut self) -> Result<expr::Enum, ()> {
        self.expect_consume(TokenKind::Dot)?;
        let variant_name = self.parse_ident()?;
        Ok(expr::Enum { variant_name })
    }

    fn parse_expr_cast(&mut self) -> Result<P<expr::Cast>, ()> {
        let mut cast_expr = self.arena.alloc::<expr::Cast>();
        self.expect_consume(TokenKind::KwCast)?;
        self.expect_consume(TokenKind::OpenParen)?;
        cast_expr.into = self.expect_consume_basic_type()?;
        self.expect_consume(TokenKind::Comma)?;
        cast_expr.expr = self.parse_expr()?;
        self.expect_consume(TokenKind::CloseParen)?;
        Ok(cast_expr)
    }

    fn parse_expr_sizeof(&mut self) -> Result<P<expr::Sizeof>, ()> {
        let mut sizeof_expr = self.arena.alloc::<expr::Sizeof>();
        self.expect_consume(TokenKind::KwSizeof)?;
        self.expect_consume(TokenKind::OpenParen)?;
        sizeof_expr.tt = self.parse_type()?;
        self.expect_consume(TokenKind::CloseParen)?;
        Ok(sizeof_expr)
    }

    fn parse_expr_literal(&mut self) -> Result<P<expr::Literal>, ()> {
        todo!()
    }

    fn parse_expr_array_init(&mut self) -> Result<P<expr::ArrayInit>, ()> {
        todo!()
    }

    fn parse_expr_struct_init(&mut self) -> Result<P<expr::StructInit>, ()> {
        todo!()
    }

    fn parse_expr_something(&mut self) -> Result<P<Something>, ()> {
        todo!()
    }
}

impl TokenKind {
    fn as_unary_op(&self) -> Option<UnaryOp> {
        match self {
            TokenKind::Minus => Some(UnaryOp::Minus),
            TokenKind::BitNot => Some(UnaryOp::BitNot),
            TokenKind::LogicNot => Some(UnaryOp::LogicNot),
            TokenKind::Times => Some(UnaryOp::AddressOf),
            TokenKind::Shl => Some(UnaryOp::Dereference),
            _ => None,
        }
    }

    fn as_binary_op(&self) -> Option<BinaryOp> {
        match self {
            TokenKind::LogicAnd => Some(BinaryOp::LogicAnd),
            TokenKind::LogicOr => Some(BinaryOp::LogicOr),
            TokenKind::Less => Some(BinaryOp::Less),
            TokenKind::Greater => Some(BinaryOp::Greater),
            TokenKind::LessEq => Some(BinaryOp::LessEq),
            TokenKind::GreaterEq => Some(BinaryOp::GreaterEq),
            TokenKind::IsEq => Some(BinaryOp::IsEq),
            TokenKind::NotEq => Some(BinaryOp::NotEq),
            TokenKind::Plus => Some(BinaryOp::Plus),
            TokenKind::Minus => Some(BinaryOp::Minus),
            TokenKind::Times => Some(BinaryOp::Times),
            TokenKind::Div => Some(BinaryOp::Div),
            TokenKind::Mod => Some(BinaryOp::Mod),
            TokenKind::BitAnd => Some(BinaryOp::BitAnd),
            TokenKind::BitOr => Some(BinaryOp::BitOr),
            TokenKind::BitXor => Some(BinaryOp::BitXor),
            TokenKind::Shl => Some(BinaryOp::Shl),
            TokenKind::Shr => Some(BinaryOp::Shr),
            _ => None,
        }
    }

    fn as_assign_op(&self) -> Option<AssignOp> {
        match self {
            TokenKind::Assign => Some(AssignOp::Assign),
            TokenKind::PlusEq => Some(AssignOp::Plus),
            TokenKind::MinusEq => Some(AssignOp::Minus),
            TokenKind::TimesEq => Some(AssignOp::Times),
            TokenKind::DivEq => Some(AssignOp::Div),
            TokenKind::ModEq => Some(AssignOp::Mod),
            TokenKind::BitAndEq => Some(AssignOp::BitAnd),
            TokenKind::BitOrEq => Some(AssignOp::BitOr),
            TokenKind::BitXorEq => Some(AssignOp::BitXor),
            TokenKind::ShlEq => Some(AssignOp::Shl),
            TokenKind::ShrEq => Some(AssignOp::Shr),
            _ => None,
        }
    }

    fn as_basic_type(&self) -> Option<BasicType> {
        match self {
            TokenKind::KwS8 => Some(BasicType::S8),
            TokenKind::KwS16 => Some(BasicType::S16),
            TokenKind::KwS32 => Some(BasicType::S32),
            TokenKind::KwS64 => Some(BasicType::S64),
            TokenKind::KwU8 => Some(BasicType::U8),
            TokenKind::KwU16 => Some(BasicType::U16),
            TokenKind::KwU32 => Some(BasicType::U32),
            TokenKind::KwU64 => Some(BasicType::U64),
            TokenKind::KwF32 => Some(BasicType::F32),
            TokenKind::KwF64 => Some(BasicType::F64),
            TokenKind::KwBool => Some(BasicType::Bool),
            TokenKind::KwString => Some(BasicType::String),
            _ => None,
        }
    }
}
