fn main() {
    link_llvmc();
}

fn link_llvmc() {
    if cfg!(not(target_pointer_width = "64")) {
        panic!("rock-llvm build is only supported on 64-bit architectures");
    }
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-search=bin/windows");
        println!("cargo:rustc-link-lib=dylib=LLVM-C");
    } else if cfg!(target_os = "linux") {
        //@breaks rust analyzer when triggered, use #[cfg] instead?
        //panic!("LLVM-C link for linux is not yet supported");
    } else if cfg!(target_os = "macos") {
        //@breaks rust analyzer when triggered, use #[cfg] instead?
        //panic!("LLVM-C link for macos is not yet supported");
    } else {
        panic!("rock-llvm build is only supported on windows, linux and macos");
    }
}
