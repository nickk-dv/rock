pub mod ast_build;
pub mod ast_layer;
mod grammar;
mod parser;
pub mod syntax_kind;
pub mod syntax_tree;

use crate::error::{ErrorBuffer, ErrorSink};
use crate::intern::{InternLit, InternPool};
use crate::lexer;
use crate::session::ModuleID;
use parser::Parser;
use syntax_tree::SyntaxTree;

pub fn parse_tree<'syn>(
    source: &str,
    intern_lit: &mut InternPool<'_, InternLit>,
    module_id: ModuleID,
    with_trivia: bool,
) -> (SyntaxTree<'syn>, ErrorBuffer) {
    let (tokens, lex_errors) = lexer::lex(source, intern_lit, module_id, with_trivia);

    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);
    let (tokens, events, mut parse_errors) = parser.finish();

    let complete = (lex_errors.error_count() + parse_errors.error_count()) == 0;
    let tree = syntax_tree::build(tokens, events, source, complete);

    parse_errors.join_e(lex_errors);
    (tree, parse_errors)
}

pub fn parse_tree_complete<'syn>(
    source: &str,
    intern_lit: &mut InternPool<'_, InternLit>,
    module_id: ModuleID,
    with_trivia: bool,
) -> Result<SyntaxTree<'syn>, ErrorBuffer> {
    let (tokens, mut lex_errors) = lexer::lex(source, intern_lit, module_id, with_trivia);

    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);
    let (tokens, events, parse_errors) = parser.finish();

    let parse_errors = parse_errors.collect();
    if let Some(first) = parse_errors.into_iter().next() {
        lex_errors.error(first);
    }

    if lex_errors.error_count() == 0 {
        let _ = lex_errors.collect();
        let tree = syntax_tree::build(tokens, events, source, true);
        Ok(tree)
    } else {
        Err(lex_errors)
    }
}
