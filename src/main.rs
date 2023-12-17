mod ast;
mod llvm;
mod mem;

use std::time::Instant;

fn test_llvm_version() {
    let mut major: u32 = 0;
    let mut minor: u32 = 0;
    let mut patch: u32 = 0;
    unsafe {
        llvm::core::LLVMGetVersion(
            &mut major as *mut u32,
            &mut minor as *mut u32,
            &mut patch as *mut u32,
        );
    }
    println!("LLVM Is threaded {}", unsafe {
        llvm::core::LLVMIsMultithreaded()
    });
    println!("LLVM Version {}.{}.{}", major, minor, patch);
}

fn main() {
    test_llvm_version();

    let now = Instant::now();
    let mut parser = ast::Parser::new();
    let result = parser.parse_package();
    let duration_ms = now.elapsed().as_secs_f64() * 1000.0;
    println!("parse time: ms {}", duration_ms);

    match result {
        Ok(_) => println!("Parse success"),
        Err(()) => print!("Parse failed"),
    }
}
