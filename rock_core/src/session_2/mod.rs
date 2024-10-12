mod vfs;

use crate::ast::Ast;
use crate::intern::{InternLit, InternName, InternPool};
use crate::package::manifest::Manifest;
use crate::support::ID;
use crate::syntax::syntax_tree::SyntaxTree;
use std::collections::HashMap;
use std::path::PathBuf;
use vfs::{FileID, Vfs};

pub struct Session<'s> {
    pub vfs: Vfs,
    pub cwd: PathBuf,
    pub intern_lit: InternPool<'s, InternLit>,
    pub intern_name: InternPool<'s, InternName>,
    pub graph: PackageGraph,
    modules: Vec<Module<'s>>,
}

#[derive(Default)]
pub struct PackageGraph {
    packages: HashMap<PackageID, Package>,
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
}

impl PackageGraph {
    #[inline]
    pub fn package(&self, package_id: PackageID) -> &Package {
        self.packages.get(&package_id).unwrap()
    }
    #[inline]
    pub fn package_mut(&mut self, package_id: PackageID) -> &mut Package {
        self.packages.get_mut(&package_id).unwrap()
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
