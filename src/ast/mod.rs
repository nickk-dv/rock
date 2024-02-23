pub mod ast;
pub mod intern;
pub mod lexer;
pub mod parse_error;
mod parser;
pub mod span;
pub mod token;
mod token_list;
mod visit;

use crate::err::error::*;
use crate::err::report;
use crate::mem::*;
use ast::*;
use intern::InternPool;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

//@empty error tokens produce invalid span diagnostic

/// Persistant data across a compilation
pub struct CompCtx {
    files: Vec<File>,
    intern: InternPool,
}

pub struct File {
    pub path: PathBuf,
    pub source: String,
}

#[derive(Clone, Copy)]
pub struct FileID(pub u32);

impl CompCtx {
    pub fn new() -> CompCtx {
        CompCtx {
            files: Vec::new(),
            intern: InternPool::new(),
        }
    }

    pub fn file(&self, file_id: FileID) -> &File {
        if let Some(file) = self.files.get(file_id.raw_index()) {
            return file;
        }
        //@internal error
        panic!("getting file using an invalid ID {}", file_id.raw())
    }

    pub fn intern(&self) -> &InternPool {
        &self.intern
    }

    pub fn intern_mut(&mut self) -> &mut InternPool {
        &mut self.intern
    }

    pub fn add_file(&mut self, path: PathBuf, source: String) -> FileID {
        let id = self.files.len();
        self.files.push(File { path, source });
        FileID(id as u32)
    }
}

impl FileID {
    fn raw(&self) -> u32 {
        self.0
    }

    fn raw_index(&self) -> usize {
        self.0 as usize
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

pub fn parse() -> Result<(CompCtx, Ast), ()> {
    let timer = Timer::new();
    let mut ctx = CompCtx::new();
    let mut ast = Ast {
        arena: Arena::new(),
        modules: Vec::new(),
    };
    timer.elapsed_ms("parse arena created");

    let timer = Timer::new();
    let files = collect_files(&mut ctx);
    timer.elapsed_ms("collect files");

    let timer = Timer::new();
    let handle = &mut std::io::BufWriter::new(std::io::stderr());
    let mut module_strings: Vec<Vec<String>> = Vec::new();

    for file_id in files {
        let source = ctx.file(file_id).source.as_str();
        let lexer = lexer::Lexer::new(source);
        let tokens = lexer.lex();
        let mut parser = parser::Parser::new(&tokens, &mut ast.arena, source);

        match parser.module(file_id) {
            Ok(module) => {
                ast.modules.push(module);
                module_strings.push(tokens.strings());
            }
            Err(err) => {
                report::report(handle, &err, &ctx);
            }
        }
    }
    timer.elapsed_ms("parsed all files");
    let _ = handle.flush();

    let timer = Timer::new();
    for (id, module) in ast.modules.iter().enumerate() {
        //@cloning entire source text, since &File + &mut InternPool
        // violates borrow checker try solving this later
        let mut interner = ModuleInterner {
            file: ctx.file(module.file_id).source.clone(),
            intern_pool: ctx.intern_mut(),
            strings: module_strings.get(id).unwrap(), //@unwrap
        };
        visit::visit_module_with(&mut interner, *module);
    }
    timer.elapsed_ms("interned all modules");

    report::err_status((ctx, ast))
}

#[must_use]
fn collect_files(ctx: &mut CompCtx) -> Vec<FileID> {
    //relative 'root' path of the project being compiled
    //@hardcoded to 'test' for faster testing
    let mut dir_paths = Vec::new();
    dir_paths.push(PathBuf::from("test"));

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
                report::report(
                    handle,
                    &Error::file_io(FileIOError::DirRead)
                        .info(err.to_string())
                        .info(format!("path: {:?}", dir_path))
                        .into(),
                    &ctx,
                );
            }
        }
    }

    let mut files = Vec::new();

    for path in filepaths {
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                let id = ctx.add_file(path, source);
                files.push(id);
            }
            Err(err) => {
                report::report(
                    handle,
                    &Error::file_io(FileIOError::FileRead)
                        .info(err.to_string())
                        .info(format!("path: {:?}", path))
                        .into(),
                    &ctx,
                );
            }
        };
    }

    files
}

struct ModuleInterner<'a> {
    file: String,
    intern_pool: &'a mut InternPool,
    strings: &'a [String],
}

impl<'a> visit::MutVisit for ModuleInterner<'a> {
    fn visit_ident(&mut self, ident: &mut Ident) {
        let string = ident.span.slice(&self.file);
        ident.id = self.intern_pool.intern(string);
    }

    fn visit_expr(&mut self, mut expr: P<Expr>) {
        if let ExprKind::LitString { ref mut id } = expr.kind {
            let string = unsafe { self.strings.get_unchecked(id.0 as usize).as_str() };
            *id = self.intern_pool.intern(string);
        }
    }
}
