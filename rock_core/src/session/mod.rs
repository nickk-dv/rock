use crate::error::ErrorComp;
use crate::fs_env;
use crate::id_impl;
use crate::intern::{InternID, InternPool};
use crate::package;
use crate::package::manifest::{Manifest, PackageKind};
use crate::text::{self, TextRange};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Session {
    cwd: PathBuf,
    modules: Vec<RockModule>,
    packages: Vec<RockPackage>,
}

id_impl!(PackageID);
pub struct RockPackage {
    pub name_id: InternID,
    pub root_dir: PathBuf,
    pub src: RockDirectory,
    manifest: Manifest,
    dependency_map: HashMap<InternID, PackageID>,
}

pub struct RockDirectory {
    pub name_id: InternID,
    pub path: PathBuf,
    modules: Vec<ModuleID>,
    sub_dirs: Vec<RockDirectory>,
}

id_impl!(ModuleID);
pub struct RockModule {
    pub name_id: InternID,
    pub path: PathBuf,
    pub source: String,
    pub line_ranges: Vec<TextRange>,
    pub package_id: PackageID,
}

impl Session {
    pub const ROOT_ID: PackageID = PackageID::new(0);

    pub fn new<'intern>(
        building: bool,
        file_cache: Option<&HashMap<PathBuf, String>>,
    ) -> Result<(Session, InternPool<'intern>), ErrorComp> {
        session_create(building, file_cache)
    }

    pub fn cwd(&self) -> &PathBuf {
        &self.cwd
    }
    pub fn module(&self, id: ModuleID) -> &RockModule {
        &self.modules[id.index()]
    }
    pub fn module_ids(&self) -> impl Iterator<Item = ModuleID> {
        (0..self.modules.len()).map(ModuleID::new)
    }
    pub fn package(&self, id: PackageID) -> &RockPackage {
        &self.packages[id.index()]
    }
    pub fn package_ids(&self) -> impl Iterator<Item = PackageID> {
        (0..self.packages.len()).map(PackageID::new)
    }
}

impl RockPackage {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn dependency(&self, name_id: InternID) -> Option<PackageID> {
        self.dependency_map.get(&name_id).copied()
    }
}

pub enum ModuleOrDirectory<'src> {
    None,
    Module(ModuleID),
    Directory(&'src RockDirectory),
}

impl RockDirectory {
    pub fn find(&self, session: &Session, name_id: InternID) -> ModuleOrDirectory {
        for module_id in self.modules.iter().copied() {
            let module = session.module(module_id);
            if module.name_id == name_id {
                return ModuleOrDirectory::Module(module_id);
            }
        }
        for directory in self.sub_dirs.iter() {
            if directory.name_id == name_id {
                return ModuleOrDirectory::Directory(directory);
            }
        }
        ModuleOrDirectory::None
    }
}

//@store file_count to be able to iterate over FileIDs or ModuleIDs of specific package
fn session_create<'intern>(
    building: bool,
    file_cache: Option<&HashMap<PathBuf, String>>,
) -> Result<(Session, InternPool<'intern>), ErrorComp> {
    let mut session = Session {
        cwd: fs_env::dir_get_current_working()?,
        modules: Vec::new(),
        packages: Vec::new(),
    };
    let mut intern_name = InternPool::new();

    let root_dir = session.cwd.clone();
    let root_id = process_package(&mut session, &mut intern_name, file_cache, &root_dir, false)?;
    let root_manifest = &session.package(root_id).manifest;

    if building && root_manifest.package.kind == PackageKind::Lib {
        return Err(ErrorComp::message(
            r#"cannot build or run a library package
use `rock check` to check your library package,
or you can change [package] `kind` to `bin` in the Rock.toml manifest"#,
        ));
    }

    //@experimental resolver usage
    package::resolver::resolve_dependencies(&root_manifest.dependencies)?;

    //@no package fetch (only using `$PATH/packages` directory)
    let mut cache_dir = fs_env::current_exe_path()?;
    cache_dir.push("packages");

    //@no version resolution or transitive dependencies (only root deps)
    let root_dependencies: Vec<String> = root_manifest
        .dependencies
        .keys()
        .map(|name| name.clone())
        .collect();
    let mut root_dependency_map = HashMap::new();

    for dependency in root_dependencies.iter() {
        let package_id = process_package(
            &mut session,
            &mut intern_name,
            file_cache,
            &cache_dir.join(dependency),
            true,
        )?;
        let name_id = session.package(package_id).name_id;
        root_dependency_map.insert(name_id, package_id);
    }

    //@only creating dependency map for root
    // package resultion process is not done yet
    session.packages[0].dependency_map = root_dependency_map;
    Ok((session, intern_name))
}

fn process_package(
    session: &mut Session,
    intern_name: &mut InternPool,
    file_cache: Option<&HashMap<PathBuf, String>>,
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
    let package_id = PackageID::new(session.packages.len());
    let src = process_directory(session, intern_name, file_cache, package_id, src_dir)?;

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
        dependency_map: HashMap::new(), //@no deps are set
    };
    session.packages.push(package);
    Ok(package_id)
}

fn process_directory(
    session: &mut Session,
    intern_name: &mut InternPool,
    file_cache: Option<&HashMap<PathBuf, String>>,
    package_id: PackageID,
    path: PathBuf,
) -> Result<RockDirectory, ErrorComp> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = intern_name.intern(filename);
    let mut modules = Vec::new();
    let mut sub_dirs = Vec::new();

    let read_dir = fs_env::dir_read(&path)?;
    for entry_result in read_dir {
        let entry = fs_env::dir_entry_validate(&path, entry_result)?;
        let entry_path = entry.path();
        fs_env::symlink_forbid(&entry_path)?;

        if entry_path.is_file() {
            let extension = fs_env::file_extension(&entry_path);
            if matches!(extension, Some("rock")) {
                modules.push(process_file(
                    session,
                    intern_name,
                    file_cache,
                    package_id,
                    entry_path,
                )?);
            }
        } else if entry_path.is_dir() {
            sub_dirs.push(process_directory(
                session,
                intern_name,
                file_cache,
                package_id,
                entry_path,
            )?);
        } else {
            unreachable!()
        }
    }

    let directory = RockDirectory {
        name_id,
        path,
        modules,
        sub_dirs,
    };
    Ok(directory)
}

fn process_file(
    session: &mut Session,
    intern_name: &mut InternPool,
    file_cache: Option<&HashMap<PathBuf, String>>,
    package_id: PackageID,
    path: PathBuf,
) -> Result<ModuleID, ErrorComp> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = intern_name.intern(filename);
    let source = read_file(&path, file_cache)?;
    let line_ranges = text::find_line_ranges(&source);

    let module = RockModule {
        name_id,
        path,
        source,
        line_ranges,
        package_id,
    };

    let module_id = ModuleID::new(session.modules.len());
    session.modules.push(module);
    Ok(module_id)
}

fn read_file(
    path: &PathBuf,
    file_cache: Option<&HashMap<PathBuf, String>>,
) -> Result<String, ErrorComp> {
    if let Some(file_cache) = file_cache {
        if let Some(source) = file_cache.get(path) {
            return Ok(source.clone());
        }
    }
    fs_env::file_read_to_string(path)
}
