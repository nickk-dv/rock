pub mod manifest;
pub mod semver;

use crate::error::ErrorComp;
use std::path::PathBuf;

pub fn manifest_serialize(manifest: &manifest::Manifest) -> Result<String, ErrorComp> {
    basic_toml::to_string(manifest).map_err(|error| {
        ErrorComp::message(format!(
            "failed to serialize manifest file\nreason: {}",
            error
        ))
    })
}

pub fn manifest_deserialize(
    manifest: String,
    manifest_path: &PathBuf,
) -> Result<manifest::Manifest, ErrorComp> {
    basic_toml::from_str(&manifest).map_err(|error| {
        ErrorComp::message(format!(
            "failed to parse manifest file: `{}`\nreason: {}",
            manifest_path.to_string_lossy(),
            error
        ))
    })
}
