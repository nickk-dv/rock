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

    pub fn visibility(&self) -> Visibility {
        match self {
            TypeData::Enum(data) => data.decl.visibility,
            TypeData::Struct(data) => data.decl.visibility,
        }
    }
}

pub struct SymbolTable {
    mods: HashMap<InternID, P<ModDecl>>,
    procs: HashMap<InternID, ProcData>,
    enums: HashMap<InternID, EnumData>,
    structs: HashMap<InternID, StructData>,
    globals: HashMap<InternID, GlobalData>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            mods: HashMap::new(),
            procs: HashMap::new(),
            enums: HashMap::new(),
            structs: HashMap::new(),
            globals: HashMap::new(),
        }
    }

    pub fn add_mod(&mut self, decl: P<ModDecl>) -> Option<P<ModDecl>> {
        self.mods.insert(decl.name.id, decl)
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

    pub fn get_mod(&self, id: InternID) -> Option<P<ModDecl>> {
        match self.mods.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }

    pub fn get_proc(&self, id: InternID) -> Option<ProcData> {
        match self.procs.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }

    pub fn get_type(&self, id: InternID) -> Option<TypeData> {
        if let Some(v) = self.structs.get(&id) {
            return Some(TypeData::Struct(*v));
        }
        if let Some(v) = self.enums.get(&id) {
            return Some(TypeData::Enum(*v));
        }
        None
    }

    pub fn get_global(&self, id: InternID) -> Option<GlobalData> {
        match self.globals.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
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
