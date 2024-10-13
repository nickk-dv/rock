mod graph;
mod vfs;

use crate::ast::Ast;
use crate::error::Error;
use crate::errors as err;
use crate::fs_env;
use crate::intern::{InternLit, InternName, InternPool};
use crate::package;
use crate::package::manifest::Manifest;
use crate::support::ID;
use crate::syntax::syntax_tree::SyntaxTree;
use graph::PackageGraph;
use std::path::PathBuf;
use vfs::{FileID, Vfs};

pub struct Session<'s> {
    pub vfs: Vfs,
    pub curr_exe_dir: PathBuf,
    pub curr_work_dir: PathBuf,
    pub intern_lit: InternPool<'s, InternLit>,
    pub intern_name: InternPool<'s, InternName>,
    pub graph: PackageGraph,
    modules: Vec<Module<'s>>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct PackageID(u32);
pub struct Package {
    name_id: ID<InternName>,
    src: Directory,
    manifest: Manifest,
    deps: Vec<PackageID>,
    modules: Vec<ModuleID>,
}

#[derive(Copy, Clone)]
pub struct ModuleID(u32);
pub struct Module<'s> {
    origin: PackageID,
    name_id: ID<InternName>,
    file_id: FileID,
    tree: Option<SyntaxTree<'s>>,
    ast: Option<Ast<'s>>,
}

pub struct Directory {
    name_id: ID<InternName>,
    modules: Vec<ModuleID>,
    sub_dirs: Vec<Directory>,
}

impl<'s> Session<'s> {
    #[inline]
    pub fn module(&self, module_id: ModuleID) -> &Module<'s> {
        &self.modules[module_id.index()]
    }
    #[inline]
    pub fn module_mut(&mut self, module_id: ModuleID) -> &mut Module<'s> {
        &mut self.modules[module_id.index()]
    }

    #[must_use]
    fn add(&mut self, module: Module<'s>) -> ModuleID {
        let module_id = ModuleID(self.modules.len() as u32);
        self.modules.push(module);
        module_id
    }
}

impl Package {
    #[inline]
    pub fn name(&self) -> ID<InternName> {
        self.name_id
    }
    #[inline]
    pub fn src(&self) -> &Directory {
        &self.src
    }
    #[inline]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    #[inline]
    pub fn dep_ids(&self) -> &[PackageID] {
        &self.deps
    }
    #[inline]
    pub fn module_ids(&self) -> &[ModuleID] {
        &self.modules
    }
}

impl PackageID {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

impl<'s> Module<'s> {
    #[inline]
    pub fn origin(&self) -> PackageID {
        self.origin
    }
    #[inline]
    pub fn name(&self) -> ID<InternName> {
        self.name_id
    }
    #[inline]
    pub fn file_id(&self) -> FileID {
        self.file_id
    }

    #[inline]
    pub fn tree(&self) -> Option<&SyntaxTree<'s>> {
        self.tree.as_ref()
    }
    #[inline]
    pub fn tree_expect(&self) -> &SyntaxTree<'s> {
        self.tree.as_ref().unwrap()
    }
    #[inline]
    pub fn ast(&self) -> Option<&SyntaxTree<'s>> {
        self.tree.as_ref()
    }
    #[inline]
    pub fn ast_expect(&self) -> &Ast<'s> {
        self.ast.as_ref().unwrap()
    }

    #[inline]
    pub fn set_ast<'ast: 's>(&mut self, ast: Ast<'ast>) {
        self.ast = Some(ast);
    }
    #[inline]
    pub fn set_tree<'syn: 's>(&mut self, tree: SyntaxTree<'syn>) {
        self.tree = Some(tree);
    }
    #[inline]
    pub fn unload_ast(&mut self) {
        self.ast = None;
    }
    #[inline]
    pub fn unload_tree(&mut self) {
        self.tree = None;
    }
}

impl ModuleID {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

pub enum ModuleOrDirectory<'s> {
    None,
    Module(ModuleID),
    Directory(&'s Directory),
}

impl Directory {
    #[inline]
    pub fn name(&self) -> ID<InternName> {
        self.name_id
    }

    pub fn find(&self, session: &Session, name_id: ID<InternName>) -> ModuleOrDirectory {
        for module_id in self.modules.iter().copied() {
            if session.module(module_id).name_id == name_id {
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

pub fn create_session<'s>() -> Result<Session<'s>, Error> {
    let mut session = Session {
        vfs: Vfs::new(128),
        curr_exe_dir: fs_env::current_exe_path()?,
        curr_work_dir: fs_env::dir_get_current_working()?,
        intern_lit: InternPool::new(512),
        intern_name: InternPool::new(1024),
        graph: PackageGraph::new(16),
        modules: Vec::with_capacity(128),
    };

    let root_dir = session.curr_work_dir.clone();
    process_package(&mut session, &root_dir, None)?;
    Ok(session)
}

fn process_package(
    session: &mut Session,
    root_dir: &PathBuf,
    dep_from: Option<PackageID>,
) -> Result<PackageID, Error> {
    if dep_from.is_some() && !root_dir.exists() {
        return Err(err::session_pkg_not_found(root_dir));
    }

    let manifest_path = root_dir.join("Rock.toml");
    if !manifest_path.exists() {
        return Err(err::session_manifest_not_found(root_dir));
    }

    let src_dir = root_dir.join("src");
    if !src_dir.exists() {
        return Err(err::session_src_not_found(root_dir));
    }

    let manifest = fs_env::file_read_to_string(&manifest_path)?;
    let manifest = package::manifest_deserialize(&manifest, &manifest_path)?;
    let name_id = session.intern_name.intern(&manifest.package.name);
    //@check if dep_from.is_some() & package is binary

    let package_id = session.graph.next_id();
    let src = process_directory(session, &src_dir, package_id)?;
    let modules = directory_all_modules(&src);

    //@forced to clone to avoid a valid borrow issue, not store [dependencies] key? move out?
    let dependencies = manifest.dependencies.clone();
    #[rustfmt::skip]
    let package = Package { name_id, src, manifest, deps: vec![], modules };
    let package_id = session.graph.add(package);

    //@semver not used, dependency package may be duplicated
    // no unique name set or `already processed` detection exists
    //@dedup by root_path? might be overall effective
    for (dep_name, _) in dependencies.iter() {
        let dep_root_dir = session.curr_exe_dir.join("packages").join(dep_name);
        let dep_id = process_package(session, &dep_root_dir, Some(package_id))?;
        //@no cycle possible without dedup detection
        session
            .graph
            .add_dep(package_id, dep_id, &session.intern_name)?;
    }
    Ok(package_id)
}

fn process_directory(
    session: &mut Session,
    path: &PathBuf,
    origin: PackageID,
) -> Result<Directory, Error> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = session.intern_name.intern(filename);

    let mut modules = Vec::new();
    let mut sub_dirs = Vec::new();

    let read_dir = fs_env::dir_read(&path)?;
    for entry_result in read_dir {
        let entry = fs_env::dir_entry_validate(&path, entry_result)?;
        let entry_path = entry.path();

        if entry_path.is_file() {
            let extension = fs_env::file_extension(&entry_path);
            if matches!(extension, Some("rock")) {
                let module_id = process_module(session, &entry_path, origin)?;
                modules.push(module_id);
            }
        } else if entry_path.is_dir() {
            let sub_dir = process_directory(session, &entry_path, origin)?;
            sub_dirs.push(sub_dir);
        } else {
            panic!("internal: unknown file type")
        }
    }

    #[rustfmt::skip]
    let dir = Directory { name_id, modules, sub_dirs };
    Ok(dir)
}

fn process_module(
    session: &mut Session,
    path: &PathBuf,
    origin: PackageID,
) -> Result<ModuleID, Error> {
    let filename = fs_env::filename_stem(&path)?;
    let name_id = session.intern_name.intern(filename);

    let source = fs_env::file_read_to_string(path)?;
    let file_id = session.vfs.open(path, source);

    #[rustfmt::skip]
    let module = Module { origin, name_id, file_id, tree: None, ast: None };
    let module_id = session.add(module);
    Ok(module_id)
}

fn directory_all_modules(dir: &Directory) -> Vec<ModuleID> {
    let mut modules = Vec::new();
    let mut dir_stack = vec![dir];

    while let Some(dir) = dir_stack.pop() {
        modules.extend(&dir.modules);
        for sub_dir in dir.sub_dirs.iter().rev() {
            dir_stack.push(sub_dir);
        }
    }
    modules
}
