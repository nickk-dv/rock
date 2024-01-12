use super::hir;
use crate::ast::ast::*;
use crate::mem::{InternID, P};
use std::collections::hash_map;
use std::collections::HashMap;

#[derive(Copy, Clone)]
pub struct ModData {
    pub decl: P<ModDecl>,
}

#[derive(Copy, Clone)]
pub struct ProcData {
    pub decl: P<ProcDecl>,
    pub id: hir::ProcID,
}

#[derive(Copy, Clone)]
pub struct EnumData {
    pub decl: P<EnumDecl>,
}

#[derive(Copy, Clone)]
pub struct StructData {
    pub decl: P<StructDecl>,
    pub id: hir::StructID,
}

#[derive(Copy, Clone)]
pub struct GlobalData {
    pub decl: P<GlobalDecl>,
}

#[derive(Copy, Clone)]
pub enum TypeData {
    Enum(EnumData),
    Struct(StructData),
}

impl TypeData {
    pub fn name(&self) -> Ident {
        match self {
            TypeData::Enum(data) => data.decl.name,
            TypeData::Struct(data) => data.decl.name,
        }
    }
}

pub struct SymbolTable2 {
    mods: HashMap<InternID, ModData>,
    procs: HashMap<InternID, ProcData>,
    enums: HashMap<InternID, EnumData>,
    structs: HashMap<InternID, StructData>,
    globals: HashMap<InternID, GlobalData>,
}

impl SymbolTable2 {
    pub fn new() -> Self {
        Self {
            mods: HashMap::new(),
            procs: HashMap::new(),
            enums: HashMap::new(),
            structs: HashMap::new(),
            globals: HashMap::new(),
        }
    }

    pub fn add_mod(&mut self, decl: P<ModDecl>) -> Option<ModData> {
        self.mods.insert(decl.name.id, ModData { decl })
    }

    pub fn add_proc(&mut self, decl: P<ProcDecl>, id: hir::ProcID) -> Option<ProcData> {
        self.procs.insert(decl.name.id, ProcData { decl, id })
    }

    pub fn add_enum(&mut self, decl: P<EnumDecl>) -> Option<TypeData> {
        if let Some(existing) = self.structs.get(&decl.name.id) {
            return Some(TypeData::Struct(*existing));
        }
        if let Some(existing) = self.enums.insert(decl.name.id, EnumData { decl }) {
            return Some(TypeData::Enum(existing));
        }
        None
    }

    pub fn add_struct(&mut self, decl: P<StructDecl>, id: hir::StructID) -> Option<TypeData> {
        if let Some(existing) = self.enums.get(&decl.name.id) {
            return Some(TypeData::Enum(*existing));
        }
        if let Some(existing) = self.structs.insert(decl.name.id, StructData { decl, id }) {
            return Some(TypeData::Struct(existing));
        }
        None
    }

    pub fn add_global(&mut self, decl: P<GlobalDecl>) -> Option<GlobalData> {
        self.globals.insert(decl.name.id, GlobalData { decl })
    }

    pub fn proc_values(&self) -> hash_map::Values<'_, InternID, ProcData> {
        self.procs.values()
    }

    pub fn enum_values(&self) -> hash_map::Values<'_, InternID, EnumData> {
        self.enums.values()
    }

    pub fn struct_values(&self) -> hash_map::Values<'_, InternID, StructData> {
        self.structs.values()
    }
}

pub struct SymbolTable {
    table: HashMap<InternID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Mod((P<ModDecl>, ScopeID)),
    Proc((P<ProcDecl>, ScopeID)),
    Enum((P<EnumDecl>, ScopeID)),
    Struct((P<StructDecl>, ScopeID)),
    Global((P<GlobalDecl>, ScopeID)),
}

#[derive(Copy, Clone)]
pub enum TypeSymbol {
    Enum((P<EnumDecl>, ScopeID)),
    Struct((P<StructDecl>, ScopeID)),
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    pub fn add(&mut self, symbol: Symbol) -> Result<(), Symbol> {
        let id = symbol.name().id;
        match self.table.get(&id) {
            Some(v) => Err(*v),
            None => {
                self.table.insert(id, symbol);
                Ok(())
            }
        }
    }

    pub fn get(&self, id: InternID) -> Option<Symbol> {
        match self.table.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }

    pub fn get_mod(&self, id: InternID) -> Option<(P<ModDecl>, ScopeID)> {
        match self.table.get(&id) {
            Some(Symbol::Mod(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_proc(&self, id: InternID) -> Option<(P<ProcDecl>, ScopeID)> {
        match self.table.get(&id) {
            Some(Symbol::Proc(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_type(&self, id: InternID) -> Option<TypeSymbol> {
        match self.table.get(&id) {
            Some(Symbol::Enum(v)) => Some(TypeSymbol::Enum(*v)),
            Some(Symbol::Struct(v)) => Some(TypeSymbol::Struct(*v)),
            _ => None,
        }
    }

    pub fn get_global(&self, id: InternID) -> Option<(P<GlobalDecl>, ScopeID)> {
        match self.table.get(&id) {
            Some(Symbol::Global(v)) => Some(*v),
            _ => None,
        }
    }
}

impl Symbol {
    pub fn from_decl(decl: Decl, source_id: ScopeID) -> Option<Self> {
        match decl {
            Decl::Mod(mod_decl) => Some(Symbol::Mod((mod_decl, source_id))),
            Decl::Proc(proc_decl) => Some(Symbol::Proc((proc_decl, source_id))),
            Decl::Enum(enum_decl) => Some(Symbol::Enum((enum_decl, source_id))),
            Decl::Struct(struct_decl) => Some(Symbol::Struct((struct_decl, source_id))),
            Decl::Global(global_decl) => Some(Symbol::Global((global_decl, source_id))),
            Decl::Import(..) => None,
        }
    }

    pub fn name(&self) -> Ident {
        match self {
            Symbol::Mod(mod_decl) => mod_decl.0.name,
            Symbol::Proc(proc_decl) => proc_decl.0.name,
            Symbol::Enum(enum_decl) => enum_decl.0.name,
            Symbol::Struct(struct_decl) => struct_decl.0.name,
            Symbol::Global(global_decl) => global_decl.0.name,
        }
    }

    pub fn visibility(&self) -> Visibility {
        match self {
            Symbol::Mod(mod_decl) => mod_decl.0.visibility,
            Symbol::Proc(proc_decl) => proc_decl.0.visibility,
            Symbol::Enum(enum_decl) => enum_decl.0.visibility,
            Symbol::Struct(struct_decl) => struct_decl.0.visibility,
            Symbol::Global(global_decl) => global_decl.0.visibility,
        }
    }
}

impl TypeSymbol {
    pub fn name(&self) -> Ident {
        match self {
            TypeSymbol::Enum(enum_decl) => enum_decl.0.name,
            TypeSymbol::Struct(struct_decl) => struct_decl.0.name,
        }
    }

    pub fn visibility(&self) -> Visibility {
        match self {
            TypeSymbol::Enum(enum_decl) => enum_decl.0.visibility,
            TypeSymbol::Struct(struct_decl) => struct_decl.0.visibility,
        }
    }
}
