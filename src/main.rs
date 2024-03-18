#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod arena;
mod ast;
mod ast_parse;
mod error;
mod hir;
mod hir_lower;
mod llvm;
mod syntax;
mod text;
mod tools;
mod vfs;

fn main() {
    tools::cli();
}
