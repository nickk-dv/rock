mod parser;

use crate::arena::Arena;
use crate::ast;
use crate::error::ErrorComp;
use crate::intern::InternPool;
use crate::lexer;
use crate::vfs;
use std::path::PathBuf;

///@persistant data across a compilation
// move to separate module / folder

pub struct CompCtx {
    pub vfs: vfs::Vfs,
    intern: InternPool,
    timings: Vec<std::time::Instant>,
    traces: Vec<String>,
}

impl CompCtx {
    pub fn new() -> CompCtx {
        CompCtx {
            vfs: vfs::Vfs::new(),
            intern: InternPool::new(),
            timings: Vec::new(),
            traces: Vec::new(),
        }
    }

    pub fn intern(&self) -> &InternPool {
        &self.intern
    }

    pub fn intern_mut(&mut self) -> &mut InternPool {
        &mut self.intern
    }

    pub fn timing_start(&mut self) {
        self.timings.push(std::time::Instant::now());
    }

    pub fn timing_end(&mut self, message: &'static str) {
        if let Some(start) = self.timings.pop() {
            let elapsed = start.elapsed();
            let sec_ms = elapsed.as_secs_f64() * 1000.0;
            let ms = sec_ms + f64::from(elapsed.subsec_nanos()) / 1_000_000.0;
            self.traces.push(format!("{}: {:.3} ms", message, ms));
        }
    }

    pub fn add_trace(&mut self, message: String) {
        self.traces.push(message);
    }

    pub fn display_traces(&self) {
        for trace in self.traces.iter() {
            eprintln!("{trace}");
        }
    }
}

impl Drop for CompCtx {
    fn drop(&mut self) {
        self.display_traces();
    }
}

pub fn parse<'ast>(ctx: &mut CompCtx) -> Result<ast::Ast<'ast>, Vec<ErrorComp>> {
    ctx.timing_start();
    let source_files = collect_cwd_source_files().map_err(|error| vec![error])?;
    ctx.timing_end("parse: collect file paths");

    ctx.timing_start();
    let file_ids: Vec<vfs::FileID> = source_files
        .into_iter()
        .map(|path| ctx.vfs.register_file(path))
        .collect();
    ctx.timing_end("parse: vfs read files");

    let mut errors = Vec::new();
    let mut ast = ast::Ast {
        arena: Arena::new(),
        modules: Vec::new(),
    };

    ctx.timing_start();
    for file_id in file_ids {
        //@fix source clone() later
        //@due to borrowing of intern_pool & source text at the same time
        let source = ctx.vfs.file(file_id).source.clone();
        let lexer = lexer::Lexer::new(&source, false);
        let tokens = lexer.lex();
        let mut parser = parser::Parser::new(tokens, &mut ast.arena, ctx.intern_mut(), &source);

        match parser::module(&mut parser, file_id) {
            Ok(module) => {
                ast.modules.push(module);
            }
            Err(error) => {
                errors.push(error);
            }
        }
    }
    ctx.timing_end("parse: ast");
    ctx.add_trace(format!(
        "arena ast: mem usage {} bytes",
        ast.arena.mem_usage()
    ));
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
