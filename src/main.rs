mod ast;
mod err;
mod hir;
mod llvm;
mod mem;
mod testing;
mod tools;

fn main() {
    testing::test_threads();
    let _ = tools::cmd::cmd_parse();
}
