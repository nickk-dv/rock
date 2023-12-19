use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let os = get_os();
    check_os_support(os);
    copy_llvm_dyn_lib();

    println!("cargo:rustc-link-lib=LLVM-C");
    println!("cargo:rustc-link-search=native=bin/windows");
}

enum OS {
    Windows,
    Linux,
    Macos,
}

fn get_os() -> OS {
    let target = env::var("TARGET").unwrap();
    if target.contains("windows") {
        OS::Windows
    } else if target.contains("linux") {
        OS::Linux
    } else if target.contains("macos") {
        OS::Macos
    } else {
        panic!("Unknown build target.");
    }
}

fn check_os_support(os: OS) {
    match os {
        OS::Windows => {}
        _ => panic!("Only windows builds are supported so far."),
    }
}

fn copy_llvm_dyn_lib() {
    let source_path = "bin/windows/LLVM-C.dll";
    let profile = std::env::var("PROFILE").unwrap();
    let copy_path = Path::new("target").join(profile).join("LLVM-C.dll");
    if !copy_path.exists() {
        if let Err(err) = fs::copy(&source_path, &copy_path) {
            panic!("Failed to copy LLVM-C.dll into target path: {}", err);
        }
    }
}
