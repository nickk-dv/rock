pub mod ast;
pub mod intern;
pub mod lexer;
pub mod parse_error;
mod parser;
pub mod token;
pub mod token_list;

use crate::error::ErrorComp;
use crate::vfs;
use ast::*;
use intern::InternPool;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

//@empty error tokens produce invalid range diagnostic
// need to handle 'missing' and `unexpected token` errors to be differently

/// Persistant data across a compilation
//@move to separate module / folder
// make iteration a private function
// if its still needed by ls server
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

struct Timer {
    start_time: Instant,
}

impl Timer {
    fn new() -> Self {
        Timer {
            start_time: Instant::now(),
        }
    }

    fn elapsed_ms(self, message: &'static str) {
        let elapsed = self.start_time.elapsed();
        let sec_ms = elapsed.as_secs_f64() * 1000.0;
        let ms = sec_ms + f64::from(elapsed.subsec_nanos()) / 1_000_000.0;
        eprintln!("{}: {:.3} ms", message, ms);
    }
}

pub fn parse<'a, 'ast>(mut ctx: &'a mut CompCtx, ast: &'a mut Ast<'ast>) -> Vec<ErrorComp> {
    let mut errors = Vec::<ErrorComp>::new();

    let timer = Timer::new();
    let files = collect_files(&mut ctx);
    timer.elapsed_ms("collect files");

    let timer = Timer::new();
    let handle = &mut std::io::BufWriter::new(std::io::stderr());

    for file_id in files {
        let lexer = lexer::Lexer::new(ctx.vfs.file(file_id).source.as_str(), false);
        let tokens = lexer.lex();
        let source_copy = ctx.vfs.file(file_id).source.clone();
        let mut parser =
            parser::Parser::new(tokens, &mut ast.arena, ctx.intern_mut(), &source_copy);
        let parse_res = parser.module(file_id);

        match parse_res {
            Ok(module) => {
                ast.modules.push(module);
            }
            Err(error) => {
                errors.push(error);
            }
        }
    }
    timer.elapsed_ms("parsed all files");
    let _ = handle.flush();
    errors
}

#[must_use]
fn collect_files(ctx: &mut CompCtx) -> Vec<vfs::FileID> {
    //relative 'root' path of the project being compiled
    //@hardcoded to 'test' for faster testing
    let mut dir_paths = Vec::new();
    let src_dir = std::env::current_dir().unwrap().join("src");
    eprintln!("collecting files from: {:?}", src_dir);
    dir_paths.push(src_dir);

    let handle = &mut std::io::BufWriter::new(std::io::stderr());
    let mut filepaths = Vec::new();

    while let Some(dir_path) = dir_paths.pop() {
        match std::fs::read_dir(&dir_path) {
            Ok(dir) => {
                for entry in dir.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(extension) = path.extension() {
                            if extension == "lang" {
                                filepaths.push(path);
                            }
                        }
                    } else if path.is_dir() {
                        dir_paths.push(path);
                    }
                }
            }
            Err(err) => {
                //@deal with the error of reading a directory
                //report::report(
                //    handle,
                //    &Error::file_io(FileIOError::DirRead)
                //        .info(err.to_string())
                //        .info(format!("path: {:?}", dir_path))
                //        .into(),
                //    &ctx,
                //);
            }
        }
    }

    let mut files = Vec::new();

    for path in filepaths {
        let id = ctx.vfs.register_file(path);
        files.push(id);
    }

    files
}
