mod context;
mod emit_expr;
mod emit_mod;
mod emit_stmt;

use crate::llvm::TargetArch;
use rock_core::hir;

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

    pub fn host() -> TargetTriple {
        #[rustfmt::skip]
        #[cfg(all(target_arch = "x86_64", target_vendor = "pc", target_os = "windows", target_env = "msvc"))]
        const HOST_TRIPLE: TargetTriple = TargetTriple::x86_64_pc_windows_msvc;
        #[rustfmt::skip]
        #[cfg(all(target_arch = "aarch64", target_vendor = "pc", target_os = "windows", target_env = "msvc"))]
        const HOST_TRIPLE: TargetTriple = TargetTriple::Arm_64_pc_windows_msvc;
        #[rustfmt::skip]
        #[cfg(all(target_arch = "x86_64", target_vendor = "unknown", target_os = "linux", target_env = "gnu"))]
        const HOST_TRIPLE: TargetTriple = TargetTriple::x86_64_unknown_linux_gnu;
        #[rustfmt::skip]
        #[cfg(all(target_arch = "aarch64", target_vendor = "unknown", target_os = "linux", target_env = "gnu"))]
        const HOST_TRIPLE: TargetTriple = TargetTriple::Arm_64_unknown_linux_gnu;
        #[cfg(all(target_arch = "x86_64", target_vendor = "apple", target_os = "macos",))]
        const HOST_TRIPLE: TargetTriple = TargetTriple::x86_64_apple_darwin;
        #[cfg(all(target_arch = "aarch64", target_vendor = "apple", target_os = "macos"))]
        const HOST_TRIPLE: TargetTriple = TargetTriple::Arm_64_apple_darwin;
        HOST_TRIPLE
    }
}

pub fn build(hir: hir::Hir) {
    emit_mod::codegen_module(hir, TargetTriple::host());
}
