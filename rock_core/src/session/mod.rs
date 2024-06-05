use crate::error::ErrorComp;
use crate::fs_env;
use crate::id_impl;
use crate::package;
use crate::package::manifest::{Manifest, PackageKind};
use crate::text::{self, TextRange};
use std::collections::HashMap;
use std::path::PathBuf;

//@package dependencies must be only lib packages 20.04.24
// bin package is the root package being compiled / checked
// currently bin deps are allowed, but their main() doesnt do anything special
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

    process_package(&mut session, &root_dir)?;
    process_package(&mut session, &cache_dir.join("core"))?;
    Ok(session)
}

//@errors arent work in progress, no context 06.06.24
// no checks if root exists before trying to parse the manifest
fn process_package(session: &mut Session, root_dir: &PathBuf) -> Result<(), ErrorComp> {
    let manifest_path = root_dir.join("Rock.toml");
    if !manifest_path.exists() {
        return Err(ErrorComp::message(format!(
            "could not find manifest `Rock.toml` in current directory\npath: `{}`",
            manifest_path.to_string_lossy()
        )));
    }
    let manifest_text = fs_env::file_read_to_string(&manifest_path)?;
    let manifest = package::manifest_deserialize(manifest_text, &manifest_path)?;

    if manifest.package.kind == PackageKind::Bin {
        return Err(ErrorComp::message(
            "cannot depend on executable package, only library dependencies are allowed",
        ));
    }

    let src_dir = root_dir.join("src");
    if !src_dir.exists() {
        return Err(ErrorComp::message(format!(
            "could not find `src` directory in current directory\npath: `{}`",
            src_dir.to_string_lossy()
        )));
    }
    let src_dir = fs_env::dir_read(&src_dir)?;

    let mut file_count: usize = 0;
    for entry in src_dir.flatten() {
        let path = entry.path();

        if path.is_file() && path.extension().unwrap_or_default() == "rock" {
            file_count += 1;
            let source = fs_env::file_read_to_string(&path)?;
            let line_ranges = text::find_line_ranges(&source);
            session.files.push(File {
                path,
                source,
                line_ranges,
            });
        }
    }

    session.packages.push(PackageData {
        file_count,
        manifest,
    });
    Ok(())
}
