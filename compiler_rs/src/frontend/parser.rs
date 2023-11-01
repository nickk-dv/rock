use super::arena::*;
use super::ast::*;
use super::lexer::*;
use super::token::*;

pub struct Parser<'a> {
    arena: Arena<'a>,
    lexer: Lexer<'a>,
    peek_index: u32,
    prev_last: Token,
    tokens: [Token; TOKEN_BUFFER_SIZE],
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
        return None;
    }

    fn parse_array_type(&mut self) -> Option<&'a mut AstArrayType<'a>> {
        return None;
    }

    fn parse_custom_type(&mut self) -> Option<&'a mut AstCustomType<'a>> {
        return None;
    }

    fn parse_import_decl(&mut self) -> Option<&'a mut AstImportDecl<'a>> {
        return None;
    }

    fn parse_use_decl(&mut self) -> Option<&'a mut AstUseDecl<'a>> {
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

    fn peek(&mut self) -> Token {
        return self.peek_next(0);
    }

    fn peek_next(&mut self, offset: u32) -> Token {
        return *self
            .tokens
            .get((self.peek_index + offset) as usize)
            .unwrap();
    }

    fn consume(&mut self) {
        self.peek_index += 1;
    }

    fn consume_get(&mut self) -> Token {
        let token = self.peek();
        self.consume();
        return token;
    }

    fn try_consume(&mut self, token_type: TokenType) -> Option<Token> {
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
