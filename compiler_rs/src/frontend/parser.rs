use super::arena::*;
use super::ast::*;
use super::lexer::*;
use super::token::*;

pub struct Parser<'a> {
    arena: Arena<'a>,
    lexer: Lexer<'a>,
    peek_index: u32,
    prev_last: Token<'a>,
    tokens: [Token<'a>; TOKEN_BUFFER_SIZE],
}

impl<'a> Parser<'a> {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(4 * 1024 * 1024),
            lexer: Lexer::default(),
            peek_index: 0,
            prev_last: Token::default(),
            tokens: [Token::default(); TOKEN_BUFFER_SIZE],
        }
    }

    pub fn parse_program(&mut self, path: &'a str) -> Option<&'a mut AstProgram<'a>> {
        return None;
    }

    fn parse_ast(&mut self, source: &'a [u8], filepath: String) -> Option<&'a mut Ast<'a>> {
        self.peek_index = 0;
        self.lexer = Lexer::new(source);
        self.lexer.tokenize(&mut self.tokens);

        let ast = self.arena.alloc::<Ast>();
        ast.source = source;
        ast.filepath = filepath;

        loop {
            match self.peek().token_type {
                TokenType::Ident => match self.peek_next(1).token_type {
                    TokenType::DotDot => {}
                    _ => {
                        println!("Expected :: in global declaration");
                        return None;
                    }
                },
                TokenType::InputEnd => {
                    return Some(ast);
                }
                _ => {
                    println!("Expected identifier in global declaration");
                    return None;
                }
            }

            match self.peek_next(2).token_type {
                TokenType::KeywordImport => {
                    self.parse_import_decl().and_then(|decl| {
                        ast.imports.push(decl);
                        Some(())
                    });
                }
                TokenType::KeywordUse => {
                    self.parse_use_decl().and_then(|decl| {
                        ast.uses.push(decl);
                        Some(())
                    });
                }
                TokenType::KeywordStruct => {
                    self.parse_struct_decl().and_then(|decl| {
                        ast.structs.push(decl);
                        Some(())
                    });
                }
                TokenType::KeywordEnum => {
                    self.parse_enum_decl().and_then(|decl| {
                        ast.enums.push(decl);
                        Some(())
                    });
                }
                TokenType::ParenStart => {
                    self.parse_proc_decl().and_then(|decl| {
                        ast.procs.push(decl);
                        Some(())
                    });
                }
                _ => {
                    self.parse_global_decl().and_then(|decl| {
                        ast.globals.push(decl);
                        Some(())
                    });
                }
            }
        }
    }

    fn parse_type(&mut self) -> Option<AstType<'a>> {
        let mut type_ = AstType::default();
        let mut token = self.peek();

        while token.token_type == TokenType::Times {
            self.consume();
            token = self.peek();
            type_.pointer_level += 1;
        }

        if let Some(basic_type) = token.token_type.to_basic_type() {
            self.consume();
            type_.union = AstTypeUnion::Basic(basic_type);
            return Some(type_);
        }

        match token.token_type {
            TokenType::Ident => {
                self.parse_custom_type().and_then(|custom| {
                    type_.union = AstTypeUnion::Custom(custom);
                    Some(())
                });
            }
            TokenType::BracketStart => {
                self.parse_array_type().and_then(|array| {
                    type_.union = AstTypeUnion::Array(array);
                    Some(())
                });
            }
            _ => {
                println!("Expected basic type, type identifier or array type");
                return None;
            }
        }

        return Some(type_);
    }

    fn parse_array_type(&mut self) -> Option<&'a mut AstArrayType<'a>> {
        let array_type = self.arena.alloc::<AstArrayType>();
        self.consume();

        self.parse_sub_expr(0).and_then(|expr| {
            array_type.const_expr.expr = expr;
            Some(())
        });

        if let None = self.try_consume(TokenType::BracketEnd) {
            println!("Expected ']' in array type");
            return None;
        }

        self.parse_type().and_then(|type_| {
            array_type.element_type = type_;
            Some(())
        });

        return Some(array_type);
    }

    fn parse_custom_type(&mut self) -> Option<&'a mut AstCustomType<'a>> {
        let custom = self.arena.alloc::<AstCustomType>();

        let import = AstIdent::from_token(self.consume_get());
        if let None = self.try_consume(TokenType::Dot) {
            custom.ident = import;
        } else {
            custom.import = Some(import);
            if let Some(ident) = self.try_consume(TokenType::Ident) {
                custom.ident = AstIdent::from_token(ident);
            } else {
                println!("Expected type identifier");
                return None;
            }
        }

        return Some(custom);
    }

    fn parse_import_decl(&mut self) -> Option<&'a mut AstImportDecl<'a>> {
        let decl = self.arena.alloc::<AstImportDecl>();
        decl.alias = AstIdent::from_token(self.consume_get());
        self.consume();
        self.consume();

        return match self.try_consume(TokenType::LiteralString) {
            Some(token) => {
                decl.filepath.token = token;
                Some(decl)
            }
            None => {
                println!("Expected string literal in 'import' declaration");
                None
            }
        };
    }

    fn parse_use_decl(&mut self) -> Option<&'a mut AstUseDecl<'a>> {
        let decl = self.arena.alloc::<AstUseDecl>();
        decl.alias = AstIdent::from_token(self.consume_get());
        self.consume();
        self.consume();

        match self.try_consume(TokenType::Ident) {
            Some(token) => {
                decl.import = AstIdent::from_token(token);
            }
            None => {
                println!("Expected imported module identifier");
                return None;
            }
        };

        if self.try_consume(TokenType::Dot).is_none() {
            println!("Expected '.' followed by symbol");
            return None;
        }

        match self.try_consume(TokenType::Ident) {
            Some(token) => {
                decl.symbol = AstIdent::from_token(token);
            }
            None => {
                println!("Expected symbol identifier");
                return None;
            }
        };

        return None;
    }

    fn parse_struct_decl(&mut self) -> Option<&'a mut AstStructDecl<'a>> {
        return None;
    }

    fn parse_enum_decl(&mut self) -> Option<&'a mut AstEnumDecl<'a>> {
        return None;
    }

    fn parse_proc_decl(&mut self) -> Option<&'a mut AstProcDecl<'a>> {
        return None;
    }

    fn parse_global_decl(&mut self) -> Option<&'a mut AstGlobalDecl<'a>> {
        return None;
    }

    fn parse_block(&mut self) -> Option<&'a mut AstBlock<'a>> {
        return None;
    }

    fn parse_short_block(&mut self) -> Option<&'a mut AstBlock<'a>> {
        return None;
    }

    fn parse_statement(&mut self) -> Option<AstStatement<'a>> {
        return None;
    }

    fn parse_if(&mut self) -> Option<&'a mut AstIf<'a>> {
        return None;
    }

    fn parse_else(&mut self) -> Option<&'a mut AstElse<'a>> {
        return None;
    }

    fn parse_for(&mut self) -> Option<&'a mut AstFor<'a>> {
        return None;
    }

    fn parse_defer(&mut self) -> Option<&'a mut AstDefer<'a>> {
        return None;
    }

    fn parse_break(&mut self) -> Option<&'a mut AstBreak> {
        return None;
    }

    fn parse_return(&mut self) -> Option<&'a mut AstReturn<'a>> {
        return None;
    }

    fn parse_switch(&mut self) -> Option<&'a mut AstSwitch<'a>> {
        return None;
    }

    fn parse_continue(&mut self) -> Option<&'a mut AstContinue> {
        return None;
    }

    fn parse_var_decl(&mut self) -> Option<&'a mut AstVarDecl<'a>> {
        return None;
    }

    fn parse_var_assign(&mut self) -> Option<&'a mut AstVarAssign<'a>> {
        return None;
    }

    fn parse_proc_call(&mut self, has_import: bool) -> Option<&'a mut AstProcCall<'a>> {
        return None;
    }

    fn parse_expr(&mut self) -> Option<&'a mut AstExpr<'a>> {
        return None;
    }

    fn parse_sub_expr(&mut self, min_precedence: u32) -> Option<&'a mut AstExpr<'a>> {
        return None;
    }

    fn parse_primary_expr(&mut self) -> Option<&'a mut AstExpr<'a>> {
        return None;
    }

    fn parse_term(&mut self) -> Option<&'a mut AstTerm<'a>> {
        return None;
    }

    fn parse_var(&mut self) -> Option<&'a mut AstVar<'a>> {
        return None;
    }

    fn parse_access(&mut self) -> Option<&'a mut AstAccess<'a>> {
        return None;
    }

    fn parse_var_access(&mut self) -> Option<&'a mut AstVarAccess<'a>> {
        return None;
    }

    fn parse_array_access(&mut self) -> Option<&'a mut AstArrayAccess<'a>> {
        return None;
    }

    fn parse_enum(&mut self, has_import: bool) -> Option<&'a mut AstEnum<'a>> {
        return None;
    }

    fn parse_sizeof(&mut self) -> Option<&'a mut AstSizeof<'a>> {
        return None;
    }

    fn parse_array_init(&mut self) -> Option<&'a mut AstArrayInit<'a>> {
        return None;
    }

    fn parse_struct_init(
        &mut self,
        has_import: bool,
        has_type: bool,
    ) -> Option<&'a mut AstStructInit<'a>> {
        return None;
    }

    fn peek(&mut self) -> Token<'a> {
        return self.peek_next(0);
    }

    fn peek_next(&mut self, offset: u32) -> Token<'a> {
        return *self
            .tokens
            .get((self.peek_index + offset) as usize)
            .unwrap();
    }

    fn consume(&mut self) {
        self.peek_index += 1;
    }

    fn consume_get(&mut self) -> Token<'a> {
        let token = self.peek();
        self.consume();
        return token;
    }

    fn try_consume(&mut self, token_type: TokenType) -> Option<Token<'a>> {
        let token = self.peek();
        if token_type == token.token_type {
            Some(token)
        } else {
            None
        }
    }

    fn span_start(&mut self) -> u32 {
        0
    }

    fn span_end(&mut self) -> u32 {
        0
    }
}
