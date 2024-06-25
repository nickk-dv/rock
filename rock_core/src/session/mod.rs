use crate::error::ErrorComp;
use crate::fs_env;
use crate::id_impl;
use crate::intern::{InternID, InternPool};
use crate::package;
use crate::package::manifest::{Manifest, PackageKind};
use crate::text::{self, TextRange};
use std::collections::HashMap;
use std::path::PathBuf;

struct SessionV2 {
    cwd: PathBuf,
    files: Vec<RockFile>,
    packages: Vec<RockPackage>,
}

struct RockPackage {
    pub name_id: InternID,
    pub root_dir: PathBuf,
    pub src: RockDirectory,
    pub manifest: Manifest,
}

struct RockDirectory {
    pub name_id: InternID,
    pub path: PathBuf,
    pub files: Vec<FileID>,
    pub sub_dirs: Vec<RockDirectory>,
}

struct RockFile {
    pub name_id: InternID,
    pub path: PathBuf,
    pub source: String,
    pub line_ranges: Vec<TextRange>,
}

impl SessionV2 {
    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }
    pub fn file(&self, id: FileID) -> &RockFile {
        &self.files[id.index()]
    }
    pub fn file_ids(&self) -> impl Iterator<Item = FileID> {
        (0..self.files.len()).map(FileID::new)
    }
    pub fn package(&self, id: PackageID) -> &RockPackage {
        &self.packages[id.index()]
    }
    pub fn package_ids(&self) -> impl Iterator<Item = PackageID> {
        (0..self.packages.len()).map(PackageID::new)
    }
}

//@create `dependency_map: HashMap<InternID, PackageID>` for RockPackage
//@store file_count to be able to iterate over FileIDs or ModuleIDs of specific package
//@store PackageID in RockFile to know module and file PackageID
fn session_create<'intern>(building: bool) -> Result<(SessionV2, InternPool<'intern>), ErrorComp> {
    let mut session = SessionV2 {
        cwd: fs_env::dir_get_current_working()?,
        files: Vec::new(),
        packages: Vec::new(),
    };
    let mut intern_name = InternPool::new();

    let root_dir = session.cwd.clone();
    let root_id = process_package_v2(&mut session, &mut intern_name, &root_dir, false)?;
    let root_manifest = &session.package(root_id).manifest;

    if building && root_manifest.package.kind == PackageKind::Lib {
        return Err(ErrorComp::message(
            r#"cannot build or run a library package
use `rock check` to check your library package,
or you can change [package] `kind` to `bin` in the Rock.toml manifest"#,
        ));
    }

    //@no package fetch (only using `$PATH/packages` directory)
    let mut cache_dir = fs_env::current_exe_path()?;
    cache_dir.pop();
    cache_dir.push("packages");

    //@no version resolution or transitive dependencies (only root deps)
    let root_dependencies: Vec<String> = root_manifest
        .dependencies
        .iter()
        .map(|(name, _)| name.clone())
        .collect();

    for dependency in root_dependencies.iter() {
        process_package_v2(
            &mut session,
            &mut intern_name,
            &cache_dir.join(dependency),
            true,
        )?;
    }

    Ok((session, intern_name))
}

fn process_package_v2(
    session: &mut SessionV2,
    intern_name: &mut InternPool,
    root_dir: &PathBuf,
    dependency: bool,
) -> Result<PackageID, ErrorComp> {
    let package_name = fs_env::filename_stem(root_dir)?;
    let name_id = intern_name.intern(package_name);

    if dependency && !root_dir.exists() {
        return Err(ErrorComp::message(format!(
            "could not find package directory, package fetch is not yet implemented\nexpected path: `{}`",
            root_dir.to_string_lossy()
        )));
    }

    let manifest_path = root_dir.join("Rock.toml");
    if !manifest_path.exists() {
        let in_kind = if dependency { "dependency" } else { "current" };
        return Err(ErrorComp::message(format!(
            "could not find manifest `Rock.toml` in {in_kind} directory\npath: `{}`",
            manifest_path.to_string_lossy()
        )));
    }

    let manifest_text = fs_env::file_read_to_string(&manifest_path)?;
    let manifest = package::manifest_deserialize(manifest_text, &manifest_path)?;
    if dependency && manifest.package.kind == PackageKind::Bin {
        //@which dependency and for which package and where? not enough information
        return Err(ErrorComp::message(
            "cannot depend on executable package, only library dependencies are allowed",
        ));
    }

    let src_dir = root_dir.join("src");
    if !src_dir.exists() {
        //@duplicate, standardize `in` `kind` directory vs package messaging
        // for package related errors
        let in_kind = if dependency { "dependency" } else { "current" };
        return Err(ErrorComp::message(format!(
            "could not find `src` directory in {in_kind} directory\npath: `{}`",
            src_dir.to_string_lossy()
        )));
    }
    let src = process_directory(session, intern_name, src_dir)?;

    if let Some(lib_paths) = &manifest.build.lib_paths {
        let location = format!(
            "\nmanifest path: `{}`\nmanifest key: [build] `lib_paths`",
            manifest_path.to_string_lossy()
        );
        //@relative path doesnt guarantee that libraries
        // are located within the same package (eg: ../../dir)
        for path in lib_paths {
            if !path.is_relative() {
                return Err(ErrorComp::message(format!(
                    "library path `{}` must be relative{location}",
                    path.to_string_lossy()
                )));
            }
            let lib_path = root_dir.join(path);
            if !lib_path.exists() {
                return Err(ErrorComp::message(format!(
                    "library path `{}` does not exist{location}",
                    lib_path.to_string_lossy()
                )));
            }
            if !lib_path.is_dir() {
                return Err(ErrorComp::message(format!(
                    "library path `{}` must be a directory{location}",
                    lib_path.to_string_lossy()
                )));
            }
        }
    }

    let package = RockPackage {
        name_id,
        root_dir: root_dir.clone(),
        src,
        manifest,
    };

    let package_id = PackageID::new(session.packages.len());
    session.packages.push(package);
    Ok(package_id)
}

fn process_directory(
    session: &mut SessionV2,
    intern_name: &mut InternPool,
    path: PathBuf,
) -> Result<RockDirectory, ErrorComp> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = intern_name.intern(filename);
    let mut files = Vec::new();
    let mut sub_dirs = Vec::new();

    let read_dir = fs_env::dir_read(&path)?;
    for entry_result in read_dir {
        let entry = fs_env::dir_entry_validate(&path, entry_result)?;
        let entry_path = entry.path();
        fs_env::symlink_forbid(&entry_path)?;

        if entry_path.is_file() {
            files.push(process_file(session, intern_name, entry_path)?);
        } else if entry_path.is_dir() {
            sub_dirs.push(process_directory(session, intern_name, entry_path)?);
        } else {
            unreachable!()
        }
    }

    let directory = RockDirectory {
        name_id,
        path,
        files,
        sub_dirs,
    };
    Ok(directory)
}

fn process_file(
    session: &mut SessionV2,
    intern_name: &mut InternPool,
    path: PathBuf,
) -> Result<FileID, ErrorComp> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = intern_name.intern(filename);
    let source = fs_env::file_read_to_string(&path)?;
    let line_ranges = text::find_line_ranges(&source);

    let file = RockFile {
        name_id,
        path,
        source,
        line_ranges,
    };

    let file_id = FileID::new(session.files.len());
    session.files.push(file);
    Ok(file_id)
}

//@old session

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
    root_dir: PathBuf,
    file_count: usize,
    manifest: Manifest,
}

impl Session {
    pub fn new(
        building: bool,
        files_in_memory: Option<&HashMap<PathBuf, String>>,
    ) -> Result<Session, ErrorComp> {
        create_session(building, files_in_memory)
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

    fn root_manifest(&self) -> &Manifest {
        &self.packages[0].manifest
    }
    pub fn is_executable(&self) -> bool {
        self.root_manifest().package.kind == PackageKind::Bin
    }
    pub fn root_package_bin_name(&self) -> String {
        let manifest = self.root_manifest();
        if let Some(bin_name) = &manifest.build.bin_name {
            return bin_name.clone();
        } else {
            manifest.package.name.clone()
        }
    }
}

impl PackageData {
    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }
    pub fn file_count(&self) -> usize {
        self.file_count
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

fn create_session(
    building: bool,
    files_in_memory: Option<&HashMap<PathBuf, String>>,
) -> Result<Session, ErrorComp> {
    let s2_test = session_create(building)?;

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

    let (root_data, files) = process_package(&root_dir, files_in_memory, false)?;
    all_files.extend(files);

    if building && root_data.manifest.package.kind == PackageKind::Lib {
        return Err(ErrorComp::message(
            r#"cannot build or run a library package
use `rock check` to check your library package,
or you can change [package] `kind` to `bin` in the Rock.toml manifest"#,
        ));
    }

    for (name, _) in root_data.manifest.dependencies.iter() {
        let (data, files) = process_package(&cache_dir.join(name), files_in_memory, true)?;
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
    files_in_memory: Option<&HashMap<PathBuf, String>>,
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

            let source = if let Some(files_in_memory) = files_in_memory {
                if let Some(source) = files_in_memory.get(&path) {
                    source.clone()
                } else {
                    fs_env::file_read_to_string(&path)?
                }
            } else {
                fs_env::file_read_to_string(&path)?
            };

            let line_ranges = text::find_line_ranges(&source);
            files.push(File {
                path,
                source,
                line_ranges,
            });
        }
    }

    let location = format!(
        "\nmanifest path: `{}`\nmanifest key: [build] `lib_paths`",
        manifest_path.to_string_lossy()
    );

    if let Some(lib_paths) = &manifest.build.lib_paths {
        for path in lib_paths {
            if !path.is_relative() {
                return Err(ErrorComp::message(format!(
                    "library path `{}` must be relative{location}",
                    path.to_string_lossy()
                )));
            }
            let lib_path = root_dir.join(path);
            if !lib_path.exists() {
                return Err(ErrorComp::message(format!(
                    "library path `{}` does not exist{location}",
                    lib_path.to_string_lossy()
                )));
            }
            if !lib_path.is_dir() {
                return Err(ErrorComp::message(format!(
                    "library path `{}` must be a directory{location}",
                    lib_path.to_string_lossy()
                )));
            }
        }
    }

    let package = PackageData {
        root_dir: root_dir.clone(),
        file_count,
        manifest,
    };
    Ok((package, files))
}
