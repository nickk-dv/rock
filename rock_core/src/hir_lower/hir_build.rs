use crate::arena::Arena;
use crate::ast;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::TextRange;
use std::collections::HashMap;

#[derive(Default)]
pub struct HirData<'hir, 'ast, 'intern> {
    ast: ast::Ast<'ast, 'intern>,
    scope_map: HashMap<InternID, hir::ScopeID>,
    scopes: Vec<Scope<'ast>>,
    ast_procs: Vec<&'ast ast::ProcItem<'ast>>,
    ast_enums: Vec<&'ast ast::EnumItem<'ast>>,
    ast_unions: Vec<&'ast ast::UnionItem<'ast>>,
    ast_structs: Vec<&'ast ast::StructItem<'ast>>,
    ast_consts: Vec<&'ast ast::ConstItem<'ast>>,
    ast_globals: Vec<&'ast ast::GlobalItem<'ast>>,
    hir_scopes: Vec<hir::ScopeData>,
    procs: Vec<hir::ProcData<'hir>>,
    enums: Vec<hir::EnumData<'hir>>,
    unions: Vec<hir::UnionData<'hir>>,
    structs: Vec<hir::StructData<'hir>>,
    consts: Vec<hir::ConstData<'hir>>,
    globals: Vec<hir::GlobalData<'hir>>,
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

#[derive(Default)]
pub struct HirEmit<'hir> {
    pub arena: Arena<'hir>,
    errors: Vec<ErrorComp>,
}

impl<'hir, 'ast, 'intern> HirData<'hir, 'ast, 'intern> {
    pub fn new(ast: ast::Ast<'ast, 'intern>) -> HirData<'hir, 'ast, 'intern> {
        HirData {
            ast,
            ..Default::default()
        }
    }

    pub fn src(&self, id: hir::ScopeID, range: TextRange) -> SourceRange {
        SourceRange::new(range, self.scope(id).module.file_id)
    }
    pub fn name_str(&self, id: InternID) -> &str {
        self.ast.intern.get_str(id)
    }

    pub fn add_ast_modules(&mut self) {
        for module in self.ast.modules.iter() {
            let id = hir::ScopeID::new(self.scopes.len());
            let scope = Scope {
                module: *module,
                symbols: HashMap::new(),
            };
            self.scopes.push(scope);
            self.hir_scopes.push(hir::ScopeData {
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

    pub fn get_module_id(&self, id: InternID) -> Option<hir::ScopeID> {
        self.scope_map.get(&id).cloned()
    }

    pub fn scope_add_imported(
        &mut self,
        origin_id: hir::ScopeID,
        import_name: ast::Name,
        kind: SymbolKind,
    ) {
        self.scope_add_symbol(
            origin_id,
            import_name.id,
            Symbol::Imported {
                kind,
                import_range: import_name.range,
            },
        );
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

    fn scope_add_symbol(&mut self, origin_id: hir::ScopeID, id: InternID, symbol: Symbol) {
        let scope = &mut self.scopes[origin_id.index()];
        scope.symbols.insert(id, symbol);
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
            SymbolKind::Proc(id) => self.proc_data(id).name.range,
            SymbolKind::Enum(id) => self.enum_data(id).name.range,
            SymbolKind::Union(id) => self.union_data(id).name.range,
            SymbolKind::Struct(id) => self.struct_data(id).name.range,
            SymbolKind::Const(id) => self.const_data(id).name.range,
            SymbolKind::Global(id) => self.global_data(id).name.range,
            SymbolKind::Module(..) => unreachable!(),
        }
    }

    fn symbol_kind_vis(&self, kind: SymbolKind) -> ast::Vis {
        match kind {
            SymbolKind::Proc(id) => self.proc_data(id).vis,
            SymbolKind::Enum(id) => self.enum_data(id).vis,
            SymbolKind::Union(id) => self.union_data(id).vis,
            SymbolKind::Struct(id) => self.struct_data(id).vis,
            SymbolKind::Const(id) => self.const_data(id).vis,
            SymbolKind::Global(id) => self.global_data(id).vis,
            SymbolKind::Module(..) => unreachable!(),
        }
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID> {
        (0..self.procs.len()).map(hir::ProcID::new)
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID> {
        (0..self.enums.len()).map(hir::EnumID::new)
    }
    pub fn union_ids(&self) -> impl Iterator<Item = hir::UnionID> {
        (0..self.unions.len()).map(hir::UnionID::new)
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID> {
        (0..self.structs.len()).map(hir::StructID::new)
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID> {
        (0..self.consts.len()).map(hir::ConstID::new)
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID> {
        (0..self.globals.len()).map(hir::GlobalID::new)
    }

    //@try removing reference lifetimes?
    pub fn proc_ast(&self, id: hir::ProcID) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.index()]
    }
    pub fn enum_ast(&self, id: hir::EnumID) -> &'ast ast::EnumItem<'ast> {
        self.ast_enums[id.index()]
    }
    pub fn union_ast(&self, id: hir::UnionID) -> &'ast ast::UnionItem<'ast> {
        self.ast_unions[id.index()]
    }
    pub fn struct_ast(&self, id: hir::StructID) -> &'ast ast::StructItem<'ast> {
        self.ast_structs[id.index()]
    }
    pub fn const_ast(&self, id: hir::ConstID) -> &'ast ast::ConstItem<'ast> {
        self.ast_consts[id.index()]
    }
    pub fn global_ast(&self, id: hir::GlobalID) -> &'ast ast::GlobalItem<'ast> {
        self.ast_globals[id.index()]
    }

    pub fn proc_data(&self, id: hir::ProcID) -> &hir::ProcData<'hir> {
        &self.procs[id.index()]
    }
    pub fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        &self.enums[id.index()]
    }
    pub fn union_data(&self, id: hir::UnionID) -> &hir::UnionData<'hir> {
        &self.unions[id.index()]
    }
    pub fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        &self.structs[id.index()]
    }
    pub fn const_data(&self, id: hir::ConstID) -> &hir::ConstData<'hir> {
        &self.consts[id.index()]
    }
    pub fn global_data(&self, id: hir::GlobalID) -> &hir::GlobalData<'hir> {
        &self.globals[id.index()]
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID) -> &mut hir::ProcData<'hir> {
        self.procs.get_mut(id.index()).unwrap()
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID) -> &mut hir::EnumData<'hir> {
        self.enums.get_mut(id.index()).unwrap()
    }
    pub fn union_data_mut(&mut self, id: hir::UnionID) -> &mut hir::UnionData<'hir> {
        self.unions.get_mut(id.index()).unwrap()
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID) -> &mut hir::StructData<'hir> {
        self.structs.get_mut(id.index()).unwrap()
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID) -> &mut hir::ConstData<'hir> {
        self.consts.get_mut(id.index()).unwrap()
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID) -> &mut hir::GlobalData<'hir> {
        self.globals.get_mut(id.index()).unwrap()
    }

    pub fn add_proc(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::ProcItem<'ast>,
        data: hir::ProcData<'hir>,
    ) {
        let id = hir::ProcID::new(self.ast_procs.len());
        self.ast_procs.push(item);
        self.procs.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Proc(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_enum(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::EnumItem<'ast>,
        data: hir::EnumData<'hir>,
    ) {
        let id = hir::EnumID::new(self.ast_enums.len());
        self.ast_enums.push(item);
        self.enums.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Enum(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_union(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::UnionItem<'ast>,
        data: hir::UnionData<'hir>,
    ) {
        let id = hir::UnionID::new(self.ast_unions.len());
        self.ast_unions.push(item);
        self.unions.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Union(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_struct(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::StructItem<'ast>,
        data: hir::StructData<'hir>,
    ) {
        let id = hir::StructID::new(self.ast_structs.len());
        self.ast_structs.push(item);
        self.structs.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Struct(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_const(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::ConstItem<'ast>,
        data: hir::ConstData<'hir>,
    ) {
        let id = hir::ConstID::new(self.ast_consts.len());
        self.ast_consts.push(item);
        self.consts.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Const(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_global(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::GlobalItem<'ast>,
        data: hir::GlobalData<'hir>,
    ) {
        let id = hir::GlobalID::new(self.ast_globals.len());
        self.ast_globals.push(item);
        self.globals.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Global(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
}

impl<'hir> HirEmit<'hir> {
    pub fn new() -> HirEmit<'hir> {
        HirEmit::default()
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
                scopes: hir.hir_scopes,
                procs: hir.procs,
                enums: hir.enums,
                unions: hir.unions,
                structs: hir.structs,
                consts: hir.consts,
                globals: hir.globals,
            })
        } else {
            Err(self.errors)
        }
    }
}
