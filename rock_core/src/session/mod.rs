use crate::ast;
use crate::error::ErrorComp;
use crate::fs_env;
use crate::intern::{InternLit, InternName, InternPool};
use crate::package;
use crate::package::manifest::{Manifest, PackageKind};
use crate::support::{IndexID, ID};
use crate::syntax::syntax_tree::SyntaxTree;
use crate::text::{self, TextRange};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct Session<'s> {
    pub cwd: PathBuf,
    pub intern_lit: InternPool<'s, InternLit>,
    pub intern_name: InternPool<'s, InternName>,
    pub pkg_storage: PackageStorage,
    //@storing separately from module for now
    pub module_asts: Vec<ast::Ast<'s>>,
    pub module_trees: Vec<Option<SyntaxTree<'s>>>,
}

pub struct PackageStorage {
    modules: Vec<RockModule>,
    packages: Vec<RockPackage>,
}

pub type PackageID = ID<RockPackage>;
pub struct RockPackage {
    pub name_id: ID<InternName>,
    pub root_dir: PathBuf,
    pub src: RockDirectory,
    manifest: Manifest,
    dependency_map: HashMap<ID<InternName>, PackageID>,
}

pub struct RockDirectory {
    pub name_id: ID<InternName>,
    pub path: PathBuf,
    modules: Vec<ModuleID>,
    sub_dirs: Vec<RockDirectory>,
}

pub type ModuleID = ID<RockModule>;
pub struct RockModule {
    pub name_id: ID<InternName>,
    pub path: PathBuf,
    pub source: String,
    pub line_ranges: Vec<TextRange>,
    pub package_id: PackageID,
}

pub enum ModuleOrDirectory<'src> {
    None,
    Module(ModuleID),
    Directory(&'src RockDirectory),
}

impl<'s> Session<'s> {
    pub const ROOT_ID: PackageID = PackageID::new_raw(0);

    pub fn new(building: bool) -> Result<Session<'s>, ErrorComp> {
        session_create(building)
    }
}

impl PackageStorage {
    pub fn module(&self, id: ModuleID) -> &RockModule {
        &self.modules.id_get(id)
    }
    pub fn module_ids(&self) -> impl Iterator<Item = ModuleID> {
        (0..self.modules.len()).map(ModuleID::new_raw)
    }
    pub fn package(&self, id: PackageID) -> &RockPackage {
        &self.packages.id_get(id)
    }
    pub fn package_ids(&self) -> impl Iterator<Item = PackageID> {
        (0..self.packages.len()).map(PackageID::new_raw)
    }
    pub fn find_module_by_path(&self, path: &PathBuf) -> Option<ModuleID> {
        for module_id in self.module_ids() {
            let module = self.module(module_id);
            if &module.path == path {
                return Some(module_id);
            }
        }
        None
    }
}

impl RockPackage {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn dependency(&self, name_id: ID<InternName>) -> Option<PackageID> {
        self.dependency_map.get(&name_id).copied()
    }
}

impl RockDirectory {
    pub fn find(&self, pkg_storage: &PackageStorage, name_id: ID<InternName>) -> ModuleOrDirectory {
        for module_id in self.modules.iter().copied() {
            let module = pkg_storage.module(module_id);
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

fn session_create<'s>(building: bool) -> Result<Session<'s>, ErrorComp> {
    let mut session = Session {
        cwd: fs_env::dir_get_current_working()?,
        intern_lit: InternPool::new(),
        intern_name: InternPool::new(),
        pkg_storage: PackageStorage {
            modules: Vec::new(),
            packages: Vec::new(),
        },
        module_asts: Vec::new(),
        module_trees: Vec::new(),
    };

    let root_dir = session.cwd.clone();
    let root_id = process_package(&mut session, &root_dir, false)?;
    let root_manifest = &session.pkg_storage.package(root_id).manifest;

    if building && root_manifest.package.kind == PackageKind::Lib {
        return Err(ErrorComp::message(
            r#"cannot build or run a library package
use `rock check` to check your library package,
or you can change [package] `kind` to `bin` in the Rock.toml manifest"#,
        ));
    }

    //@experimental resolver usage
    //package::resolver::resolve_dependencies(&root_manifest.dependencies)?;

    //@no package fetch (only using `$EXE_PATH/packages` directory)
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
        let package_id = process_package(&mut session, &cache_dir.join(dependency), true)?;
        let name_id = session.pkg_storage.package(package_id).name_id;
        root_dependency_map.insert(name_id, package_id);
    }

    //@only creating dependency map for root
    // package resultion process is not done yet
    session.pkg_storage.packages[0].dependency_map = root_dependency_map;
    Ok(session)
}

fn process_package(
    session: &mut Session,
    root_dir: &PathBuf,
    dependency: bool,
) -> Result<PackageID, ErrorComp> {
    let package_name = fs_env::filename_stem(root_dir)?;
    let name_id = session.intern_name.intern(package_name);

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

    let package_id = PackageID::new(&session.pkg_storage.packages);
    let src = process_directory(session, package_id, src_dir)?;

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
    session.pkg_storage.packages.push(package);
    Ok(package_id)
}

fn process_directory(
    session: &mut Session,
    package_id: PackageID,
    path: PathBuf,
) -> Result<RockDirectory, ErrorComp> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = session.intern_name.intern(filename);
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
                modules.push(process_file(session, package_id, entry_path)?);
            }
        } else if entry_path.is_dir() {
            sub_dirs.push(process_directory(session, package_id, entry_path)?);
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
    package_id: PackageID,
    path: PathBuf,
) -> Result<ModuleID, ErrorComp> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = session.intern_name.intern(filename);
    let source = read_file(&path)?;
    let line_ranges = text::find_line_ranges(&source);

    let module = RockModule {
        name_id,
        path,
        source,
        line_ranges,
        package_id,
    };

    let module_id = ModuleID::new(&session.pkg_storage.modules);
    session.pkg_storage.modules.push(module);
    Ok(module_id)
}

fn read_file(path: &PathBuf) -> Result<String, ErrorComp> {
    fs_env::file_read_to_string(path)
}
