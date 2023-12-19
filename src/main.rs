mod ast;
mod llvm;
mod mem;

use llvm::*;
use std::time::Instant;

const fn cstr(s: &'static str) -> *const i8 {
    match s.as_bytes().last() {
        Some(c) => {
            if *c != b'\0' {
                panic!("Missing null terminator in cstr at comptime");
            }
        }
        None => {
            panic!("Empty cstr at comptime");
        }
    }
    s.as_ptr() as *const i8
}

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

    unsafe {
        let module = core::LLVMModuleCreateWithName(cstr("main_module\0"));
        let fn_type = core::LLVMFunctionType(core::LLVMInt32Type(), std::ptr::null_mut(), 0, 0);
        let fn_value = core::LLVMAddFunction(module, cstr("main\0"), fn_type);
        let builder = core::LLVMCreateBuilder();
        let block = core::LLVMAppendBasicBlock(fn_value, cstr("b\0"));
        core::LLVMPositionBuilderAtEnd(builder, block);
        core::LLVMBuildRet(builder, core::LLVMConstInt(core::LLVMInt32Type(), 32, 1));
        core::LLVMDumpModule(module);
        analysis::LLVMVerifyModule(
            module,
            analysis::LLVMVerifierFailureAction::LLVMPrintMessageAction,
            std::ptr::null_mut(),
        );
    }
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
