mod ast;
mod err;
mod llvm;
mod mem;
mod tools;

fn main() {
    let _ = tools::cmd::cmd_parse();
}
