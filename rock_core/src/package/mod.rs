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

pub fn verify_name(name: &str) -> Result<&str, ErrorComp> {
    const MAX_NAME_LEN: usize = 32;

    if name.len() > MAX_NAME_LEN {
        return Err(ErrorComp::message(format!(
            "package name cannot exceed {} characters",
            MAX_NAME_LEN
        )));
    }

    let fc = match name.chars().next() {
        Some(c) => c,
        None => return Err(ErrorComp::message("package name cannot be empty")),
    };

    for c in name.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(
                ErrorComp::message(format!("package name contains invalid character `{c}`\nallowed characters: lowercase `a-z`, digits `0-9` and `_`")),
            );
        }
    }

    if !fc.is_ascii_lowercase() {
        return Err(ErrorComp::message(format!(
            "package name cannot start with `{fc}`"
        )));
    }

    Ok(name)
}
