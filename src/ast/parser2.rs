use super::*;
use crate::mem::*;

pub struct Parser2 {
    arena: Arena,
    peek_index: usize,
    tokens: Vec<TokenSpan>,
    sources: Vec<String>,
}

impl Parser2 {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(1024 * 1024 * 4),
            peek_index: 0,
            tokens: Vec::new(),
            sources: Vec::new(),
        }
    }

    pub fn parse_package(&mut self) -> Result<P<Package>, ()> {
        todo!()
    }

    fn parse_module(&mut self) -> Result<P<Module>, ()> {
        let mut module = self.arena.alloc::<Module>();
        while self.peek() != Token::Eof {
            let decl = self.parse_decl()?;
            module.decls.add(&mut self.arena, decl);
        }
        Ok(module)
    }

    fn parse_ident(&mut self) -> Result<Ident, ()> {
        if self.peek() == Token::Ident {
            let span = self.peek_span();
            self.consume();
            return Ok(Ident { span });
        }
        Err(())
    }

    fn parse_module_access(&mut self) -> Option<ModuleAccess> {
        if self.peek() != Token::Ident && self.peek_next(1) != Token::ColonColon {
            return None;
        }
        let mut module_access = ModuleAccess { names: List::new() };
        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = unsafe { self.parse_ident().unwrap_unchecked() };
            self.consume();
            module_access.names.add(&mut self.arena, name);
        }
        Some(module_access)
    }

    fn parse_generic_declaration(&mut self) -> Result<Option<GenericDeclaration>, ()> {
        let mut generic_decl = GenericDeclaration { names: List::new() };
        self.expect_token(Token::Less)?;
        loop {
            let ident = self.parse_ident()?;
            generic_decl.names.add(&mut self.arena, ident);
            if !self.try_consume(Token::Comma) {
                break;
            }
        }
        self.expect_token(Token::Greater)?;
        Ok(Some(generic_decl))
    }

    fn parse_generic_specialization(&mut self) -> Result<Option<GenericSpecialization>, ()> {
        let mut generic_spec = GenericSpecialization { types: List::new() };
        self.expect_token(Token::Less)?;
        loop {
            let tt = self.parse_type()?;
            generic_spec.types.add(&mut self.arena, tt);
            if !self.try_consume(Token::Comma) {
                break;
            }
        }
        if self.peek() == Token::Shr {
            let token = self.tokens.last_mut().unwrap();
            token.span.start += 1;
            token.token = Token::Greater;
        } else {
            self.expect_token(Token::Greater)?;
        }
        Ok(Some(generic_spec))
    }

    fn parse_type(&mut self) -> Result<Type, ()> {
        let mut tt = Type {
            pointer_level: 0,
            kind: TypeKind::Basic(BasicType::Void),
        };
        while self.try_consume(Token::Times) {
            tt.pointer_level += 1;
        }
        todo!();
        Ok(tt)
    }

    fn parse_custom_type(&mut self) -> Result<P<CustomType>, ()> {
        let mut custom_type = self.arena.alloc::<CustomType>();
        custom_type.module_access = self.parse_module_access();
        custom_type.name = self.parse_ident()?;
        custom_type.generic = self.parse_generic_specialization()?;
        Ok(custom_type)
    }

    fn parse_array_slice_type(&mut self) -> Result<P<ArraySliceType>, ()> {
        let mut array_slice_type = self.arena.alloc::<ArraySliceType>();
        self.expect_token(Token::OpenBracket)?;
        self.expect_token(Token::CloseBracket)?;
        array_slice_type.element = self.parse_type()?;
        Ok(array_slice_type)
    }

    fn parse_array_static_type(&mut self) -> Result<P<ArrayStaticType>, ()> {
        let mut array_static_type = self.arena.alloc::<ArrayStaticType>();
        self.expect_token(Token::OpenBracket)?;
        array_static_type.size = self.parse_expr()?;
        self.expect_token(Token::CloseBracket)?;
        array_static_type.element = self.parse_type()?;
        Ok(array_static_type)
    }

    fn parse_visibility(&mut self) -> Visibility {
        match self.peek() {
            Token::KwPub => Visibility::Public,
            _ => Visibility::Private,
        }
    }

    fn parse_mutability(&mut self) -> Mutability {
        match self.peek() {
            Token::KwMut => Mutability::Mutable,
            _ => Mutability::Immutable,
        }
    }

    fn parse_decl(&mut self) -> Result<Decl, ()> {
        todo!()
    }

    fn parse_mod_decl(&mut self) -> Result<P<ModDecl>, ()> {
        todo!()
    }

    fn parse_proc_decl(&mut self) -> Result<P<ProcDecl>, ()> {
        todo!()
    }

    fn parse_proc_param(&mut self) -> Result<ProcParam, ()> {
        todo!()
    }

    fn parse_impl_decl(&mut self) -> Result<P<ImplDecl>, ()> {
        todo!()
    }

    fn parse_enum_decl(&mut self) -> Result<P<EnumDecl>, ()> {
        todo!()
    }

    fn parse_enum_variant(&mut self) -> Result<EnumVariant, ()> {
        todo!()
    }

    fn parse_struct_decl(&mut self) -> Result<P<StructDecl>, ()> {
        todo!()
    }

    fn parse_struct_field(&mut self) -> Result<StructField, ()> {
        todo!()
    }

    fn parse_global_decl(&mut self) -> Result<P<GlobalDecl>, ()> {
        todo!()
    }

    fn parse_import_decl(&mut self) -> Result<P<ImportDecl>, ()> {
        todo!()
    }

    fn parse_import_target(&mut self) -> Result<P<ImportTarget>, ()> {
        todo!()
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ()> {
        todo!()
    }

    fn parse_for(&mut self) -> Result<P<For>, ()> {
        todo!()
    }

    fn parse_return(&mut self) -> Result<P<Return>, ()> {
        todo!()
    }

    fn parse_var_decl(&mut self) -> Result<P<VarDecl>, ()> {
        todo!()
    }

    //@var assign missing (harder to parse)

    fn parse_expr(&mut self) -> Result<Expr, ()> {
        todo!()
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, ()> {
        todo!()
    }

    fn parse_if(&mut self) -> Result<P<If>, ()> {
        todo!()
    }

    fn parse_else(&mut self) -> Result<Option<Else>, ()> {
        todo!()
    }

    fn parse_enum(&mut self) -> Result<P<Enum>, ()> {
        todo!()
    }

    //@can appear both in match and in regular assignment of sum type, unclear parse rules / data
    fn parse_enum_destructure(&mut self) -> Result<P<EnumDestructure>, ()> {
        todo!()
    }

    fn parse_cast(&mut self) -> Result<P<Cast>, ()> {
        todo!()
    }

    fn parse_block(&mut self) -> Result<P<Block>, ()> {
        todo!()
    }

    fn parse_match(&mut self) -> Result<P<Match>, ()> {
        todo!()
    }

    fn parse_match_arm(&mut self) -> Result<MatchArm, ()> {
        todo!()
    }

    fn parse_sizeof(&mut self) -> Result<P<Sizeof>, ()> {
        todo!()
    }

    fn parse_literal(&mut self) -> Result<P<Literal>, ()> {
        todo!()
    }

    fn parse_array_init(&mut self) -> Result<P<ArrayInit>, ()> {
        todo!()
    }

    fn parse_struct_init(&mut self) -> Result<P<StructInit>, ()> {
        todo!()
    }

    fn parse_access_chain(&mut self) -> Result<P<AccessChain>, ()> {
        todo!()
    }

    fn parse_primary_access(&mut self) -> Result<P<Access>, ()> {
        todo!()
    }

    fn parse_access(&mut self) -> Result<P<Access>, ()> {
        todo!()
    }

    fn peek(&self) -> Token {
        unsafe { self.tokens.get_unchecked(self.peek_index).token }
    }

    fn peek_next(&self, offset: usize) -> Token {
        unsafe { self.tokens.get_unchecked(self.peek_index + offset).token }
    }

    fn peek_span(&self) -> Span {
        unsafe { self.tokens.get_unchecked(self.peek_index).span }
    }

    fn consume(&mut self) {
        self.peek_index += 1;
    }

    fn try_consume(&mut self, token: Token) -> bool {
        if token == self.peek() {
            self.consume();
            return true;
        }
        false
    }

    fn expect_token(&mut self, token: Token) -> Result<(), ()> {
        if token == self.peek() {
            self.consume();
            return Ok(());
        }
        Err(())
    }
}
