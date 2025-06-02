pub mod config;
mod graph;
pub mod manifest;
pub mod vfs;

use crate::ast::Ast;
use crate::error::{
    DiagnosticData, Error, ErrorBuffer, ErrorSink, ErrorWarningBuffer, Warning, WarningSink,
};
use crate::errors as err;
use crate::intern::{InternPool, LitID, NameID};
use crate::support::os;
use crate::syntax::ast_build::AstBuildState;
use crate::syntax::syntax_tree::SyntaxTree;
use manifest::{Dependency, Manifest, PackageKind};
use std::path::PathBuf;
pub use vfs::FileData;
use vfs::{FileID, Vfs};

pub struct Session<'s> {
    pub vfs: Vfs,
    pub curr_exe_dir: PathBuf,
    pub curr_work_dir: PathBuf,
    pub intern_lit: InternPool<'s, LitID>,
    pub intern_name: InternPool<'s, NameID>,
    pub graph: graph::PackageGraph,
    pub module: Modules<'s>,
    pub stats: BuildStats,
    pub config: config::Config,
    pub errors: ErrorBuffer,
    pub root_id: PackageID,
    pub ast_state: AstBuildState<'s>,
}

pub struct Modules<'s> {
    modules: Vec<Module<'s>>,
}

crate::define_id!(pub PackageID);
pub struct Package {
    pub root_dir: PathBuf,
    pub name_id: NameID,
    pub src: Directory,
    pub manifest: Manifest,
    pub deps: Vec<PackageID>,
}

crate::define_id!(pub ModuleID);
pub struct Module<'s> {
    pub origin: PackageID,
    pub name_id: NameID,
    pub file_id: FileID,
    tree: Option<SyntaxTree<'s>>,
    pub tree_version: u32,
    ast: Option<Ast<'s>>,
    pub ast_version: u32,
    pub parse_errors: ErrorBuffer,
    pub errors: ErrorWarningBuffer,
}

pub struct Directory {
    name_id: NameID,
    modules: Vec<ModuleID>,
    sub_dirs: Vec<Directory>,
}

#[derive(Default)]
pub struct BuildStats {
    pub package_count: u32,
    pub module_count: u32,
    pub line_count: u32,
    pub token_count: u32,
    pub session_ms: f64,
    pub parse_ms: f64,
    pub check_ms: f64,
    pub llvm_ir_ms: f64,
    pub object_ms: f64,
    pub link_ms: f64,
}

pub const CORE_PACKAGE_ID: PackageID = PackageID(0);

impl<'s> Session<'s> {
    pub fn result(&self) -> Result<(), ()> {
        if self.errors.did_error(0) {
            return Err(());
        }
        for module in &self.module.modules {
            if module.parse_errors.did_error(0) {
                return Err(());
            }
            if module.errors.did_error(0) {
                return Err(());
            }
        }
        Ok(())
    }

    pub fn move_errors(&mut self, errors: Vec<Error>, warnings: Vec<Warning>) {
        for e in errors {
            let origin = match e.diagnostic().data() {
                DiagnosticData::Message => {
                    self.errors.error(e);
                    continue;
                }
                DiagnosticData::Context { main, .. } => main.src().module_id(),
                DiagnosticData::ContextVec { main, .. } => main.src().module_id(),
            };
            self.module.get_mut(origin).errors.error(e);
        }
        for w in warnings {
            let origin = match w.diagnostic().data() {
                DiagnosticData::Message => unreachable!(),
                DiagnosticData::Context { main, .. } => main.src().module_id(),
                DiagnosticData::ContextVec { main, .. } => main.src().module_id(),
            };
            self.module.get_mut(origin).errors.warning(w);
        }
    }
}

impl<'s> Modules<'s> {
    fn new(cap: usize) -> Modules<'s> {
        Modules { modules: Vec::with_capacity(cap) }
    }

    #[inline]
    pub fn ids(&self) -> impl Iterator<Item = ModuleID> {
        (0..(self.modules.len() as u32)).map(ModuleID)
    }
    #[inline]
    pub fn count(&self) -> usize {
        self.modules.len()
    }
    #[inline]
    pub fn get(&self, module_id: ModuleID) -> &Module<'s> {
        &self.modules[module_id.index()]
    }
    #[inline]
    pub fn get_mut(&mut self, module_id: ModuleID) -> &mut Module<'s> {
        &mut self.modules[module_id.index()]
    }

    #[must_use]
    fn add(&mut self, module: Module<'s>) -> ModuleID {
        let module_id = ModuleID(self.modules.len() as u32);
        self.modules.push(module);
        module_id
    }
}

impl<'s> Module<'s> {
    #[inline]
    pub fn tree_expect(&self) -> &SyntaxTree<'s> {
        self.tree.as_ref().unwrap()
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
}

pub enum ModuleOrDirectory<'s> {
    None,
    Module(ModuleID),
    Directory(&'s Directory),
}

impl Directory {
    pub fn find(&self, session: &Session, name_id: NameID) -> ModuleOrDirectory {
        for module_id in self.modules.iter().copied() {
            if session.module.get(module_id).name_id == name_id {
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

impl BuildStats {
    pub fn total_secs(&self) -> f64 {
        let total_ms = self.session_ms
            + self.parse_ms
            + self.check_ms
            + self.llvm_ir_ms
            + self.object_ms
            + self.link_ms;
        total_ms / 1000.0
    }
}

fn default_session<'s>(config: config::Config) -> Result<Session<'s>, Error> {
    Ok(Session {
        vfs: Vfs::new(64),
        curr_exe_dir: os::current_exe_path()?,
        curr_work_dir: os::dir_get_current_working()?,
        intern_lit: InternPool::new(512),
        intern_name: InternPool::new(1024),
        graph: graph::PackageGraph::new(8),
        module: Modules::new(64),
        stats: BuildStats::default(),
        config,
        errors: ErrorBuffer::default(),
        root_id: PackageID(0),
        ast_state: AstBuildState::new(),
    })
}

pub fn create_session<'s>(config: config::Config) -> Result<Session<'s>, Error> {
    let mut session = default_session(config)?;
    let core_dir = session.curr_exe_dir.join("core");
    process_package(&mut session, &core_dir, None, true, false)?;
    let root_dir = session.curr_work_dir.clone();
    if root_dir != core_dir {
        session.root_id = process_package(&mut session, &root_dir, None, false, false)?;
    }
    session.stats.package_count = session.graph.package_count() as u32;
    session.stats.module_count = session.module.modules.len() as u32;
    Ok(session)
}

pub fn format_session<'s>(config: config::Config) -> Result<Session<'s>, Error> {
    let mut session = default_session(config)?;
    let root_dir = session.curr_work_dir.clone();
    let _ = process_package(&mut session, &root_dir, None, false, true)?;
    Ok(session)
}

fn process_package(
    session: &mut Session,
    root_dir: &PathBuf,
    dep_from: Option<PackageID>,
    is_core: bool,
    format: bool,
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

    let package_id = session.graph.next_id();
    let src = process_directory(session, &src_dir, package_id)?;
    if format {
        return Ok(package_id);
    }

    let manifest = os::file_read(&manifest_path)?;
    let manifest = manifest::deserialize(&manifest, &manifest_path)?;
    let name_id = session.intern_name.intern(&manifest.package.name);

    if let Some(dep_id) = dep_from {
        let dep = session.graph.package(dep_id);
        let dep_path = dep.root_dir.to_str().unwrap_or("");
        let dep_name = session.intern_name.get(dep.name_id);
        let pkg_name = session.intern_name.get(name_id);

        if manifest.package.kind == PackageKind::Bin {
            return Err(err::session_dep_on_bin(dep_path, dep_name, pkg_name));
        }
    }

    // disallow core lib from having any dependencies
    assert!(!is_core || manifest.dependencies.is_empty());
    let deps = if is_core { vec![] } else { vec![CORE_PACKAGE_ID] };
    let package = Package { root_dir: root_dir.clone(), name_id, src, manifest, deps };
    let package_deps = package.manifest.dependencies.clone();
    let package_id = session.graph.add(package, root_dir);

    for (dep_name, dep) in package_deps {
        let dep_root_dir = match dep {
            Dependency::Dep(semver) => {
                return Err(Error::message(format!(
                    "package fetch not implemented, cannot find: {dep_name}-{semver}"
                )));
            }
            Dependency::Path { path } => os::canonicalize(&PathBuf::from(path.as_str()))?,
        };
        let dep_id = match session.graph.get_unique(&dep_root_dir) {
            Some(dep_id) => dep_id,
            None => process_package(session, &dep_root_dir, Some(package_id), false, false)?,
        };
        session.graph.add_dep(package_id, dep_id, &session.intern_name, &manifest_path)?;
    }
    Ok(package_id)
}

fn process_directory(
    session: &mut Session,
    path: &PathBuf,
    origin: PackageID,
) -> Result<Directory, Error> {
    let filename = os::filename(path)?;
    let name_id = session.intern_name.intern(filename);

    let mut modules = Vec::new();
    let mut sub_dirs = Vec::new();

    let read_dir = os::dir_read(path)?;
    for entry_result in read_dir {
        let entry = os::dir_entry_read(path, entry_result)?;
        let entry_path = entry.path();

        if let Ok(metadata) = std::fs::metadata(&entry_path) {
            if metadata.is_file() {
                if os::file_extension(&entry_path) == Some("rock") {
                    let module_id = process_module(session, &entry_path, origin)?;
                    modules.push(module_id);
                }
            } else if metadata.is_dir() {
                let sub_dir = process_directory(session, &entry_path, origin)?;
                sub_dirs.push(sub_dir);
            }
        }
    }

    Ok(Directory { name_id, modules, sub_dirs })
}

fn process_module(
    session: &mut Session,
    path: &PathBuf,
    origin: PackageID,
) -> Result<ModuleID, Error> {
    let filename = os::filename(path)?;
    let name_id = session.intern_name.intern(filename);

    let source = os::file_read_with_sentinel(path)?;
    let file_id = session.vfs.open(path, source);

    let module = Module {
        origin,
        name_id,
        file_id,
        tree: None,
        tree_version: 0,
        ast: None,
        ast_version: 0,
        parse_errors: ErrorBuffer::default(),
        errors: ErrorWarningBuffer::default(),
    };
    let module_id = session.module.add(module);
    Ok(module_id)
}
