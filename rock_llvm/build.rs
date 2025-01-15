fn main() {
    if cfg!(not(target_pointer_width = "64")) {
        panic!("rock-llvm build is only supported on 64-bit architectures");
    }
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-search=bin/windows");
    } else if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-search=bin/linux");
    } else if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-search=bin/macos");
    } else {
        panic!("rock-llvm build is only supported on windows, linux, macos");
    }
    println!("cargo:rustc-link-lib=dylib=LLVM-C");
}
