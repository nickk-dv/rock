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

//@general de-couple errors and dont pass them in syntax_tree build
// they are not part of syntax_tree building process

pub fn parse(source: &str, module_id: ModuleID, with_trivia: bool) -> (SyntaxTree, Vec<ErrorComp>) {
    let (tokens, lex_errors) = lexer::lex(source, module_id, with_trivia);
    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);

    let (tree, mut parse_errors) = syntax_tree::build(parser.finish());
    parse_errors.extend(lex_errors);
    (tree, parse_errors)
}

pub fn parse_complete(
    source: &str,
    module_id: ModuleID,
    with_trivia: bool,
) -> Result<SyntaxTree, Vec<ErrorComp>> {
    let (tokens, mut lex_errors) = lexer::lex(source, module_id, with_trivia);
    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);
    let parse_result = parser.finish();

    if !lex_errors.is_empty() || parse_result.2.first().is_some() {
        if let Some(error) = parse_result.2.into_iter().next() {
            lex_errors.push(error);
        }
        Err(lex_errors)
    } else {
        let (tree, _) = syntax_tree::build(parse_result);
        Ok(tree)
    }
}
