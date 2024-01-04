use crate::ast::ast::*;
use crate::mem::{InternID, P};
use std::collections::HashMap;

pub struct SymbolTable {
    table: HashMap<InternID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Mod((P<ModDecl>, SourceID)),
    Proc((P<ProcDecl>, SourceID)),
    Enum((P<EnumDecl>, SourceID)),
    Struct((P<StructDecl>, SourceID)),
    Global((P<GlobalDecl>, SourceID)),
}

#[derive(Copy, Clone)]
pub enum TypeSymbol {
    Enum((P<EnumDecl>, SourceID)),
    Struct((P<StructDecl>, SourceID)),
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

    pub fn get_mod(&self, id: InternID) -> Option<(P<ModDecl>, SourceID)> {
        match self.table.get(&id) {
            Some(Symbol::Mod(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_proc(&self, id: InternID) -> Option<(P<ProcDecl>, SourceID)> {
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

    pub fn get_global(&self, id: InternID) -> Option<(P<GlobalDecl>, SourceID)> {
        match self.table.get(&id) {
            Some(Symbol::Global(v)) => Some(*v),
            _ => None,
        }
    }
}

impl Symbol {
    pub fn from_decl(decl: Decl, source_id: SourceID) -> Option<Self> {
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
