use crate::arena::Arena;
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirData<'hir, 'ast, 'intern> {
    ast: ast::Ast<'ast, 'intern>,
    scope_map: HashMap<InternID, hir::ScopeID>,
    scopes: Vec<Scope<'ast>>,
    registry: Registry<'hir, 'ast>,
}

pub struct Registry<'hir, 'ast> {
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

pub struct Scope<'ast> {
    module: ast::Module<'ast>,
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
    Proc(hir::ProcID),
    Enum(hir::EnumID),
    Union(hir::UnionID),
    Struct(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
    Module(hir::ScopeID),
}

pub struct HirEmit<'hir> {
    pub arena: Arena<'hir>,
    errors: Vec<ErrorComp>,
}

impl<'hir, 'ast, 'intern> HirData<'hir, 'ast, 'intern> {
    pub fn new(ast: ast::Ast<'ast, 'intern>) -> HirData<'hir, 'ast, 'intern> {
        HirData {
            ast,
            scope_map: HashMap::new(),
            scopes: Vec::new(),
            //@item count isnt implemented on ast level, so using default 0 initialized on @17.04.24
            registry: Registry::new(ast::ItemCount::default()),
        }
    }

    pub fn src(&self, id: hir::ScopeID, range: TextRange) -> SourceRange {
        SourceRange::new(range, self.scope(id).module.file_id)
    }
    pub fn name_str(&self, id: InternID) -> &str {
        self.ast.intern.get_str(id)
    }

    pub fn registry(&self) -> &Registry<'hir, 'ast> {
        &self.registry
    }
    pub fn registry_mut(&mut self) -> &mut Registry<'hir, 'ast> {
        &mut self.registry
    }

    /// this inserts symbol into the map assuming that duplicate was already checked  
    /// this api can be error prone, but may the only option so far  @`17.04.24`
    pub fn add_symbol(&mut self, origin_id: hir::ScopeID, id: InternID, symbol: Symbol) {
        self.scope_mut(origin_id).symbols.insert(id, symbol);
    }

    pub fn add_ast_modules(&mut self) {
        //@getting package 0 temporarely, no hir package support is done yet @19.04.24
        for module in self.ast.packages[0].modules.iter() {
            let id = hir::ScopeID::new(self.scopes.len());
            let scope = Scope {
                module: *module,
                symbols: HashMap::new(),
            };
            self.scopes.push(scope);
            self.registry.add_module(hir::ModuleData {
                file_id: module.file_id,
            });
            self.scope_map.insert(module.name_id, id);
        }
    }

    pub fn scope_ids(&self) -> impl Iterator<Item = hir::ScopeID> {
        (0..self.scopes.len()).map(hir::ScopeID::new)
    }
    fn scope(&self, id: hir::ScopeID) -> &Scope<'ast> {
        &self.scopes[id.index()]
    }
    fn scope_mut(&mut self, id: hir::ScopeID) -> &mut Scope<'ast> {
        &mut self.scopes[id.index()]
    }

    pub fn get_module_id(&self, id: InternID) -> Option<hir::ScopeID> {
        self.scope_map.get(&id).cloned()
    }

    pub fn scope_name_defined(&self, origin_id: hir::ScopeID, id: InternID) -> Option<SourceRange> {
        let origin = self.scope(origin_id);
        if let Some(symbol) = origin.symbols.get(&id).cloned() {
            let file_id = origin.module.file_id;
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

    pub fn scope_ast_items(&self, id: hir::ScopeID) -> impl Iterator<Item = ast::Item<'ast>> {
        self.scope(id).module.items.iter().cloned()
    }

    pub fn symbol_from_scope(
        &self,
        emit: &mut HirEmit<'hir>,
        origin_id: hir::ScopeID,
        target_id: hir::ScopeID,
        name: ast::Name,
    ) -> Option<(SymbolKind, SourceRange)> {
        let target = self.scope(target_id);
        match target.symbols.get(&name.id).cloned() {
            Some(symbol) => match symbol {
                Symbol::Defined { kind } => {
                    let source =
                        SourceRange::new(self.symbol_kind_range(kind), target.module.file_id);
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
                        let source = SourceRange::new(import_range, target.module.file_id);
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
            SymbolKind::Proc(_) => "procedure",
            SymbolKind::Enum(_) => "enum",
            SymbolKind::Union(_) => "union",
            SymbolKind::Struct(_) => "struct",
            SymbolKind::Const(_) => "constant",
            SymbolKind::Global(_) => "global",
            SymbolKind::Module(_) => "module",
        }
    }

    fn symbol_kind_range(&self, kind: SymbolKind) -> TextRange {
        match kind {
            SymbolKind::Proc(id) => self.registry.proc_data(id).name.range,
            SymbolKind::Enum(id) => self.registry.enum_data(id).name.range,
            SymbolKind::Union(id) => self.registry.union_data(id).name.range,
            SymbolKind::Struct(id) => self.registry.struct_data(id).name.range,
            SymbolKind::Const(id) => self.registry.const_data(id).name.range,
            SymbolKind::Global(id) => self.registry.global_data(id).name.range,
            SymbolKind::Module(..) => unreachable!(),
        }
    }

    fn symbol_kind_vis(&self, kind: SymbolKind) -> ast::Vis {
        match kind {
            SymbolKind::Proc(id) => self.registry.proc_data(id).vis,
            SymbolKind::Enum(id) => self.registry.enum_data(id).vis,
            SymbolKind::Union(id) => self.registry.union_data(id).vis,
            SymbolKind::Struct(id) => self.registry.struct_data(id).vis,
            SymbolKind::Const(id) => self.registry.const_data(id).vis,
            SymbolKind::Global(id) => self.registry.global_data(id).vis,
            SymbolKind::Module(..) => unreachable!(),
        }
    }
}

impl<'hir, 'ast> Registry<'hir, 'ast> {
    pub fn new(total: ast::ItemCount) -> Registry<'hir, 'ast> {
        Registry {
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

    pub fn add_module(&mut self, data: hir::ModuleData) {
        self.hir_modules.push(data);
    }
    pub fn add_proc(
        &mut self,
        item: &'ast ast::ProcItem<'ast>,
        origin_id: hir::ScopeID,
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
        };
        self.ast_procs.push(item);
        self.hir_procs.push(data);
        id
    }
    pub fn add_enum(
        &mut self,
        item: &'ast ast::EnumItem<'ast>,
        origin_id: hir::ScopeID,
    ) -> hir::EnumID {
        let id = hir::EnumID::new(self.hir_enums.len());
        let data = hir::EnumData {
            origin_id,
            vis: item.vis,
            name: item.name,
            basic: item.basic,
            variants: &[],
        };
        self.ast_enums.push(item);
        self.hir_enums.push(data);
        id
    }
    pub fn add_union(
        &mut self,
        item: &'ast ast::UnionItem<'ast>,
        origin_id: hir::ScopeID,
    ) -> hir::UnionID {
        let id = hir::UnionID::new(self.hir_unions.len());
        let data = hir::UnionData {
            origin_id,
            vis: item.vis,
            name: item.name,
            members: &[],
        };
        self.ast_unions.push(item);
        self.hir_unions.push(data);
        id
    }
    pub fn add_struct(
        &mut self,
        item: &'ast ast::StructItem<'ast>,
        origin_id: hir::ScopeID,
    ) -> hir::StructID {
        let id = hir::StructID::new(self.hir_structs.len());
        let data = hir::StructData {
            origin_id,
            vis: item.vis,
            name: item.name,
            fields: &[],
        };
        self.ast_structs.push(item);
        self.hir_structs.push(data);
        id
    }
    pub fn add_const(
        &mut self,
        item: &'ast ast::ConstItem<'ast>,
        data: hir::ConstData<'hir>,
    ) -> hir::ConstID {
        let id = hir::ConstID::new(self.hir_consts.len());
        self.ast_consts.push(item);
        self.hir_consts.push(data);
        id
    }
    pub fn add_global(
        &mut self,
        item: &'ast ast::GlobalItem<'ast>,
        data: hir::GlobalData<'hir>,
    ) -> hir::GlobalID {
        let id = hir::GlobalID::new(self.hir_globals.len());
        self.ast_globals.push(item);
        self.hir_globals.push(data);
        id
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
