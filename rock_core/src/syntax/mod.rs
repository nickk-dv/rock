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
use ast_build::AstBuild;
use parser::Parser;
use syntax_tree::SyntaxTree;

pub fn parse_all(session: &mut Session, with_trivia: bool) {
    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());

        let tree_result =
            parse_tree_complete(&file.source, module_id, with_trivia, &mut session.intern_lit);
        let module = session.module.get_mut(module_id);

        match tree_result {
            Ok(tree) => {
                session.stats.line_count += file.line_ranges.len() as u32;
                session.stats.token_count += tree.tokens().token_count() as u32;
                module.set_tree(tree);
            }
            Err(errors) => module.errors.replace_e(errors),
        }
    }

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        if module.errors.did_error(0) {
            return;
        }
    }

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());

        let tree = module.tree_expect();
        let mut ctx =
            AstBuild::new(tree, &file.source, &mut session.intern_name, &mut session.ast_state);
        let items = ast_build::source_file(&mut ctx, tree.source_file());
        let ast = ctx.finish(items);

        let module = session.module.get_mut(module_id);
        module.set_ast(ast);
    }
}

pub fn parse_all_lsp(session: &mut Session, with_trivia: bool) {
    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());
        if module.tree_version == file.version {
            continue;
        }

        let (tree, errors) =
            parse_tree(&file.source, module_id, with_trivia, &mut session.intern_lit);

        let module = session.module.get_mut(module_id);
        module.set_tree(tree);
        module.tree_version = file.version;
        module.errors.replace_e(errors);
    }

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        if module.errors.did_error(0) {
            return;
        }
    }

    for module_id in session.module.ids() {
        let module = session.module.get(module_id);
        let file = session.vfs.file(module.file_id());
        if module.ast_version == file.version {
            continue;
        }

        let tree = module.tree_expect();
        let mut ctx =
            AstBuild::new(tree, &file.source, &mut session.intern_name, &mut session.ast_state);
        let items = ast_build::source_file(&mut ctx, tree.source_file());
        let ast = ctx.finish(items);

        let module = session.module.get_mut(module_id);
        module.set_ast(ast);
        module.ast_version = file.version;
    }
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
