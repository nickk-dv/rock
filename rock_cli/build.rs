fn main() {
    if cfg!(not(target_pointer_width = "64")) {
        panic!("only 64-bit builds are supported");
    }
    if cfg!(target_os = "windows") {
        println!("cargo:rustc-link-search=bin/windows");
    } else {
        panic!("unsupported target_os");
    }
    println!("cargo:rustc-link-lib=dylib=LLVM-C");
}
