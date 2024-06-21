mod ast_layer;
mod grammar;
mod parser;
mod syntax_kind;
mod syntax_tree;
mod token_set;

use crate::error::ErrorComp;
use crate::lexer;
use crate::session::FileID;
use parser::Parser;
use syntax_tree::SyntaxTree;

pub fn parse(source: &str, file_id: FileID) -> (SyntaxTree, Vec<ErrorComp>) {
    let tokens = if let Ok(tokens) = lexer::lex(source, file_id, false) {
        tokens
    } else {
        //@temp work-around
        panic!("lexer failed");
    };
    let mut parser = Parser::new(tokens);
    grammar::source_file(&mut parser);
    syntax_tree::tree_build(parser.finish(), file_id)
}
