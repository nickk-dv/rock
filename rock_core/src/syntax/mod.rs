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

pub fn parse_tree<'src, 'syn>(
    source: &'src str,
    intern_lit: &'src mut InternPool<'_, InternLit>,
    module_id: ModuleID,
    with_trivia: bool,
) -> (SyntaxTree<'syn>, ErrorBuffer) {
    let (tokens, mut lex_errors) = lexer::lex(source, intern_lit, module_id, with_trivia);

    let mut parser = Parser::new(tokens, module_id);
    grammar::source_file(&mut parser);
    let (tokens, events, parse_errors) = parser.finish();

    let tree = syntax_tree::build(tokens, events, source);

    lex_errors.join_e(parse_errors);
    (tree, lex_errors)
}

pub fn parse_tree_complete<'src, 'syn>(
    source: &'src str,
    intern_lit: &'src mut InternPool<'_, InternLit>,
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
        let tree = syntax_tree::build(tokens, events, source);
        Ok(tree)
    } else {
        Err(lex_errors)
    }
}
