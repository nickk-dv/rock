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

pub enum Symbol {
    Mod((P<ModDecl>, SourceID)),
    Proc((P<ProcDecl>, SourceID)),
    Type((TypeSymbol, SourceID)),
    Global((P<GlobalDecl>, SourceID)),
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

    pub fn get_public_unique(&self, id: InternID) -> Result<Option<Symbol>, Vec<Symbol>> {
        let mut unique = None;
        let mut conflits = Vec::new();

        if let Some(mod_decl) = self.get_mod(id) {
            if mod_decl.0.visibility == Visibility::Public {
                let symbol = Symbol::Mod(mod_decl);
                unique = Some(symbol);
            }
        }
        if let Some(proc_decl) = self.get_proc(id) {
            if proc_decl.0.visibility == Visibility::Public {
                let symbol = Symbol::Proc(proc_decl);
                match unique {
                    Some(..) => conflits.push(symbol),
                    None => unique = Some(symbol),
                }
            }
        }
        if let Some(type_decl) = self.get_type(id) {
            if type_decl.0.visibility() == Visibility::Public {
                let symbol = Symbol::Type(type_decl);
                match unique {
                    Some(..) => conflits.push(symbol),
                    None => unique = Some(symbol),
                }
            }
        }
        if let Some(global_decl) = self.get_global(id) {
            if global_decl.0.visibility == Visibility::Public {
                let symbol = Symbol::Global(global_decl);
                match unique {
                    Some(..) => conflits.push(symbol),
                    None => unique = Some(symbol),
                }
            }
        }
        if conflits.is_empty() {
            Ok(unique)
        } else {
            Err(conflits)
        }
    }

    pub fn get_all_private(&self, id: InternID) -> Vec<Symbol> {
        let mut private_symbols = Vec::new();
        if let Some(mod_decl) = self.get_mod(id) {
            if mod_decl.0.visibility == Visibility::Private {
                private_symbols.push(Symbol::Mod(mod_decl));
            }
        }
        if let Some(proc_decl) = self.get_proc(id) {
            if proc_decl.0.visibility == Visibility::Public {
                private_symbols.push(Symbol::Proc(proc_decl));
            }
        }
        if let Some(type_decl) = self.get_type(id) {
            if type_decl.0.visibility() == Visibility::Public {
                private_symbols.push(Symbol::Type(type_decl));
            }
        }
        if let Some(global_decl) = self.get_global(id) {
            if global_decl.0.visibility == Visibility::Public {
                private_symbols.push(Symbol::Global(global_decl));
            }
        }
        private_symbols
    }
}

impl TypeSymbol {
    pub fn name(&self) -> Ident {
        match self {
            TypeSymbol::Enum(enum_decl) => enum_decl.name,
            TypeSymbol::Struct(struct_decl) => struct_decl.name,
        }
    }

    pub fn visibility(&self) -> Visibility {
        match self {
            TypeSymbol::Enum(enum_decl) => enum_decl.visibility,
            TypeSymbol::Struct(struct_decl) => struct_decl.visibility,
        }
    }
}

impl Symbol {
    pub fn name(&self) -> Ident {
        match self {
            Symbol::Mod(mod_decl) => mod_decl.0.name,
            Symbol::Proc(proc_decl) => proc_decl.0.name,
            Symbol::Type(type_decl) => type_decl.0.name(),
            Symbol::Global(global_decl) => global_decl.0.name,
        }
    }
}
