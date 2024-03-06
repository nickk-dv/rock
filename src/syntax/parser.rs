use std::cell::Cell;

use super::syntax_tree::{SyntaxNode, SyntaxNodeKind, SyntaxTree, SyntaxTreeBuilder};
use crate::ast::{lexer, token::Token, token_list::TokenList};

#[test]
fn parse_syntax_tree() {
    let file = std::fs::read_to_string("test/core.lang").expect("failed to read main file source");
    let lex = lexer::Lexer::new(&file, true);
    let tokens = lex.lex();

    let builder = SyntaxTreeBuilder::new();
    let mut parser = Parser {
        cursor: 0,
        tokens,
        builder,
        steps: Cell::new(0),
    };

    parser.parse_source();
    let root = SyntaxTree {
        source: file,
        root: parser.finish(),
    };
    println!("{}", root.format_to_string(true));

    std::fs::File::create("test/syntax_tree.rast").expect("failed to create file");
    std::fs::write("test/syntax_tree.rast", root.format_to_string(false))
        .expect("failed to write to file");

    // verify that tree is lossless
    assert_eq!(root.to_string(), root.source);
}

struct Parser {
    cursor: usize,
    tokens: TokenList,
    builder: SyntaxTreeBuilder,
    steps: Cell<u32>,
}

//@whitespace needs to be enclosed in correct node (to reflect correct structure of syntax)
// current approach eats it upfront, and assumes only a single whitespace token
// same applies for peek next
// and error nodes should not include the whitespace
// not sure how to properly delegate the whitespace handling and at which stage

impl Parser {
    fn peek(&self) -> Token {
        self.step_inc();
        self.tokens.get_token(self.cursor)
    }

    fn peek_next(&self) -> Token {
        self.step_inc();
        let mut offset = 1;
        if self.tokens.get_token(self.cursor + 1) == Token::Whitespace {
            offset = 2;
        }
        self.tokens.get_token(self.cursor + offset)
    }

    fn eat(&mut self) {
        let t = (
            self.tokens.get_token(self.cursor),
            self.tokens.get_range(self.cursor),
        );
        self.builder.token(t);
        self.cursor += 1;
        if self.peek() == Token::Whitespace {
            let t = (
                self.tokens.get_token(self.cursor),
                self.tokens.get_range(self.cursor),
            );
            self.builder.token(t);
            self.cursor += 1;
        }
    }

    fn error_eat(&mut self) {
        self.builder.start_node(SyntaxNodeKind::ERROR);
        self.eat();
        self.builder.end_node();
    }

    fn step_inc(&self) {
        self.steps.set(self.steps.get() + 1);
        if self.steps.get() >= 10_000_000 {
            //@add extra information about location
            panic!("parser appears to be stuck...");
        }
    }

    fn finish(self) -> SyntaxNode {
        self.builder.finish()
    }

    fn parse_source(&mut self) {
        self.builder.start_node(SyntaxNodeKind::SOURCE_FILE);

        //@hack for 1st possible whitespace
        if self.peek() == Token::Whitespace {
            let t = (
                self.tokens.get_token(self.cursor),
                self.tokens.get_range(self.cursor),
            );
            self.builder.token(t);
            self.cursor += 1;
        }

        while self.peek() != Token::Eof {
            self.decl();
        }
        self.builder.end_node();
    }

    fn decl(&mut self) {
        // vis is ignored
        // it should expect all top level keywords exept `use`
        match self.peek() {
            Token::KwUse => self.use_decl(),
            Token::KwMod => self.mod_decl(),
            Token::KwProc => self.proc_decl(),
            Token::KwEnum => self.enum_decl(),
            Token::KwUnion => self.union_decl(),
            Token::KwStruct => self.struct_decl(),
            Token::KwConst => self.const_decl(),
            Token::KwGlobal => self.global_decl(),
            _ => self.error_eat(),
        }
    }

    fn use_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::USE_DECL);
        self.eat(); // 'use'
                    // @...
        self.builder.end_node();
    }

    fn mod_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::MOD_DECL);
        self.eat(); // 'mod'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn proc_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::MOD_DECL);
        self.eat(); // 'proc'
        self.name();
        // @...
        self.builder.end_node();
    }

    fn enum_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::ENUM_DECL);
        self.eat(); // 'mod'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn union_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::UNION_DECL);
        self.eat(); // 'mod'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn struct_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::STRUCT_DECL);
        self.eat(); // 'mod'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn const_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::CONST_DECL);
        self.eat(); // 'mod'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn global_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::GLOBAL_DECL);
        self.eat(); // 'mod'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn name(&mut self) {
        match self.peek() {
            Token::Ident => {
                self.builder.start_node(SyntaxNodeKind::NAME);
                self.eat();
                self.builder.end_node();
            }
            _ => self.error_eat(),
        }
    }

    fn name_ref(&mut self) {
        match self.peek() {
            Token::Ident => {
                self.builder.start_node(SyntaxNodeKind::NAME_REF);
                self.eat();
                self.builder.end_node();
            }
            _ => self.error_eat(),
        }
    }

    fn path(&mut self) {
        //
    }

    fn ty(&mut self) {
        //
    }
}
