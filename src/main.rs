mod ast;
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
    std::env::set_var("RUST_BACKTRACE", "1");
    let _ = tools::cmd::cmd_parse();
}
