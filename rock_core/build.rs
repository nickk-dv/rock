fn main() {
    #[cfg(feature = "codegen_llvm")]
    link_llvm_dylib();
}

fn link_llvm_dylib() {
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-search=bin/windows");
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-search=bin/linux");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=bin/macos");
    } else {
        panic!("Rock-cli build is only supported for windows, linux, macos.");
    }
    if cfg!(not(target_pointer_width = "64")) {
        panic!("LLVM-C linking is only supported on 64-bit architectures.");
    }
    println!("cargo:rustc-link-lib=dylib=LLVM-C");
}
