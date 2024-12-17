pub mod manifest;
pub mod resolver;
pub mod semver;

use crate::error::Error;
use std::path::PathBuf;

pub fn manifest_serialize(manifest: &manifest::Manifest) -> Result<String, Error> {
    basic_toml::to_string(manifest).map_err(|error| {
        Error::message(format!("failed to serialize manifest file\nreason: {}", error))
    })
}

pub fn manifest_deserialize(
    manifest: &str,
    manifest_path: &PathBuf,
) -> Result<manifest::Manifest, Error> {
    basic_toml::from_str(manifest).map_err(|error| {
        Error::message(format!(
            "failed to parse manifest file: `{}`\nreason: {}",
            manifest_path.to_string_lossy(),
            error
        ))
    })
}

pub fn index_manifest_serialize(manifest: &manifest::IndexManifest) -> Result<String, Error> {
    serde_json::to_string(manifest).map_err(|error| {
        Error::message(format!("failed to serialize index manifest file\nreason: {}", error))
    })
}

pub fn index_manifest_deserialize(
    manifest: String,
    manifest_path: &PathBuf,
) -> Result<Vec<manifest::IndexManifest>, Error> {
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

pub fn verify_name(name: &str) -> Result<&str, Error> {
    const MAX_NAME_LEN: usize = 32;

    if name.len() > MAX_NAME_LEN {
        return Err(Error::message(format!(
            "package name cannot exceed {} characters",
            MAX_NAME_LEN
        )));
    }

    let fc = match name.chars().next() {
        Some(c) => c,
        None => return Err(Error::message("package name cannot be empty")),
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
