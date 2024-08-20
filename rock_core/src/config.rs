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

#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum TargetArch {
    x86_64,
    Arm_64,
}

#[derive(Copy, Clone)]
pub enum TargetOS {
    Windows,
    Linux,
    Macos,
}

#[derive(Copy, Clone)]
pub enum BuildKind {
    Debug,
    Release,
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

impl TargetArch {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetArch::x86_64 => "x86_64",
            TargetArch::Arm_64 => "aarch64",
        }
    }
}

impl TargetOS {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetOS::Windows => "windows",
            TargetOS::Linux => "linux",
            TargetOS::Macos => "macos",
        }
    }
}

impl BuildKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildKind::Debug => "debug",
            BuildKind::Release => "release",
        }
    }
}
