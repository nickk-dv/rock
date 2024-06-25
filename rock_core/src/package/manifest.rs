use super::semver::Semver;
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
    pub authors: Option<Vec<String>>, // list of authors
    pub repository: Option<String>,   // repository link
    pub description: Option<String>,  // short package description
}

#[derive(Serialize, Deserialize)]
pub struct BuildManifest {
    pub bin_name: Option<String>,        // executable name
    pub nodefaultlib: Option<bool>,      // dont link against default lib
    pub lib_paths: Option<Vec<PathBuf>>, // library search paths
    pub links: Option<Vec<String>>,      // library names or paths to link against
}

#[derive(Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum PackageKind {
    #[serde(rename = "bin")]
    Bin,
    #[serde(rename = "lib")]
    Lib,
}

impl PackageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PackageKind::Bin => "bin",
            PackageKind::Lib => "lib",
        }
    }
    pub fn as_str_full(self) -> &'static str {
        match self {
            PackageKind::Bin => "executable",
            PackageKind::Lib => "library",
        }
    }
}
