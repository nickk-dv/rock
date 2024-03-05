mod ast;
mod err;
mod hir;
mod hir_lower;
mod llvm;
mod mem;
mod syntax;
mod tools;

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let _ = tools::cmd::cmd_parse();
}
