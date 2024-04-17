use crate::error::ErrorComp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub package: PackageManifest,
    pub build: Option<BuildManifest>,
    pub dependencies: BTreeMap<String, Semver>,
}

#[derive(Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub kind: PackageKind,
    pub version: Semver,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum PackageKind {
    #[serde(rename = "bin")]
    Bin,
    #[serde(rename = "lib")]
    Lib,
}

#[derive(Copy, Clone)]
pub struct Semver {
    major: u32,
    minor: u32,
    patch: u32,
}

#[derive(Serialize, Deserialize)]
pub struct BuildManifest {
    pub bin_name: String,
}

impl std::str::FromStr for Semver {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err("Invalid Semver format");
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| "Invalid major version")?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| "Invalid minor version")?;
        let patch = parts[2]
            .parse::<u32>()
            .map_err(|_| "Invalid patch version")?;

        Ok(Semver {
            major,
            minor,
            patch,
        })
    }
}

impl Serialize for Semver {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Semver {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<Semver>().map_err(serde::de::Error::custom)
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

impl Manifest {
    pub fn serialize(&self) -> Result<String, ErrorComp> {
        basic_toml::to_string(self).map_err(|error| {
            ErrorComp::message(format!(
                "failed to serialize manifest file, reason:\n{}",
                error
            ))
        })
    }
}
