use crate::ast::ast::*;
use crate::ast::intern::*;
use crate::ast::span::Span;
use crate::err::error::*;
use crate::mem::*;
use std::collections::HashMap;

pub struct Scope {
    pub id: ScopeID,
    pub module: P<Module>,
    pub parent: Option<ScopeID>,
    pub errors: Drop<Vec<Error>>,
    mods: Drop<HashMap<InternID, P<ModuleDecl>>>,
    procs: Drop<HashMap<InternID, ProcData>>,
    enums: Drop<HashMap<InternID, EnumData>>,
    unions: Drop<HashMap<InternID, UnionData>>,
    structs: Drop<HashMap<InternID, StructData>>,
    globals: Drop<HashMap<InternID, GlobalData>>,
    pub glob_imports: Drop<Vec<GlobImport>>,
    pub symbol_imports: Drop<HashMap<InternID, SymbolImport>>,
}

impl ManualDrop for P<Scope> {
    fn manual_drop(mut self) {
        unsafe {
            Drop::drop(&mut self.errors);
            Drop::drop(&mut self.mods);
            Drop::drop(&mut self.procs);
            Drop::drop(&mut self.enums);
            Drop::drop(&mut self.unions);
            Drop::drop(&mut self.structs);
            Drop::drop(&mut self.globals);
            Drop::drop(&mut self.glob_imports);
            Drop::drop(&mut self.symbol_imports);
        }
    }
}

impl Scope {
    pub fn new(id: ScopeID, module: P<Module>, parent: Option<ScopeID>) -> Self {
        Self {
            id,
            module,
            parent,
            errors: Drop::new(Vec::new()),
            mods: Drop::new(HashMap::new()),
            procs: Drop::new(HashMap::new()),
            enums: Drop::new(HashMap::new()),
            unions: Drop::new(HashMap::new()),
            structs: Drop::new(HashMap::new()),
            globals: Drop::new(HashMap::new()),
            glob_imports: Drop::new(Vec::new()),
            symbol_imports: Drop::new(HashMap::new()),
        }
    }
}

pub type GlobalID = u32;
pub type ProcID = u32;
pub type EnumID = u32;
pub type UnionID = u32;
pub type StructID = u32;

#[derive(Copy, Clone)]
pub struct ProcData {
    pub decl: P<ProcDecl>,
    pub id: ProcID,
}

#[derive(Copy, Clone)]
pub struct EnumData {
    pub decl: P<EnumDecl>,
    pub id: EnumID,
}

#[derive(Copy, Clone)]
pub struct UnionData {
    pub decl: P<UnionDecl>,
    pub id: UnionID,
}

#[derive(Copy, Clone)]
pub struct StructData {
    pub decl: P<StructDecl>,
    pub id: StructID,
}

#[derive(Copy, Clone)]
pub struct GlobalData {
    pub decl: P<GlobalDecl>,
    pub id: GlobalID,
}

#[derive(Copy, Clone)]
pub enum TypeData {
    Enum(EnumData),
    Union(UnionData),
    Struct(StructData),
}

#[derive(Copy, Clone)]
pub struct GlobImport {
    pub from_id: ScopeID,
    pub import_span: Span,
}

#[derive(Copy, Clone)]
pub struct SymbolImport {
    pub from_id: ScopeID,
    pub name: Ident,
}

impl TypeData {
    pub fn name(&self) -> Ident {
        match self {
            TypeData::Enum(data) => data.decl.name,
            TypeData::Union(data) => data.decl.name,
            TypeData::Struct(data) => data.decl.name,
        }
    }

    pub fn vis(&self) -> Vis {
        match self {
            TypeData::Enum(data) => data.decl.vis,
            TypeData::Union(data) => data.decl.vis,
            TypeData::Struct(data) => data.decl.vis,
        }
    }
}

impl Scope {
    pub fn md(&self) -> P<Module> {
        self.module.copy()
    }

    pub fn error(&mut self, error: CheckError, span: Span) {
        let md = self.md();
        self.errors
            .push(Error::check(error, md.file_id, span).into());
    }

    pub fn add_mod(&mut self, decl: P<ModuleDecl>) -> Result<(), P<ModuleDecl>> {
        if let Some(existing) = self.mods.get(&decl.name.id) {
            return Err(*existing);
        }
        self.mods.insert(decl.name.id, decl);
        Ok(())
    }

    pub fn add_proc(&mut self, decl: P<ProcDecl>, id: ProcID) -> Result<(), ProcData> {
        if let Some(existing) = self.procs.get(&decl.name.id) {
            return Err(*existing);
        }
        self.procs.insert(decl.name.id, ProcData { decl, id });
        Ok(())
    }

    pub fn add_enum(&mut self, decl: P<EnumDecl>, id: EnumID) -> Result<(), TypeData> {
        if let Some(existing) = self.enums.get(&decl.name.id) {
            return Err(TypeData::Enum(*existing));
        }
        if let Some(existing) = self.unions.get(&decl.name.id) {
            return Err(TypeData::Union(*existing));
        }
        if let Some(existing) = self.structs.get(&decl.name.id) {
            return Err(TypeData::Struct(*existing));
        }
        self.enums.insert(decl.name.id, EnumData { decl, id });
        Ok(())
    }

    pub fn add_union(&mut self, decl: P<UnionDecl>, id: UnionID) -> Result<(), TypeData> {
        if let Some(existing) = self.enums.get(&decl.name.id) {
            return Err(TypeData::Enum(*existing));
        }
        if let Some(existing) = self.unions.get(&decl.name.id) {
            return Err(TypeData::Union(*existing));
        }
        if let Some(existing) = self.structs.get(&decl.name.id) {
            return Err(TypeData::Struct(*existing));
        }
        self.unions.insert(decl.name.id, UnionData { decl, id });
        Ok(())
    }

    pub fn add_struct(&mut self, decl: P<StructDecl>, id: StructID) -> Result<(), TypeData> {
        if let Some(existing) = self.enums.get(&decl.name.id) {
            return Err(TypeData::Enum(*existing));
        }
        if let Some(existing) = self.unions.get(&decl.name.id) {
            return Err(TypeData::Union(*existing));
        }
        if let Some(existing) = self.structs.get(&decl.name.id) {
            return Err(TypeData::Struct(*existing));
        }
        self.structs.insert(decl.name.id, StructData { decl, id });
        Ok(())
    }

    pub fn add_global(&mut self, decl: P<GlobalDecl>, id: GlobalID) -> Result<(), GlobalData> {
        if let Some(existing) = self.globals.get(&decl.name.id) {
            return Err(*existing);
        }
        self.globals.insert(decl.name.id, GlobalData { decl, id });
        Ok(())
    }

    pub fn add_glob_import(&mut self, import: GlobImport) -> Result<(), GlobImport> {
        for existing in self.glob_imports.iter() {
            if import.from_id == import.from_id {
                return Err(*existing);
            }
        }
        self.glob_imports.push(import);
        Ok(())
    }

    pub fn add_symbol_import(&mut self, import: SymbolImport) -> Result<(), SymbolImport> {
        if let Some(existing) = self.symbol_imports.get(&import.name.id) {
            return Err(*existing);
        }
        self.symbol_imports.insert(import.name.id, import);
        Ok(())
    }

    pub fn get_mod(&self, id: InternID) -> Option<P<ModuleDecl>> {
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
        if let Some(v) = self.enums.get(&id) {
            return Some(TypeData::Enum(*v));
        }
        if let Some(v) = self.unions.get(&id) {
            return Some(TypeData::Union(*v));
        }
        if let Some(v) = self.structs.get(&id) {
            return Some(TypeData::Struct(*v));
        }
        None
    }

    pub fn get_global(&self, id: InternID) -> Option<GlobalData> {
        match self.globals.get(&id) {
            Some(v) => Some(*v),
            None => None,
        }
    }
}
