mod ast;
mod err;
mod hir;
mod llvm;
mod mem;
mod tools;

fn main() {
    let _ = tools::cmd::cmd_parse();
}
