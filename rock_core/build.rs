//@rock-ls also attepmts to find symbols from llvm-sys when building
// so llvm is linked in both cases, address this if it poses an issue
// doesnt matter so far. only rock-cli actually uses any llvm touching codegen modules.
fn main() {
    println!("cargo:rustc-link-search=bin/windows");
    println!("cargo:rustc-link-lib=dylib=LLVM-C");
}
