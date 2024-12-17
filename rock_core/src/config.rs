use crate::support::AsStr;

#[derive(Copy, Clone)]
pub struct Config {
    pub target: TargetTriple,
    pub target_os: TargetOS,
    pub target_arch: TargetArch,
    pub target_ptr_width: TargetPtrWidth,
    pub build_kind: BuildKind,
}

crate::enum_as_str! {
    #[allow(non_camel_case_types)]
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetTriple {
        x86_64_pc_windows_msvc "x86_64-pc-windows-msvc",
        x86_64_unknown_linux_gnu "x86_64-unknown-linux-gnu",
        Arm_64_pc_windows_msvc "aarch64-pc-windows-msvc",
        Arm_64_unknown_linux_gnu "aarch64-unknown-linux-gnu",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetOS {
        Windows "windows",
        Linux "linux",
    }
}

crate::enum_as_str! {
    #[allow(non_camel_case_types)]
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetArch {
        x86_64 "x86_64",
        Arm_64 "aarch64",
    }
}

crate::enum_as_str! {
    #[allow(non_camel_case_types)]
    #[derive(Copy, Clone, PartialEq)]
    pub enum TargetPtrWidth {
        Bit_32 "32",
        Bit_64 "64",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum BuildKind {
        Debug "debug",
        Release "release",
    }
}

impl Config {
    pub fn new(target: TargetTriple, build_kind: BuildKind) -> Config {
        Config {
            target,
            target_os: target.os(),
            target_arch: target.arch(),
            target_ptr_width: target.arch().ptr_width(),
            build_kind,
        }
    }
}

impl TargetTriple {
    pub fn os(self) -> TargetOS {
        match self {
            TargetTriple::x86_64_pc_windows_msvc => TargetOS::Windows,
            TargetTriple::x86_64_unknown_linux_gnu => TargetOS::Linux,
            TargetTriple::Arm_64_pc_windows_msvc => TargetOS::Windows,
            TargetTriple::Arm_64_unknown_linux_gnu => TargetOS::Linux,
        }
    }

    pub fn arch(self) -> TargetArch {
        match self {
            TargetTriple::x86_64_pc_windows_msvc | TargetTriple::x86_64_unknown_linux_gnu => {
                TargetArch::x86_64
            }
            TargetTriple::Arm_64_pc_windows_msvc | TargetTriple::Arm_64_unknown_linux_gnu => {
                TargetArch::Arm_64
            }
        }
    }

    pub fn host() -> TargetTriple {
        #[cfg(target_arch = "x86_64")]
        {
            #[cfg(all(target_vendor = "pc", target_os = "windows", target_env = "msvc"))]
            return TargetTriple::x86_64_pc_windows_msvc;
            #[cfg(all(target_vendor = "unknown", target_os = "linux", target_env = "gnu"))]
            return TargetTriple::x86_64_unknown_linux_gnu;
        }
        #[cfg(target_arch = "aarch64")]
        {
            #[cfg(all(target_vendor = "pc", target_os = "windows", target_env = "msvc"))]
            return TargetTriple::Arm_64_pc_windows_msvc;
            #[cfg(all(target_vendor = "unknown", target_os = "linux", target_env = "gnu"))]
            return TargetTriple::Arm_64_unknown_linux_gnu;
        }
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
