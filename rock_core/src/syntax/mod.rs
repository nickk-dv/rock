pub mod ast_build;
pub mod ast_layer;
mod grammar;
mod parser;
mod syntax_kind;
pub mod syntax_tree;
mod token_set;

use crate::error::ErrorComp;
use crate::lexer;
use crate::session::ModuleID;
use parser::Parser;
use syntax_tree::SyntaxTree;

pub fn parse(source: &str, module_id: ModuleID) -> (SyntaxTree, Vec<ErrorComp>) {
    let (tokens, lex_errors) = lexer::lex(source, module_id, false);
    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);
    syntax_tree::build(parser.finish(), lex_errors)
}
