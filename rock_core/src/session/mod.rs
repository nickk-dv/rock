use crate::error::ErrorComp;
use crate::fs_env;
use crate::id_impl;
use crate::package;
use crate::package::manifest::{Manifest, PackageKind};
use crate::text::{self, TextRange};
use std::path::PathBuf;

pub struct Session {
    cwd: PathBuf,
    files: Vec<File>,
    packages: Vec<PackageData>,
}

id_impl!(FileID);
pub struct File {
    pub path: PathBuf,
    pub source: String,
    pub line_ranges: Vec<TextRange>,
}

id_impl!(PackageID);
pub struct PackageData {
    file_count: usize,
    manifest: Manifest,
}

impl Session {
    pub fn new() -> Result<Session, ErrorComp> {
        create_session()
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }
    pub fn file(&self, id: FileID) -> &File {
        &self.files[id.index()]
    }
    pub fn file_ids(&self) -> impl Iterator<Item = FileID> {
        (0..self.files.len()).map(FileID::new)
    }
    pub fn package(&self, id: PackageID) -> &PackageData {
        &self.packages[id.index()]
    }
    pub fn package_ids(&self) -> impl Iterator<Item = PackageID> {
        (0..self.packages.len()).map(PackageID::new)
    }

    pub fn root_package_bin_name(&self) -> String {
        let manifest = &self.packages[0].manifest;
        if let Some(build) = &manifest.build {
            build.bin_name.clone()
        } else {
            manifest.package.name.clone()
        }
    }
}

impl PackageData {
    pub fn file_count(&self) -> usize {
        self.file_count
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

fn create_session() -> Result<Session, ErrorComp> {
    let mut session = Session {
        cwd: fs_env::dir_get_current_working()?,
        files: Vec::new(),
        packages: Vec::new(),
    };

    let root_dir = session.cwd.clone();
    let mut cache_dir = fs_env::current_exe_path()?;
    cache_dir.pop();
    cache_dir.push("packages");

    let mut all_packages = Vec::with_capacity(32);
    let mut all_files = Vec::with_capacity(32);

    let (root_data, files) = process_package(&root_dir, false)?;
    all_files.extend(files);

    for (name, _) in root_data.manifest.dependencies.iter() {
        let (data, files) = process_package(&cache_dir.join(name), true)?;
        all_packages.push(data);
        all_files.extend(files);
    }

    session.files.extend(all_files);
    session.packages.push(root_data);
    session.packages.extend(all_packages);
    Ok(session)
}

fn process_package(
    root_dir: &PathBuf,
    dependency: bool,
) -> Result<(PackageData, Vec<File>), ErrorComp> {
    if dependency && !root_dir.exists() {
        return Err(ErrorComp::message(format!(
            "could not find package directory, package fetch is not yet implemented\nexpected path: `{}`",
            root_dir.to_string_lossy()
        )));
    }

    let manifest_path = root_dir.join("Rock.toml");
    if !manifest_path.exists() {
        let in_message = if dependency { "dependency" } else { "current" };
        return Err(ErrorComp::message(format!(
            "could not find manifest `Rock.toml` in {in_message} directory\npath: `{}`",
            manifest_path.to_string_lossy()
        )));
    }

    let manifest_text = fs_env::file_read_to_string(&manifest_path)?;
    let manifest = package::manifest_deserialize(manifest_text, &manifest_path)?;

    if dependency && manifest.package.kind == PackageKind::Bin {
        return Err(ErrorComp::message(
            "cannot depend on executable package, only library dependencies are allowed",
        ));
    }

    let src_dir = root_dir.join("src");
    if !src_dir.exists() {
        let in_message = if dependency { "dependency" } else { "current" };
        return Err(ErrorComp::message(format!(
            "could not find `src` directory in {in_message} directory\npath: `{}`",
            src_dir.to_string_lossy()
        )));
    }
    let src_dir = fs_env::dir_read(&src_dir)?;

    let mut files = Vec::new();
    let mut file_count: usize = 0;

    for entry in src_dir.flatten() {
        let path = entry.path();

        if path.is_file() && path.extension().unwrap_or_default() == "rock" {
            file_count += 1;
            let source = fs_env::file_read_to_string(&path)?;
            let line_ranges = text::find_line_ranges(&source);
            files.push(File {
                path,
                source,
                line_ranges,
            });
        }
    }

    let package = PackageData {
        file_count,
        manifest,
    };
    Ok((package, files))
}
