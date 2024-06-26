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

//@allow early returns for usage where broken tree is not needed (eg: format & ast_build)
// currently syntax_tree::build always runs producing syntax tree
pub fn parse(source: &str, module_id: ModuleID, with_trivia: bool) -> (SyntaxTree, Vec<ErrorComp>) {
    let (tokens, lex_errors) = lexer::lex(source, module_id, with_trivia);

    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);

    let (tree, mut parse_errors) = syntax_tree::build(parser.finish());
    syntax_tree::tree_print(&tree, source); //@debug only
    parse_errors.extend(lex_errors);
    (tree, parse_errors)
}
