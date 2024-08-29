use crate::enum_str_convert;

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetTriple {
        x86_64_pc_windows_msvc => "x86_64-pc-windows-msvc",
        x86_64_unknown_linux_gnu => "x86_64-unknown-linux-gnu",
        x86_64_apple_darwin => "x86_64-apple-darwin",
        Arm_64_pc_windows_msvc => "aarch64-pc-windows-msvc",
        Arm_64_unknown_linux_gnu => "aarch64-unknown-linux-gnu",
        Arm_64_apple_darwin => "aarch64-apple-darwin",
    }
);

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetArch {
        x86_64 => "x86_64",
        Arm_64 => "aarch64",
    }
);

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetOS {
        Windows => "windows",
        Linux => "linux",
        Macos => "macos",
    }
);

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetPtrWidth {
        Bit_32 => "32",
        Bit_64 => "64",
    }
);

enum_str_convert!(
    fn as_str, fn from_str,
    #[derive(Copy, Clone, PartialEq)]
    pub enum BuildKind {
        Debug => "debug",
        Release => "release",
    }
);

impl TargetTriple {
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
    pub fn ptr_width(self) -> TargetPtrWidth {
        match self {
            TargetArch::x86_64 => TargetPtrWidth::Bit_64,
            TargetArch::Arm_64 => TargetPtrWidth::Bit_64,
        }
    }
}

impl TargetPtrWidth {
    pub fn ptr_size(self) -> u64 {
        match self {
            TargetPtrWidth::Bit_32 => 4,
            TargetPtrWidth::Bit_64 => 8,
        }
    }
}
