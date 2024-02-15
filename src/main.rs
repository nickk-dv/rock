mod ast;
mod err;
mod hir;
mod llvm;
mod mem;
mod tools;
mod check;

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let _ = tools::cmd::cmd_parse();
}
