use crate::arena::Arena;
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::session::{PackageID, Session};
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirData<'hir, 'ast, 'intern> {
    packages: Vec<Package>,
    modules: Vec<Module>,
    registry: Registry<'hir, 'ast>,
    ast: ast::Ast<'ast, 'intern>,
}

pub struct Package {
    module_map: HashMap<InternID, hir::ModuleID>,
    dependency_map: HashMap<InternID, PackageID>,
}

pub struct Module {
    package_id: PackageID,
    symbols: HashMap<InternID, Symbol>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Symbol {
    Defined  { kind: SymbolKind, },
    Imported { kind: SymbolKind, import_range: TextRange },
}

#[derive(Copy, Clone)]
pub enum SymbolKind {
    Module(hir::ModuleID),
    Proc(hir::ProcID),
    Enum(hir::EnumID),
    Union(hir::UnionID),
    Struct(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
}

pub struct Registry<'hir, 'ast> {
    ast_modules: Vec<ast::Module<'ast>>,
    ast_procs: Vec<&'ast ast::ProcItem<'ast>>,
    ast_enums: Vec<&'ast ast::EnumItem<'ast>>,
    ast_unions: Vec<&'ast ast::UnionItem<'ast>>,
    ast_structs: Vec<&'ast ast::StructItem<'ast>>,
    ast_consts: Vec<&'ast ast::ConstItem<'ast>>,
    ast_globals: Vec<&'ast ast::GlobalItem<'ast>>,
    hir_modules: Vec<hir::ModuleData>,
    hir_procs: Vec<hir::ProcData<'hir>>,
    hir_enums: Vec<hir::EnumData<'hir>>,
    hir_unions: Vec<hir::UnionData<'hir>>,
    hir_structs: Vec<hir::StructData<'hir>>,
    hir_consts: Vec<hir::ConstData<'hir>>,
    hir_globals: Vec<hir::GlobalData<'hir>>,
}

pub struct HirEmit<'hir> {
    pub arena: Arena<'hir>,
    errors: Vec<ErrorComp>,
}

impl<'hir, 'ast, 'intern> HirData<'hir, 'ast, 'intern> {
    pub fn new(
        mut ast: ast::Ast<'ast, 'intern>,
        session: &Session,
    ) -> HirData<'hir, 'ast, 'intern> {
        let mut packages = Vec::<Package>::new();
        let mut modules = Vec::<Module>::new();
        //@item count isnt implemented on ast level, so using default 0 initialized on @17.04.24
        let mut registry = Registry::new(ast::ItemCount::default());

        for ast_package in ast.packages.iter() {
            let package_id = PackageID::new(packages.len());
            let mut module_map: HashMap<InternID, hir::ModuleID> = HashMap::new();
            let mut dependency_map: HashMap<InternID, PackageID> = HashMap::new();

            for ast_module in ast_package.modules.iter() {
                modules.push(Module {
                    package_id,
                    symbols: HashMap::new(),
                });
                let module_id = registry.add_module(*ast_module);
                module_map.insert(ast_module.name_id, module_id);
            }

            let manifest = session.package(package_id).manifest();
            for dep in manifest.dependencies.keys() {
                //@identify dep packages by ID earlier? @20.04.24
                // have to seach for this package in ast by name id, to get the dep PackageID
                let dep_name_id = ast.intern.intern(dep.as_str());
                let dep_package_id = ast
                    .packages
                    .iter()
                    .enumerate()
                    .find_map(|(idx, pkg)| {
                        (pkg.name_id == dep_name_id).then(|| PackageID::new(idx))
                    })
                    .expect("package dependency name must be in ast packages");
                dependency_map.insert(dep_name_id, dep_package_id);
            }

            packages.push(Package {
                module_map,
                dependency_map,
            })
        }

        HirData {
            modules,
            packages,
            registry,
            ast,
        }
    }

    pub fn src(&self, id: hir::ModuleID, range: TextRange) -> SourceRange {
        SourceRange::new(range, self.registry().module_ast(id).file_id)
    }
    pub fn name_str(&self, id: InternID) -> &str {
        self.ast.intern.get_str(id)
    }

    pub fn get_package_module_id(
        &self,
        package_id: PackageID,
        name: ast::Name,
    ) -> Option<hir::ModuleID> {
        self.package(package_id).module_map.get(&name.id).copied()
    }
    pub fn get_package_dep_id(&self, package_id: PackageID, name: ast::Name) -> Option<PackageID> {
        self.package(package_id)
            .dependency_map
            .get(&name.id)
            .copied()
    }

    /// this inserts symbol into the map assuming that duplicate was already checked  
    /// this api can be error prone, but may the only option so far  @`17.04.24`
    pub fn add_symbol(&mut self, origin_id: hir::ModuleID, id: InternID, symbol: Symbol) {
        self.module_mut(origin_id).symbols.insert(id, symbol);
    }
    pub fn module_package_id(&self, id: hir::ModuleID) -> PackageID {
        self.module(id).package_id
    }

    pub fn registry(&self) -> &Registry<'hir, 'ast> {
        &self.registry
    }
    pub fn registry_mut(&mut self) -> &mut Registry<'hir, 'ast> {
        &mut self.registry
    }

    fn package(&self, id: PackageID) -> &Package {
        &self.packages[id.index()]
    }
    fn module(&self, id: hir::ModuleID) -> &Module {
        &self.modules[id.index()]
    }
    fn module_mut(&mut self, id: hir::ModuleID) -> &mut Module {
        &mut self.modules[id.index()]
    }

    pub fn scope_name_defined(
        &self,
        origin_id: hir::ModuleID,
        id: InternID,
    ) -> Option<SourceRange> {
        let origin = self.module(origin_id);
        if let Some(symbol) = origin.symbols.get(&id).cloned() {
            let file_id = self.registry().module_ast(origin_id).file_id;
            match symbol {
                Symbol::Defined { kind } => {
                    Some(SourceRange::new(self.symbol_kind_range(kind), file_id))
                }
                Symbol::Imported { import_range, .. } => {
                    Some(SourceRange::new(import_range, file_id))
                }
            }
        } else {
            None
        }
    }

    pub fn symbol_from_scope(
        &self,
        emit: &mut HirEmit<'hir>,
        origin_id: hir::ModuleID,
        target_id: hir::ModuleID,
        name: ast::Name,
    ) -> Option<(SymbolKind, SourceRange)> {
        let target = self.module(target_id);
        match target.symbols.get(&name.id).cloned() {
            Some(symbol) => match symbol {
                Symbol::Defined { kind } => {
                    let file_id = self.registry().module_ast(target_id).file_id;
                    let source = SourceRange::new(self.symbol_kind_range(kind), file_id);
                    let vis = if origin_id == target_id {
                        ast::Vis::Public
                    } else {
                        self.symbol_kind_vis(kind)
                    };
                    if vis == ast::Vis::Private {
                        emit.error(ErrorComp::error(
                            format!(
                                "{} `{}` is private",
                                Self::symbol_kind_name(kind),
                                self.name_str(name.id)
                            ),
                            self.src(origin_id, name.range),
                            ErrorComp::info("defined here", source),
                        ));
                        None
                    } else {
                        Some((kind, source))
                    }
                }
                Symbol::Imported { kind, import_range } => {
                    if origin_id == target_id {
                        let file_id = self.registry().module_ast(target_id).file_id;
                        let source = SourceRange::new(import_range, file_id);
                        Some((kind, source))
                    } else {
                        None
                    }
                }
            },
            None => {
                //@sometimes its in self scope
                // else its in some module.
                // display module path?
                emit.error(ErrorComp::error(
                    format!("name `{}` is not found in module", self.name_str(name.id)),
                    self.src(origin_id, name.range),
                    None,
                ));
                None
            }
        }
    }

    pub fn symbol_kind_name(kind: SymbolKind) -> &'static str {
        match kind {
            SymbolKind::Module(_) => "module",
            SymbolKind::Proc(_) => "procedure",
            SymbolKind::Enum(_) => "enum",
            SymbolKind::Union(_) => "union",
            SymbolKind::Struct(_) => "struct",
            SymbolKind::Const(_) => "constant",
            SymbolKind::Global(_) => "global",
        }
    }

    fn symbol_kind_range(&self, kind: SymbolKind) -> TextRange {
        match kind {
            SymbolKind::Module(..) => unreachable!(),
            SymbolKind::Proc(id) => self.registry.proc_data(id).name.range,
            SymbolKind::Enum(id) => self.registry.enum_data(id).name.range,
            SymbolKind::Union(id) => self.registry.union_data(id).name.range,
            SymbolKind::Struct(id) => self.registry.struct_data(id).name.range,
            SymbolKind::Const(id) => self.registry.const_data(id).name.range,
            SymbolKind::Global(id) => self.registry.global_data(id).name.range,
        }
    }

    fn symbol_kind_vis(&self, kind: SymbolKind) -> ast::Vis {
        match kind {
            SymbolKind::Module(..) => unreachable!(),
            SymbolKind::Proc(id) => self.registry.proc_data(id).vis,
            SymbolKind::Enum(id) => self.registry.enum_data(id).vis,
            SymbolKind::Union(id) => self.registry.union_data(id).vis,
            SymbolKind::Struct(id) => self.registry.struct_data(id).vis,
            SymbolKind::Const(id) => self.registry.const_data(id).vis,
            SymbolKind::Global(id) => self.registry.global_data(id).vis,
        }
    }
}

impl<'hir, 'ast> Registry<'hir, 'ast> {
    pub fn new(total: ast::ItemCount) -> Registry<'hir, 'ast> {
        Registry {
            ast_modules: Vec::with_capacity(total.modules as usize),
            ast_procs: Vec::with_capacity(total.procs as usize),
            ast_enums: Vec::with_capacity(total.enums as usize),
            ast_unions: Vec::with_capacity(total.unions as usize),
            ast_structs: Vec::with_capacity(total.structs as usize),
            ast_consts: Vec::with_capacity(total.consts as usize),
            ast_globals: Vec::with_capacity(total.globals as usize),
            hir_modules: Vec::with_capacity(total.modules as usize),
            hir_procs: Vec::with_capacity(total.procs as usize),
            hir_enums: Vec::with_capacity(total.enums as usize),
            hir_unions: Vec::with_capacity(total.unions as usize),
            hir_structs: Vec::with_capacity(total.structs as usize),
            hir_consts: Vec::with_capacity(total.consts as usize),
            hir_globals: Vec::with_capacity(total.globals as usize),
        }
    }

    pub fn add_module(&mut self, module: ast::Module<'ast>) -> hir::ModuleID {
        let id = hir::ModuleID::new(self.hir_modules.len());
        let data = hir::ModuleData {
            file_id: module.file_id,
        };
        self.ast_modules.push(module);
        self.hir_modules.push(data);
        id
    }
    pub fn add_proc(
        &mut self,
        item: &'ast ast::ProcItem<'ast>,
        origin_id: hir::ModuleID,
        is_test: bool,
        is_main: bool,
    ) -> hir::ProcID {
        let id = hir::ProcID::new(self.hir_procs.len());
        let data = hir::ProcData {
            origin_id,
            vis: item.vis,
            name: item.name,
            params: &[],
            is_variadic: item.is_variadic,
            return_ty: hir::Type::Error,
            block: None,
            locals: &[],
            is_test,
            is_main,
        };
        self.ast_procs.push(item);
        self.hir_procs.push(data);
        id
    }
    pub fn add_enum(
        &mut self,
        item: &'ast ast::EnumItem<'ast>,
        origin_id: hir::ModuleID,
    ) -> hir::EnumID {
        let id = hir::EnumID::new(self.hir_enums.len());
        let data = hir::EnumData {
            origin_id,
            vis: item.vis,
            name: item.name,
            basic: item.basic.unwrap_or(ast::BasicType::S32),
            variants: &[],
        };
        self.ast_enums.push(item);
        self.hir_enums.push(data);
        id
    }
    pub fn add_union(
        &mut self,
        item: &'ast ast::UnionItem<'ast>,
        origin_id: hir::ModuleID,
    ) -> hir::UnionID {
        let id = hir::UnionID::new(self.hir_unions.len());
        let data = hir::UnionData {
            origin_id,
            vis: item.vis,
            name: item.name,
            members: &[],
            size_eval: hir::SizeEval::Unresolved,
        };
        self.ast_unions.push(item);
        self.hir_unions.push(data);
        id
    }
    pub fn add_struct(
        &mut self,
        item: &'ast ast::StructItem<'ast>,
        origin_id: hir::ModuleID,
    ) -> hir::StructID {
        let id = hir::StructID::new(self.hir_structs.len());
        let data = hir::StructData {
            origin_id,
            vis: item.vis,
            name: item.name,
            fields: &[],
            size_eval: hir::SizeEval::Unresolved,
        };
        self.ast_structs.push(item);
        self.hir_structs.push(data);
        id
    }
    pub fn add_const(
        &mut self,
        item: &'ast ast::ConstItem<'ast>,
        origin_id: hir::ModuleID,
    ) -> hir::ConstID {
        let id = hir::ConstID::new(self.hir_consts.len());
        let data = hir::ConstData {
            origin_id,
            vis: item.vis,
            name: item.name,
            ty: hir::Type::Error,
            value: hir::ConstValueEval::Unresolved,
        };
        self.ast_consts.push(item);
        self.hir_consts.push(data);
        id
    }
    pub fn add_global(
        &mut self,
        item: &'ast ast::GlobalItem<'ast>,
        origin_id: hir::ModuleID,
        thread_local: bool,
    ) -> hir::GlobalID {
        let id = hir::GlobalID::new(self.hir_globals.len());
        let data = hir::GlobalData {
            origin_id,
            vis: item.vis,
            mutt: item.mutt,
            name: item.name,
            ty: hir::Type::Error,
            value: hir::ConstValueEval::Unresolved,
            thread_local,
        };
        self.ast_globals.push(item);
        self.hir_globals.push(data);
        id
    }

    pub fn module_ids(&self) -> impl Iterator<Item = hir::ModuleID> {
        (0..self.hir_modules.len()).map(hir::ModuleID::new)
    }
    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID> {
        (0..self.hir_procs.len()).map(hir::ProcID::new)
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID> {
        (0..self.hir_enums.len()).map(hir::EnumID::new)
    }
    pub fn union_ids(&self) -> impl Iterator<Item = hir::UnionID> {
        (0..self.hir_unions.len()).map(hir::UnionID::new)
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID> {
        (0..self.hir_structs.len()).map(hir::StructID::new)
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID> {
        (0..self.hir_consts.len()).map(hir::ConstID::new)
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID> {
        (0..self.hir_globals.len()).map(hir::GlobalID::new)
    }

    pub fn module_ast(&self, id: hir::ModuleID) -> &ast::Module<'ast> {
        &self.ast_modules[id.index()]
    }
    pub fn proc_item(&self, id: hir::ProcID) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.index()]
    }
    pub fn enum_item(&self, id: hir::EnumID) -> &'ast ast::EnumItem<'ast> {
        self.ast_enums[id.index()]
    }
    pub fn union_item(&self, id: hir::UnionID) -> &'ast ast::UnionItem<'ast> {
        self.ast_unions[id.index()]
    }
    pub fn struct_item(&self, id: hir::StructID) -> &'ast ast::StructItem<'ast> {
        self.ast_structs[id.index()]
    }
    pub fn const_item(&self, id: hir::ConstID) -> &'ast ast::ConstItem<'ast> {
        self.ast_consts[id.index()]
    }
    pub fn global_item(&self, id: hir::GlobalID) -> &'ast ast::GlobalItem<'ast> {
        self.ast_globals[id.index()]
    }

    pub fn module_data(&self, id: hir::ModuleID) -> &hir::ModuleData {
        &self.hir_modules[id.index()]
    }
    pub fn proc_data(&self, id: hir::ProcID) -> &hir::ProcData<'hir> {
        &self.hir_procs[id.index()]
    }
    pub fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        &self.hir_enums[id.index()]
    }
    pub fn union_data(&self, id: hir::UnionID) -> &hir::UnionData<'hir> {
        &self.hir_unions[id.index()]
    }
    pub fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        &self.hir_structs[id.index()]
    }
    pub fn const_data(&self, id: hir::ConstID) -> &hir::ConstData<'hir> {
        &self.hir_consts[id.index()]
    }
    pub fn global_data(&self, id: hir::GlobalID) -> &hir::GlobalData<'hir> {
        &self.hir_globals[id.index()]
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID) -> &mut hir::ProcData<'hir> {
        &mut self.hir_procs[id.index()]
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID) -> &mut hir::EnumData<'hir> {
        &mut self.hir_enums[id.index()]
    }
    pub fn union_data_mut(&mut self, id: hir::UnionID) -> &mut hir::UnionData<'hir> {
        &mut self.hir_unions[id.index()]
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID) -> &mut hir::StructData<'hir> {
        &mut self.hir_structs[id.index()]
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID) -> &mut hir::ConstData<'hir> {
        &mut self.hir_consts[id.index()]
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID) -> &mut hir::GlobalData<'hir> {
        &mut self.hir_globals[id.index()]
    }
}

impl<'hir> HirEmit<'hir> {
    pub fn new() -> HirEmit<'hir> {
        HirEmit {
            arena: Arena::new(),
            errors: Vec::new(),
        }
    }

    pub fn error(&mut self, error: ErrorComp) {
        self.errors.push(error);
    }

    pub fn emit<'ast, 'intern: 'hir>(
        self,
        hir: HirData<'hir, 'ast, 'intern>,
    ) -> Result<hir::Hir<'hir>, Vec<ErrorComp>> {
        //@debug info
        eprintln!("ast mem: {}", hir.ast.arena.mem_usage());
        eprintln!("hir mem: {}", self.arena.mem_usage());
        if self.errors.is_empty() {
            Ok(hir::Hir {
                arena: self.arena,
                intern: hir.ast.intern,
                modules: hir.registry.hir_modules,
                procs: hir.registry.hir_procs,
                enums: hir.registry.hir_enums,
                unions: hir.registry.hir_unions,
                structs: hir.registry.hir_structs,
                consts: hir.registry.hir_consts,
                globals: hir.registry.hir_globals,
            })
        } else {
            Err(self.errors)
        }
    }
}
