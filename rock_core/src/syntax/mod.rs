pub mod ast_build;
pub mod ast_layer;
mod grammar;
mod parser;
mod syntax_kind;
pub mod syntax_tree;
mod token_set;

use crate::error::ErrorComp;
use crate::lexer;
use crate::session::FileID;
use parser::Parser;
use syntax_tree::SyntaxTree;

pub fn parse(source: &str, file_id: FileID) -> (SyntaxTree, Vec<ErrorComp>) {
    let (tokens, lex_errors) = lexer::lex(source, file_id, false);
    let mut parser = Parser::new(tokens, file_id);
    grammar::source_file(&mut parser);
    syntax_tree::build(parser.finish(), lex_errors)
}
