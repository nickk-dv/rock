mod ast;
mod ast_parse;
mod error;
mod hir;
mod hir_lower;
mod llvm;
mod mem;
mod syntax;
mod text;
mod tools;
mod vfs;

fn main() {
    tools::cli();
}
