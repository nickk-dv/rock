use crate::ast::ast::*;
use crate::ast::intern::InternID;
use crate::ast::parser::FileID;
use crate::ast::span::Span;
use crate::mem::P;
use std::collections::HashMap;

pub mod check;

pub struct Context {
    scopes: Vec<Scope>,
    modules: Vec<ModuleData>,
    globals: Vec<GlobalData>,
    procs: Vec<ProcData>,
    enums: Vec<EnumData>,
    unions: Vec<UnionData>,
    structs: Vec<StructData>,
}

pub struct Scope {
    pub module: P<Module>,
    symbols: HashMap<InternID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Module(ModuleID),
    Global(GlobalID),
    Proc(ProcID),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
}

#[derive(Copy, Clone)]
pub struct ScopeID(u32);

#[derive(Copy, Clone)]
pub struct ModuleID(u32);

#[derive(Copy, Clone)]
pub struct GlobalID(u32);

#[derive(Copy, Clone)]
pub struct ProcID(u32);

#[derive(Copy, Clone)]
pub struct EnumID(u32);

#[derive(Copy, Clone)]
pub struct UnionID(u32);

#[derive(Copy, Clone)]
pub struct StructID(u32);

pub struct ModuleData {
    pub from_id: ScopeID,
    pub decl: P<ModuleDecl>,
    pub target_id: Option<ScopeID>,
}

pub struct GlobalData {
    pub from_id: ScopeID,
    pub decl: P<GlobalDecl>,
}

pub struct ProcData {
    pub from_id: ScopeID,
    pub decl: P<ProcDecl>,
}

pub struct EnumData {
    pub from_id: ScopeID,
    pub decl: P<EnumDecl>,
}

pub struct UnionData {
    pub from_id: ScopeID,
    pub decl: P<UnionDecl>,
    pub size: usize,
    pub align: u32,
}

pub struct StructData {
    pub from_id: ScopeID,
    pub decl: P<StructDecl>,
    pub size: usize,
    pub align: u32,
}

macro_rules! impl_context_item {
    ($(
        $item:ty, $item_id:ident, $collection:ident,
        $fn_add:ident, $fn_get:ident, $fn_get_mut:ident;
    )+) => {
        $(
        #[must_use]
        pub fn $fn_add(&mut self, data: $item) -> $item_id {
            let id = $item_id(self.$collection.len() as u32);
            self.$collection.push(data);
            id
        }

        #[must_use]
        pub fn $fn_get(&self, id: $item_id) -> &$item {
            self.$collection.get(id.0 as usize).unwrap() //@unwrapping
        }

        #[must_use]
        pub fn $fn_get_mut(&mut self, id: $item_id) -> &mut $item {
            self.$collection.get_mut(id.0 as usize).unwrap() //@unwrapping
        }
        )+
    };
}

macro_rules! impl_scope_item {
    ($(
        $item_id:ident, $symbol_name:ident, $fn_get:ident;
    )+) => {
        $(
        #[must_use]
        pub fn $fn_get(&self, id: InternID) -> Result<$item_id, Option<Symbol>> {
            match self.symbols.get(&id).cloned() {
                Some(symbol) => {
                    if let Symbol::$symbol_name(id) = symbol {
                        Ok(id)
                    } else {
                        Err(Some(symbol))
                    }
                }
                None => Err(None),
            }
        }
        )+
    };
}

impl Context {
    #[must_use]
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            modules: Vec::new(),
            globals: Vec::new(),
            procs: Vec::new(),
            enums: Vec::new(),
            unions: Vec::new(),
            structs: Vec::new(),
        }
    }

    impl_context_item! {
        Scope, ScopeID, scopes, add_scope, get_scope, get_scope_mut;
        ModuleData, ModuleID, modules, add_module, get_module, get_module_mut;
        GlobalData, GlobalID, globals, add_global, get_global, get_global_mut;
        ProcData, ProcID, procs, add_proc, get_proc, get_proc_mut;
        EnumData, EnumID, enums, add_enum, get_enum, get_enum_mut;
        UnionData, UnionID, unions, add_union, get_union, get_union_mut;
        StructData, StructID, structs, add_struct, get_struct, get_struct_mut;
    }

    pub fn get_symbol_src(&self, symbol: Symbol) -> SourceLoc {
        match symbol {
            Symbol::Module(id) => {
                let module_data = self.get_module(id);
                let scope = self.get_scope(module_data.from_id);
                SourceLoc::new(module_data.decl.name.span, scope.module.file_id)
            }
            Symbol::Global(id) => {
                let global_data = self.get_global(id);
                let scope = self.get_scope(global_data.from_id);
                SourceLoc::new(global_data.decl.name.span, scope.module.file_id)
            }
            Symbol::Proc(id) => {
                let proc_data = self.get_proc(id);
                let scope = self.get_scope(proc_data.from_id);
                SourceLoc::new(proc_data.decl.name.span, scope.module.file_id)
            }
            Symbol::Enum(id) => {
                let enum_data = self.get_enum(id);
                let scope = self.get_scope(enum_data.from_id);
                SourceLoc::new(enum_data.decl.name.span, scope.module.file_id)
            }
            Symbol::Union(id) => {
                let union_data = self.get_union(id);
                let scope = self.get_scope(union_data.from_id);
                SourceLoc::new(union_data.decl.name.span, scope.module.file_id)
            }
            Symbol::Struct(id) => {
                let struct_data = self.get_struct(id);
                let scope = self.get_scope(struct_data.from_id);
                SourceLoc::new(struct_data.decl.name.span, scope.module.file_id)
            }
        }
    }
}

impl Scope {
    #[must_use]
    pub fn new(module: P<Module>) -> Self {
        Self {
            module,
            symbols: HashMap::with_capacity(module.decls.len()),
        }
    }

    impl_scope_item! {
        ModuleID, Module, get_module;
        GlobalID, Global, get_global;
        ProcID, Proc, get_proc;
        EnumID, Enum, get_enum;
        UnionID, Union, get_union;
        StructID, Struct, get_struct;
    }

    #[must_use]
    pub fn add_symbol(&mut self, id: InternID, symbol: Symbol) -> Result<(), Symbol> {
        match self.symbols.get(&id).cloned() {
            Some(existing) => Err(existing),
            None => {
                self.symbols.insert(id, symbol);
                Ok(())
            }
        }
    }

    #[must_use]
    pub fn get_symbol(&self, id: InternID) -> Option<Symbol> {
        self.symbols.get(&id).cloned()
    }
}

pub struct SourceLoc {
    pub span: Span,
    pub file_id: FileID,
}

impl SourceLoc {
    pub fn new(span: Span, file_id: FileID) -> Self {
        Self { span, file_id }
    }
}
