pub struct PackageData {
    name: String,
    bin_name: Option<String>,
    kind: PackageKind,
    version: Semver,
    dependencies: Vec<PackageDependency>,
}

pub struct PackageDependency {
    name: String,
    version: Semver,
}

#[derive(Copy, Clone)]
pub enum PackageKind {
    Bin,
    Lib,
}

/// https://semver.org/
#[derive(Copy, Clone)]
pub struct Semver {
    major: u32,
    minor: u32,
    patch: u32,
}

impl PackageData {
    pub fn new(
        name: String,
        bin_name: Option<String>,
        kind: PackageKind,
        version: Semver,
        dependencies: Vec<PackageDependency>,
    ) -> PackageData {
        PackageData {
            name,
            bin_name,
            kind,
            version,
            dependencies,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn bin_name(&self) -> &str {
        match self.bin_name.as_ref() {
            Some(name) => name,
            None => self.name(),
        }
    }
    pub fn kind(&self) -> PackageKind {
        self.kind
    }
    pub fn version(&self) -> Semver {
        self.version
    }
    pub fn dependencies(&self) -> &[PackageDependency] {
        &self.dependencies
    }
}

impl PackageDependency {
    pub fn new(name: String, version: Semver) -> PackageDependency {
        PackageDependency { name, version }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn version(&self) -> Semver {
        self.version
    }
}

impl PackageKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            PackageKind::Bin => "bin",
            PackageKind::Lib => "lib",
        }
    }
    pub const fn as_str_full(self) -> &'static str {
        match self {
            PackageKind::Bin => "executable",
            PackageKind::Lib => "library",
        }
    }
}

impl Semver {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Semver {
        Semver {
            major,
            minor,
            patch,
        }
    }
}

impl std::fmt::Display for Semver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
