use crate::ast::ast::*;
use crate::mem::{InternID, P};
use std::collections::HashMap;

pub struct SymbolTable {
    mods: HashMap<InternID, (P<ModDecl>, SourceID)>,
    procs: HashMap<InternID, (P<ProcDecl>, SourceID)>,
    types: HashMap<InternID, (TypeSymbol, SourceID)>,
    globals: HashMap<InternID, (P<GlobalDecl>, SourceID)>,
}

#[derive(Copy, Clone)]
pub enum TypeSymbol {
    Enum(P<EnumDecl>),
    Struct(P<StructDecl>),
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            mods: HashMap::new(),
            procs: HashMap::new(),
            types: HashMap::new(),
            globals: HashMap::new(),
        }
    }

    pub fn get_mod(&self, id: InternID) -> Option<(P<ModDecl>, SourceID)> {
        match self.mods.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }

    pub fn get_proc(&self, id: InternID) -> Option<(P<ProcDecl>, SourceID)> {
        match self.procs.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }

    pub fn get_type(&self, id: InternID) -> Option<(TypeSymbol, SourceID)> {
        match self.types.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }

    pub fn get_global(&self, id: InternID) -> Option<(P<GlobalDecl>, SourceID)> {
        match self.globals.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }

    pub fn add_mod(
        &mut self,
        mod_decl: P<ModDecl>,
        source: SourceID,
    ) -> Result<(), (P<ModDecl>, SourceID)> {
        let id = mod_decl.name.id;
        match self.get_mod(id) {
            Some(v) => Err(v),
            None => {
                self.mods.insert(id, (mod_decl, source));
                Ok(())
            }
        }
    }

    pub fn add_proc(
        &mut self,
        proc_decl: P<ProcDecl>,
        source: SourceID,
    ) -> Result<(), (P<ProcDecl>, SourceID)> {
        let id = proc_decl.name.id;
        match self.get_proc(id) {
            Some(v) => Err(v),
            None => {
                self.procs.insert(id, (proc_decl, source));
                Ok(())
            }
        }
    }

    pub fn add_type(
        &mut self,
        tt: TypeSymbol,
        source: SourceID,
    ) -> Result<(), (TypeSymbol, SourceID)> {
        let id = tt.name().id;
        match self.get_type(id) {
            Some(v) => Err(v),
            None => {
                self.types.insert(id, (tt, source));
                Ok(())
            }
        }
    }

    pub fn add_global(
        &mut self,
        global_decl: P<GlobalDecl>,
        source: SourceID,
    ) -> Result<(), (P<GlobalDecl>, SourceID)> {
        let id = global_decl.name.id;
        match self.get_global(id) {
            Some(v) => Err(v),
            None => {
                self.globals.insert(id, (global_decl, source));
                Ok(())
            }
        }
    }

    pub fn merge(&mut self, mut other: SymbolTable) {
        for (id, (v, source)) in other.mods.drain() {
            self.mods.entry(id).or_insert((v, source));
        }
        for (id, (v, source)) in other.procs.drain() {
            self.procs.entry(id).or_insert((v, source));
        }
        for (id, (v, source)) in other.types.drain() {
            self.types.entry(id).or_insert((v, source));
        }
        for (id, (v, source)) in other.globals.drain() {
            self.globals.entry(id).or_insert((v, source));
        }
    }
}

impl TypeSymbol {
    pub fn name(&self) -> Ident {
        match self {
            TypeSymbol::Enum(enum_decl) => enum_decl.name,
            TypeSymbol::Struct(struct_decl) => struct_decl.name,
        }
    }
}
