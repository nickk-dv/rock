use super::ast2::*;
use super::intern::INTERN_DUMMY_ID;
use super::parser::FileID;
use super::token2::*;
use crate::err::error::{ParseContext, ParseError};
use crate::mem::arena_id::*;
use crate::mem::list_id::*;

struct Parser<'ast> {
    cursor: usize,
    tokens: TokenList,
    arena: &'ast mut Arena,
    source: &'ast str,
}

impl<'ast> Parser<'ast> {
    fn new(tokens: TokenList, arena: &'ast mut Arena, source: &'ast str) -> Self {
        Self {
            cursor: 0,
            tokens,
            arena,
            source,
        }
    }

    fn peek(&self) -> Token {
        self.tokens.token(self.cursor)
    }

    fn peek_next(&self, offset: usize) -> Token {
        self.tokens.token(self.cursor + offset)
    }

    fn eat(&mut self) {
        self.cursor += 1;
    }

    fn try_eat(&mut self, token: Token) -> bool {
        if self.peek() == token {
            self.eat();
            return true;
        }
        return false;
    }

    fn expect(&mut self, token: Token, ctx: ParseContext) -> Result<(), ParseError> {
        if self.peek() == token {
            self.eat();
            Ok(())
        } else {
            Err(ParseError::ExpectToken2(ctx, token))
        }
    }

    fn module(&mut self) -> Module {
        //@templ file id
        let module = Module {
            file_id: FileID(0),
            decls: List::new(),
        };
        //@decls
        module
    }

    fn vis(&mut self) -> Vis {
        if self.try_eat(Token::KwPub) {
            Vis::Public
        } else {
            Vis::Private
        }
    }

    fn mutt(&mut self) -> Mut {
        if self.try_eat(Token::KwMut) {
            Mut::Mutable
        } else {
            Mut::Immutable
        }
    }

    fn ident(&mut self, ctx: ParseContext) -> Result<Ident, ParseError> {
        let index = TokenIndex::new(self.cursor);
        if self.try_eat(Token::Ident) {
            Ok(Ident {
                id: INTERN_DUMMY_ID,
                index,
            })
        } else {
            Err(ParseError::Ident(ctx))
        }
    }

    fn path(&mut self) -> Result<Option<Id<Path>>, ParseError> {
        let kind_index = TokenIndex::new(self.cursor);
        let kind = match self.peek() {
            Token::KwSuper => PathKind::Super,
            Token::KwPackage => PathKind::Package,
            _ => PathKind::None,
        };
        if kind != PathKind::None {
            self.eat();
            self.expect(Token::ColonColon, ParseContext::ModulePath)?;
        }
        let mut segments = List::new();
        while self.peek() == Token::Ident && self.peek_next(1) == Token::ColonColon {
            let name = self.ident(ParseContext::ModulePath)?;
            self.eat();
            segments.add(&mut self.arena, name);
        }
        if segments.is_empty() {
            Ok(None)
        } else {
            let path = self.arena.alloc(Path {
                kind,
                kind_index,
                segments,
            });
            Ok(Some(path))
        }
    }

    //@testing parsing of numbers
    pub fn number_parse_test(&mut self) -> Result<(), ()> {
        while self.peek() != Token::Eof {
            match self.peek() {
                Token::IntLit => {
                    if let Ok(value) = self.int() {
                        eprintln!("int:   {}", value);
                    }
                }
                Token::FloatLit => {
                    if let Ok(value) = self.float() {
                        eprintln!("float: {}", value);
                    }
                }
                _ => {
                    self.eat();
                }
            }
        }
        Ok(())
    }

    fn int(&mut self) -> Result<u64, ()> {
        let span = self.tokens.span(self.cursor);
        self.eat();
        let str = span.slice(self.source);
        match str.parse::<u64>() {
            Ok(value) => Ok(value),
            Err(error) => {
                eprint!("parse int error: {}", error.to_string());
                Err(())
            }
        }
    }

    fn float(&mut self) -> Result<f64, ()> {
        let span = self.tokens.span(self.cursor);
        self.eat();
        let str = span.slice(self.source);
        match str.parse::<f64>() {
            Ok(value) => Ok(value),
            Err(error) => {
                eprintln!("parse float error: {}", error.to_string());
                Err(())
            }
        }
    }
}

#[test]
fn test_number_parsing() {
    let source = " 1024   500  3.14  0.123  49494 002  0000.1  2.14596 2323.0";
    let lexer = super::lexer2::Lexer::new(&source);
    let tokens = lexer.lex();
    let mut arena = Arena::new(1024);
    let mut parser = Parser::new(tokens, &mut arena, source);
    match parser.number_parse_test() {
        Ok(_) => todo!(),
        Err(_) => todo!(),
    }
}
