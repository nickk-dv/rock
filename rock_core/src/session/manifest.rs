use crate::error::Error;
use crate::support::AsStr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub package: PackageManifest,               // table key [package]
    pub build: BuildManifest,                   // table key [build]
    pub dependencies: BTreeMap<String, Semver>, // table key [dependencies]
}

#[derive(Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,                 // package name
    pub kind: PackageKind,            // package kind
    pub version: Semver,              // semver version
    pub owner: Option<String>,        // repository owner (github.com username)
    pub authors: Option<Vec<String>>, // list of authors
    pub description: Option<String>,  // short package description
}

#[derive(Serialize, Deserialize)]
pub struct BuildManifest {
    pub bin_name: Option<String>,        // executable name
    pub nodefaultlib: Option<bool>,      // dont link against default lib
    pub lib_paths: Option<Vec<PathBuf>>, // library search paths
    pub links: Option<Vec<String>>,      // library names or paths to link against
}

#[derive(Serialize, Deserialize)]
pub struct IndexManifest {
    #[serde(rename = "v")]
    pub version: Semver,
    pub owner: String,
    #[serde(rename = "deps")]
    pub dependencies: Vec<IndexDependency>,
}

#[derive(Serialize, Deserialize)]
pub struct IndexDependency {
    pub name: String,
    #[serde(rename = "req")]
    pub version_req: Semver,
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
    pub enum PackageKind {
        #[serde(rename = "bin")]
        Bin "bin",
        #[serde(rename = "lib")]
        Lib "lib",
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct Semver {
    major: u32,
    minor: u32,
    patch: u32,
}

pub fn serialize(manifest: &Manifest) -> Result<String, Error> {
    basic_toml::to_string(manifest).map_err(|error| {
        Error::message(format!("failed to serialize manifest file\nreason: {}", error))
    })
}

pub fn deserialize(manifest: &str, manifest_path: &PathBuf) -> Result<Manifest, Error> {
    basic_toml::from_str(manifest).map_err(|error| {
        Error::message(format!(
            "failed to parse manifest file: `{}`\nreason: {}",
            manifest_path.to_string_lossy(),
            error
        ))
    })
}

pub fn index_serialize(manifest: &IndexManifest) -> Result<String, Error> {
    serde_json::to_string(manifest).map_err(|error| {
        Error::message(format!("failed to serialize index manifest file\nreason: {}", error))
    })
}

pub fn index_deserialize(
    manifest: String,
    manifest_path: &PathBuf,
) -> Result<Vec<IndexManifest>, Error> {
    let mut manifests = Vec::with_capacity(manifest.lines().count());
    for line in manifest.lines() {
        let index_manifest = serde_json::from_str(line).map_err(|error| {
            Error::message(format!(
                "failed to parse index manifest file: `{}`\nreason: {}",
                manifest_path.to_string_lossy(),
                error
            ))
        })?;
        manifests.push(index_manifest);
    }
    Ok(manifests)
}

pub fn package_verify_name(name: &str) -> Result<&str, Error> {
    const MAX_NAME_LEN: usize = 32;

    if name.len() > MAX_NAME_LEN {
        return Err(Error::message(format!(
            "package name cannot exceed {MAX_NAME_LEN} characters",
        )));
    }
    let Some(fc) = name.chars().next() else {
        return Err(Error::message("package name cannot be empty"));
    };
    for c in name.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(
                Error::message(format!("package name contains invalid character `{c}`\nallowed characters: lowercase `a-z`, digits `0-9` and `_`")),
            );
        }
    }
    if !fc.is_ascii_lowercase() {
        return Err(Error::message(format!("package name cannot start with `{fc}`")));
    }
    Ok(name)
}

impl PackageKind {
    pub fn full_name(self) -> &'static str {
        match self {
            PackageKind::Bin => "executable",
            PackageKind::Lib => "library",
        }
    }
}

impl Semver {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Semver {
        Semver { major, minor, patch }
    }
}
impl std::fmt::Display for Semver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
impl Serialize for Semver {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'a> Deserialize<'a> for Semver {
    fn deserialize<D: serde::Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse::<Semver>().map_err(serde::de::Error::custom)
    }
}
impl std::str::FromStr for Semver {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err("invalid semver format, expected 3 numbers separated by `.`".into());
        }
        let major = parts[0]
            .parse::<u32>()
            .map_err(|error| format!("failed to parse major version\nreason: {error}"))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|error| format!("failed to parse minor version\nreason: {error}"))?;
        let patch = parts[2]
            .parse::<u32>()
            .map_err(|error| format!("failed to parse patch version\nreason: {error}"))?;
        Ok(Semver { major, minor, patch })
    }
}
