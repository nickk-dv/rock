use crate::error::ErrorComp;
use crate::fs_env;
use crate::id_impl;
use crate::package;
use crate::text::{self, TextRange};
use std::path::PathBuf;

#[derive(Copy, Clone)]
pub enum BuildKind {
    Debug,
    Release,
}

impl BuildKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BuildKind::Debug => "debug",
            BuildKind::Release => "release",
        }
    }
}

//@package dependencies must be only lib packages @20.04.24
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
    manifest: package::Manifest,
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
    pub fn manifest(&self) -> &package::Manifest {
        &self.manifest
    }
}

fn create_session() -> Result<Session, ErrorComp> {
    let mut session = Session {
        cwd: fs_env::dir_get_current_working()?,
        files: Vec::new(),
        packages: Vec::new(),
    };

    let root: PathBuf = session.cwd.clone();
    process_package(&mut session, root)?;

    //@hardcoded core library dependency which is loaded always @19.04.24
    // process unique packages based on dependency graph of main package's manifest
    let mut core_root = fs_env::current_exe_path()?;
    core_root.pop();
    core_root.push("core");
    process_package(&mut session, core_root)?;

    Ok(session)
}

//@errors arent perfect for core / root / dependency packages @19.04.24
// no checks if root exists at all, trying to parse manifest right away
fn process_package(session: &mut Session, root: PathBuf) -> Result<(), ErrorComp> {
    let manifest_path = root.join("Rock.toml");
    if !manifest_path.exists() {
        return Err(ErrorComp::message(format!(
            "could not find manifest `Rock.toml` in current directory\npath: `{}`",
            manifest_path.to_string_lossy()
        )));
    }

    let manifest = fs_env::file_read_to_string(&manifest_path)?;
    let manifest: package::Manifest = basic_toml::from_str(&manifest).map_err(|error| {
        ErrorComp::message(format!(
            "could not parse manifest `{}`\nreason: {}",
            manifest_path.to_string_lossy(),
            error
        ))
    })?;

    let src_dir = root.join("src");
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
            let source = fs_env::file_read_to_string(&path)?;
            let line_ranges = text::find_line_ranges(&source);
            file_count += 1;
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
