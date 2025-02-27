pub mod ast_build;
pub mod ast_layer;
pub mod format;
mod grammar;
#[allow(unsafe_code)]
mod lexer;
mod parser;
pub mod syntax_kind;
pub mod syntax_tree;
#[allow(unsafe_code)]
pub mod token;

use crate::error::{ErrorBuffer, ErrorSink};
use crate::intern::{InternPool, LitID};
use crate::session::ModuleID;
use crate::session::Session;
use ast_build::{AstBuild, AstBuildState};
use parser::Parser;
use syntax_tree::SyntaxTree;

pub fn parse_all(session: &mut Session, with_trivia: bool) -> Result<(), ErrorBuffer> {
    let mut state = AstBuildState::new();

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());
        if module.tree_version == file.version {
            eprintln!("[info] tree up to date: `{}`", file.path.to_string_lossy());
            continue;
        }

        let tree_result =
            parse_tree_complete(&file.source, module_id, with_trivia, &mut session.intern_lit);
        match tree_result {
            Ok(tree) => {
                session.stats.line_count += file.line_ranges.len() as u32;
                session.stats.token_count += tree.tokens().token_count() as u32;
                session.module.get_mut(module_id).set_tree(tree);
            }
            Err(errors) => state.errors.join_e(errors),
        }
        session.module.get_mut(module_id).tree_version = file.version;
    }

    if state.errors.error_count() > 0 {
        return state.errors.result(());
    }

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());
        if module.ast_version == file.version {
            eprintln!("[info] ast up to date: `{}`", file.path.to_string_lossy());
            continue;
        }

        let tree = module.tree_expect();
        let mut ctx = AstBuild::new(tree, &file.source, &mut session.intern_name, &mut state);
        let items = ast_build::source_file(&mut ctx, tree.source_file());
        let ast = ctx.finish(items);
        session.module.get_mut(module_id).set_ast(ast);
        session.module.get_mut(module_id).ast_version = file.version;
    }

    state.errors.result(())
}

pub fn parse_tree<'syn>(
    source: &str,
    module_id: ModuleID,
    with_trivia: bool,
    intern_lit: &mut InternPool<'_, LitID>,
) -> (SyntaxTree<'syn>, ErrorBuffer) {
    let (tokens, lex_errors) = lexer::lex(source, module_id, with_trivia, intern_lit);

    let mut parser = Parser::new(tokens, module_id, source);
    grammar::source_file(&mut parser);
    let (tokens, events, mut parse_errors) = parser.finish();

    let complete = (lex_errors.error_count() + parse_errors.error_count()) == 0;
    let (tree, tree_errors) = syntax_tree::tree_build(source, tokens, events, module_id, complete);

    parse_errors.join_e(lex_errors);
    parse_errors.join_e(tree_errors);
    (tree, parse_errors)
}

pub fn parse_tree_complete<'syn>(
    source: &str,
    module_id: ModuleID,
    with_trivia: bool,
    intern_lit: &mut InternPool<'_, LitID>,
) -> Result<SyntaxTree<'syn>, ErrorBuffer> {
    let (tokens, mut lex_errors) = lexer::lex(source, module_id, with_trivia, intern_lit);

    let mut parser = Parser::new(tokens, module_id, source);
    grammar::source_file(&mut parser);
    let (tokens, events, parse_errors) = parser.finish();

    if let Some(first) = parse_errors.collect().into_iter().next() {
        lex_errors.error(first);
    }

    if lex_errors.error_count() == 0 {
        let _ = lex_errors.collect();
        let (tree, tree_errors) = syntax_tree::tree_build(source, tokens, events, module_id, true);

        if tree_errors.error_count() == 0 {
            let _ = tree_errors.collect();
            Ok(tree)
        } else {
            Err(tree_errors)
        }
    } else {
        Err(lex_errors)
    }
}
