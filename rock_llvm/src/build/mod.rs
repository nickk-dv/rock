mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;

use crate::llvm::{TargetArch, TargetOS};
use rock_core::error::ErrorComp;
use rock_core::fs_env;
use rock_core::hir;
use rock_core::session::Session;
use std::path::PathBuf;

#[derive(Copy, Clone)]
pub enum BuildKind {
    Debug,
    Release,
}

impl BuildKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildKind::Debug => "debug",
            BuildKind::Release => "release",
        }
    }
}

#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum TargetTriple {
    x86_64_pc_windows_msvc,
    x86_64_unknown_linux_gnu,
    x86_64_apple_darwin,
    Arm_64_pc_windows_msvc,
    Arm_64_unknown_linux_gnu,
    Arm_64_apple_darwin,
}

impl TargetTriple {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetTriple::x86_64_pc_windows_msvc => "x86_64-pc-windows-msvc",
            TargetTriple::x86_64_unknown_linux_gnu => "x86_64-unknown-linux-gnu",
            TargetTriple::x86_64_apple_darwin => "x86_64-apple-darwin",
            TargetTriple::Arm_64_pc_windows_msvc => "aarch64-pc-windows-msvc",
            TargetTriple::Arm_64_unknown_linux_gnu => "aarch64-unknown-linux-gnu",
            TargetTriple::Arm_64_apple_darwin => "aarch64-apple-darwin",
        }
    }

    pub fn arch(self) -> TargetArch {
        match self {
            TargetTriple::x86_64_pc_windows_msvc
            | TargetTriple::x86_64_unknown_linux_gnu
            | TargetTriple::x86_64_apple_darwin => TargetArch::x86_64,
            TargetTriple::Arm_64_pc_windows_msvc
            | TargetTriple::Arm_64_unknown_linux_gnu
            | TargetTriple::Arm_64_apple_darwin => TargetArch::Arm_64,
        }
    }

    pub fn os(self) -> TargetOS {
        match self {
            TargetTriple::x86_64_pc_windows_msvc => TargetOS::Windows,
            TargetTriple::x86_64_unknown_linux_gnu => TargetOS::Linux,
            TargetTriple::x86_64_apple_darwin => TargetOS::Macos,
            TargetTriple::Arm_64_pc_windows_msvc => TargetOS::Windows,
            TargetTriple::Arm_64_unknown_linux_gnu => TargetOS::Linux,
            TargetTriple::Arm_64_apple_darwin => TargetOS::Macos,
        }
    }

    pub fn host() -> TargetTriple {
        #[rustfmt::skip]
        #[cfg(all(target_arch = "x86_64", target_vendor = "pc", target_os = "windows", target_env = "msvc"))]
        return TargetTriple::x86_64_pc_windows_msvc;
        #[rustfmt::skip]
        #[cfg(all(target_arch = "aarch64", target_vendor = "pc", target_os = "windows", target_env = "msvc"))]
        return TargetTriple::Arm_64_pc_windows_msvc;
        #[rustfmt::skip]
        #[cfg(all(target_arch = "x86_64", target_vendor = "unknown", target_os = "linux", target_env = "gnu"))]
        return TargetTriple::x86_64_unknown_linux_gnu;
        #[rustfmt::skip]
        #[cfg(all(target_arch = "aarch64", target_vendor = "unknown", target_os = "linux", target_env = "gnu"))]
        return TargetTriple::Arm_64_unknown_linux_gnu;
        #[cfg(all(target_arch = "x86_64", target_vendor = "apple", target_os = "macos",))]
        return TargetTriple::x86_64_apple_darwin;
        #[cfg(all(target_arch = "aarch64", target_vendor = "apple", target_os = "macos"))]
        return TargetTriple::Arm_64_apple_darwin;
    }
}

pub struct BuildOptions {
    pub kind: BuildKind,
    pub emit_llvm: bool,
}

pub fn build(
    hir: hir::Hir,
    session: &Session,
    options: BuildOptions,
) -> Result<PathBuf, ErrorComp> {
    let triple = TargetTriple::host();
    let (target, module) = emit_mod::codegen_module(hir, triple);

    let cwd = fs_env::dir_get_current_working()?;
    let mut build_path = cwd.join("build");
    fs_env::dir_create(&build_path, false)?;
    build_path.push(options.kind.as_str());
    fs_env::dir_create(&build_path, false)?;

    let manifest = session.package(Session::ROOT_ID).manifest();
    let bin_name = match &manifest.build.bin_name {
        Some(name) => name.clone(),
        None => manifest.package.name.clone(),
    };
    let binary_path = match triple.os() {
        TargetOS::Windows => build_path.join(format!("{bin_name}.exe")),
        TargetOS::Linux => build_path.join(&bin_name),
        TargetOS::Macos => build_path.join(&bin_name),
    };

    if options.emit_llvm {
        fs_env::file_create_or_rewrite(
            &build_path.join(format!("{bin_name}.ll")),
            &module.to_string(),
        )?;
    }

    //@fix module verify errors & return ErrorComp
    let verify_result = module.verify();
    let _ = verify_result.map_err(|e| eprintln!("llvm verify module failed:\n{}", e));

    let object_path = build_path.join("module.o");
    let object_result = module.emit_to_file(&target, object_path.to_str().unwrap());
    object_result.map_err(|e| ErrorComp::message(format!("llvm emit object failed:\n{}", e)))?;

    let arg_obj = object_path.to_string_lossy().to_string();
    let arg_out = format!("/out:{}", binary_path.to_string_lossy()); //@windows only format
    let mut args: Vec<String> = vec![arg_obj, arg_out];
    args.push("/defaultlib:libcmt.lib".into()); //@windows only

    //@assuming windows target + lld-link being in PATH
    // instead use bundled executable from the compiler toolchain
    //@use different command api, capture outputs + error
    let status = std::process::Command::new("lld-link")
        .args(args)
        .status()
        .map_err(|io_error| ErrorComp::message(format!("failed to link program:\n{}", io_error)))?;
    Ok(binary_path)
}

pub fn run(binary_path: PathBuf, args: Vec<String>) -> Result<(), ErrorComp> {
    let _ = std::process::Command::new(&binary_path)
        .args(args)
        .status()
        .map_err(|io_error| {
            ErrorComp::message(format!(
                "failed to run executable `{}`\nreason: {}",
                binary_path.to_string_lossy(),
                io_error
            ))
        })?;
    Ok(())
}
