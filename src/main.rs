mod ast;
mod check;
mod err;
mod llvm;
mod mem;
mod tools;

fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    let _ = tools::cmd::cmd_parse();
}
