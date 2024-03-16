use std::{cell::Cell, path::PathBuf};

use super::{
    event::Event,
    parser_api,
    syntax_tree::{SyntaxNode, SyntaxNodeKind, SyntaxTree, SyntaxTreeBuilder},
};
use crate::{
    ast::{lexer, token::Token, token_list::TokenList, CompCtx},
    error::{ErrorComp, SourceRange},
    text::TextRange,
    vfs,
};

#[test]
fn parse_api_test() {
    let mut ctx = CompCtx::new();
    let file_id = ctx.vfs.register_file(PathBuf::from("test/core.lang"));

    // not parsing whitespace, in real case would need to split whitespace and non whitespace tokens
    let lex = lexer::Lexer::new(&ctx.vfs.file(file_id).source, false);
    let tokens = lex.lex();

    let mut p = parser_api::Parser::new(tokens.tokens());
    super::grammar::source_file(&mut p);
    let events = p.finish();
    pretty_print_events(&events);
}

fn pretty_print_events(events: &[Event]) {
    use crate::error::ansi;

    println!("EVENTS:");
    let mut depth = 0;
    fn print_depth(depth: i32) {
        for _ in 0..depth {
            print!("  ");
        }
    }
    for e in events {
        match e {
            Event::EndNode => {}
            _ => print_depth(depth),
        }
        match e {
            Event::StartNode { kind } => {
                println!("{}{:?}{}", ansi::YELLOW_BOLD, kind, ansi::CLEAR);
                depth += 1;
            }
            Event::EndNode => {
                depth -= 1;
            }
            Event::Token { token } => println!("{}{:?}{}", ansi::GREEN, token, ansi::CLEAR),
            Event::Error { message } => {
                println!("{}ERROR:{} {:?}", ansi::MAGENTA, message, ansi::CLEAR)
            }
        }
    }
}

#[test]
fn parse_syntax_tree() {
    return;

    let mut ctx = CompCtx::new();
    let file_id = ctx.vfs.register_file(PathBuf::from("test/core.lang"));

    let lex = lexer::Lexer::new(&ctx.vfs.file(file_id).source, true);
    let tokens = lex.lex();

    let builder = SyntaxTreeBuilder::new();
    let mut parser = Parser {
        cursor: 0,
        tokens,
        builder,
        steps: Cell::new(0),
        ctx: &ctx,
        errors: Vec::new(),
    };

    parser.parse_source();
    let (root, errors) = parser.finish();
    let root = SyntaxTree {
        source: ctx.vfs.file(file_id).source.clone(),
        root,
    };
    crate::error::format::print_errors(&ctx.vfs, &errors);

    std::fs::File::create("test/syntax_tree.rast").expect("failed to create file");
    std::fs::write("test/syntax_tree.rast", root.format_to_string(false))
        .expect("failed to write to file");

    //println!("{}", root.format_to_string(true));
    // verify that tree is lossless
    assert_eq!(root.to_string(), root.source);
}

struct Parser<'a> {
    cursor: usize,
    tokens: TokenList,
    builder: SyntaxTreeBuilder,
    steps: Cell<u32>,
    ctx: &'a CompCtx,
    errors: Vec<ErrorComp>,
}

//@whitespace needs to be enclosed in correct node (to reflect correct structure of syntax)
// current approach eats it upfront, and assumes only a single whitespace token
// same applies for peek next
// and error nodes should not include the whitespace
// not sure how to properly delegate the whitespace handling and at which stage

impl<'a> Parser<'a> {
    fn at(&self, token: Token) -> bool {
        self.step_inc();
        token == self.tokens.get_token(self.cursor)
    }

    fn peek(&self) -> Token {
        self.step_inc();
        self.tokens.get_token(self.cursor)
    }

    fn expect(&mut self, token: Token) -> bool {
        if self.at(token) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn peek_next(&self) -> Token {
        self.step_inc();
        let mut offset = 1;
        if self.tokens.get_token(self.cursor + 1) == Token::Whitespace {
            offset = 2;
        }
        self.tokens.get_token(self.cursor + offset)
    }

    fn bump(&mut self) {
        let t = (
            self.tokens.get_token(self.cursor),
            self.tokens.get_range(self.cursor),
        );
        self.builder.token(t);
        self.cursor += 1;
        if self.at(Token::Whitespace) {
            let t = (
                self.tokens.get_token(self.cursor),
                self.tokens.get_range(self.cursor),
            );
            self.builder.token(t);
            self.cursor += 1;
        }
    }

    fn expect_after_or_end_node(&mut self, token: Token, end_node: bool) -> Option<()> {
        if self.at(token) {
            self.bump();
            Some(())
        } else {
            // look back considering whitespace
            // @hacky solution to whitescape
            let range = if self.tokens.get_token(self.cursor - 1) == Token::Whitespace {
                self.tokens.get_range(self.cursor - 2)
            } else {
                self.tokens.get_range(self.cursor - 1)
            };
            self.errors.push(
                ErrorComp::error(format!("add `{}` after this token", token.as_str()))
                    .context(SourceRange::new(range, vfs::FileID(0))),
            );
            if end_node {
                self.builder.end_node();
            }
            None
        }
    }

    fn expect_or_end_node(&mut self, token: Token) -> Option<()> {
        if self.at(token) {
            self.bump();
            Some(())
        } else {
            let message = format!("expected `{}`", token.as_str());
            self.error_eat(&message);
            self.builder.end_node();
            None
        }
    }

    fn error_eat(&mut self, message: &str) {
        let range = self.tokens.get_range(self.cursor);
        self.errors.push(
            ErrorComp::error(message.to_string()).context(SourceRange::new(range, vfs::FileID(0))), //@temp file 0
        );

        self.builder.start_node(SyntaxNodeKind::ERROR);
        self.bump();
        self.builder.end_node();
    }

    fn step_inc(&self) {
        self.steps.set(self.steps.get() + 1);
        if self.steps.get() >= 10_000_000 {
            //@add extra information about location
            panic!("parser appears to be stuck...");
        }
    }

    fn finish(self) -> (SyntaxNode, Vec<ErrorComp>) {
        (self.builder.finish(), self.errors)
    }

    fn parse_source(&mut self) {
        self.builder.start_node(SyntaxNodeKind::SOURCE_FILE);

        //@hack for 1st possible whitespace
        if self.at(Token::Whitespace) {
            let t = (
                self.tokens.get_token(self.cursor),
                self.tokens.get_range(self.cursor),
            );
            self.builder.token(t);
            self.cursor += 1;
        }

        while !self.at(Token::Eof) {
            self.decl();
        }
        self.builder.end_node();
    }

    fn decl(&mut self) {
        // vis is ignored
        // it should expect all top level keywords exept `use`
        match self.peek() {
            Token::KwUse => {
                self.use_decl();
            }
            Token::KwMod => {
                self.mod_decl();
            }
            Token::KwProc => self.proc_decl(),
            Token::KwEnum => self.enum_decl(),
            Token::KwUnion => self.union_decl(),
            Token::KwStruct => self.struct_decl(),
            Token::KwConst => self.const_decl(),
            Token::KwGlobal => self.global_decl(),
            _ => {
                self.error_eat("expected declaration");
                self.sync_to_decl();
            }
        }
    }

    fn at_decl_pattern(&self) -> bool {
        matches!(
            self.peek(),
            Token::KwUse
                | Token::KwMod
                | Token::KwProc
                | Token::KwEnum
                | Token::KwUnion
                | Token::KwStruct
                | Token::KwConst
                | Token::KwGlobal
        )
    }

    fn sync_to_decl(&mut self) {
        if self.at(Token::Eof) || self.at_decl_pattern() {
            return;
        }
        self.builder.start_node(SyntaxNodeKind::TOMBSTONE);
        while !self.at(Token::Eof) {
            if self.at_decl_pattern() {
                self.builder.end_node();
                return;
            }
            self.bump()
        }
        self.builder.end_node();
    }

    fn use_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::USE_DECL);
        self.bump(); // 'use'
        if self.path().is_none() {
            self.builder.end_node();
            return;
        }
        if self.use_symbol_list().is_none() {
            self.builder.end_node();
            return;
        }
        self.builder.end_node();
    }

    #[must_use]
    fn use_symbol_list(&mut self) -> Option<()> {
        self.builder.start_node(SyntaxNodeKind::USE_SYMBOL_LIST);
        self.expect_after_or_end_node(Token::Dot, true)?;
        self.expect_after_or_end_node(Token::BlockOpen, true)?;

        while !self.at(Token::Eof) && !self.at(Token::BlockClose) {
            if self.expect(Token::Comma) {
                self.error_eat("expected use symbol");
                break;
            }
            self.use_symbol();
            if !self.at(Token::BlockClose) {
                self.expect(Token::Comma);
            }
        }
        self.expect(Token::BlockClose);
        self.builder.end_node();
        Some(())
    }

    fn use_symbol(&mut self) {
        self.builder.start_node(SyntaxNodeKind::USE_SYMBOL);
        self.name_ref();
        self.use_rename();
        self.builder.end_node();
    }

    fn use_rename(&mut self) {
        if self.at(Token::KwAs) {
            self.builder.start_node(SyntaxNodeKind::USE_RENAME);
            self.bump();
            self.name();
            self.builder.end_node();
        }
    }

    // mod name;
    fn mod_decl(&mut self) -> Option<()> {
        self.builder.start_node(SyntaxNodeKind::MOD_DECL);
        self.bump(); // 'mod'
        if self.name_after().is_none() {
            self.builder.end_node();
            return None;
        }
        self.expect_after_or_end_node(Token::Semicolon, true)?;
        self.builder.end_node();
        Some(())
    }

    fn proc_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::PROC_DECL);
        self.bump(); // 'proc'
        self.name();
        // @...
        self.builder.end_node();
    }

    fn enum_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::ENUM_DECL);
        self.bump(); // 'enum'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn union_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::UNION_DECL);
        self.bump(); // 'union'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn struct_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::STRUCT_DECL);
        self.bump(); // 'struct'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn const_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::CONST_DECL);
        self.bump(); // 'const'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn global_decl(&mut self) {
        self.builder.start_node(SyntaxNodeKind::GLOBAL_DECL);
        self.bump(); // 'global'
        self.name();
        // @ ;
        self.builder.end_node();
    }

    fn name_after(&mut self) -> Option<()> {
        if self.at(Token::Ident) {
            self.builder.start_node(SyntaxNodeKind::NAME);
            self.bump();
            self.builder.end_node();
            Some(())
        } else {
            self.expect_after_or_end_node(Token::Ident, false)
        }
    }

    fn name(&mut self) {
        if self.at(Token::Ident) {
            self.builder.start_node(SyntaxNodeKind::NAME);
            self.bump();
            self.builder.end_node();
        } else {
            self.error_eat("expected `identifier`");
        }
    }

    fn name_ref(&mut self) {
        if self.at(Token::Ident) {
            self.builder.start_node(SyntaxNodeKind::NAME_REF);
            self.bump();
            self.builder.end_node();
        } else {
            self.error_eat("expected `identifier`");
        }
    }

    fn path(&mut self) -> Option<()> {
        self.builder.start_node(SyntaxNodeKind::PATH);
        match self.peek() {
            Token::KwSuper => self.bump(),
            Token::KwPackage => self.bump(),
            Token::Ident => self.name_ref(),
            _ => {
                self.error_eat("expected `identifier` `super` `package` in path");
                self.builder.end_node();
                return None;
            }
        }
        self.builder.end_node();
        Some(())
    }

    fn ty(&mut self) {
        //
    }
}
