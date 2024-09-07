pub mod ast_build;
pub mod ast_layer;
mod grammar;
mod parser;
mod syntax_kind;
pub mod syntax_tree;

use crate::error::ErrorComp;
use crate::lexer;
use crate::session::ModuleID;
use parser::Parser;
use syntax_tree::SyntaxTree;

pub fn parse_tree(
    source: &str,
    module_id: ModuleID,
    with_trivia: bool,
) -> (SyntaxTree, Vec<ErrorComp>) {
    let (tokens, lex_errors) = lexer::lex(source, module_id, with_trivia);

    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);
    let (tokens, events, mut parse_errors) = parser.finish();

    let tree = syntax_tree::build(tokens, events);

    parse_errors.extend(lex_errors);
    (tree, parse_errors)
}

pub fn parse_tree_complete(
    source: &str,
    module_id: ModuleID,
    with_trivia: bool,
) -> Result<SyntaxTree, Vec<ErrorComp>> {
    let (tokens, mut lex_errors) = lexer::lex(source, module_id, with_trivia);

    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);
    let (tokens, events, parse_errors) = parser.finish();

    if lex_errors.is_empty() && parse_errors.is_empty() {
        let tree = syntax_tree::build(tokens, events);
        Ok(tree)
    } else {
        if let Some(parse_error) = parse_errors.into_iter().next() {
            lex_errors.push(parse_error);
        }
        Err(lex_errors)
    }
}
