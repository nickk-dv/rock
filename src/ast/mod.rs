pub mod ast;
pub mod intern;
pub mod lexer;
pub mod parse_error;
mod parser;
pub mod token;
mod token_gen;
pub mod token_list;

use crate::error::ErrorComp;
use crate::mem::Arena;
use crate::vfs;
use ast::*;
use intern::InternPool;
use std::path::PathBuf;

///@persistant data across a compilation
// move to separate module / folder

pub struct CompCtx {
    pub vfs: vfs::Vfs,
    intern: InternPool,
}

impl CompCtx {
    pub fn new() -> CompCtx {
        CompCtx {
            vfs: vfs::Vfs::new(),
            intern: InternPool::new(),
        }
    }

    pub fn intern(&self) -> &InternPool {
        &self.intern
    }

    pub fn intern_mut(&mut self) -> &mut InternPool {
        &mut self.intern
    }
}

pub fn parse<'ast>(ctx: &mut CompCtx) -> Result<Ast<'ast>, Vec<ErrorComp>> {
    let source_files = collect_cwd_source_files().map_err(|error| vec![error])?;

    let mut errors = Vec::new();
    let mut ast = Ast {
        arena: Arena::new(),
        modules: Vec::new(),
    };

    for path in source_files {
        let file_id = ctx.vfs.register_file(path);
        let source = ctx.vfs.file(file_id).source.clone();

        let lexer = lexer::Lexer::new(&source, false);
        let tokens = lexer.lex();
        let mut parser = parser::Parser::new(tokens, &mut ast.arena, ctx.intern_mut(), &source);

        match parser.module(file_id) {
            Ok(module) => {
                ast.modules.push(module);
            }
            Err(error) => {
                errors.push(error);
            }
        }
    }

    if errors.is_empty() {
        Ok(ast)
    } else {
        Err(errors)
    }
}

fn collect_cwd_source_files() -> Result<Vec<PathBuf>, ErrorComp> {
    let cwd = std::env::current_dir().map_err(|io_error| {
        ErrorComp::error(format!(
            "failed to read current working directory. reason: {}",
            io_error
        ))
    })?;

    let src_dir = cwd.join("src");
    if !src_dir.exists() {
        return Err(ErrorComp::error(format!(
            "could not find src folder. source files must be located in `{}`",
            src_dir.to_string_lossy()
        )));
    }

    let mut dir_visits = vec![src_dir];
    let mut source_files = Vec::new();

    while let Some(dir) = dir_visits.pop() {
        let read_dir = std::fs::read_dir(&dir).map_err(|io_error| {
            ErrorComp::error(format!(
                "failed to read directory: `{}`. reason: {}",
                dir.to_string_lossy(),
                io_error
            ))
        })?;
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().unwrap_or_default() == "lang" {
                source_files.push(path);
            } else if path.is_dir() {
                dir_visits.push(path);
            }
        }
    }

    Ok(source_files)
}
