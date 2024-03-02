//pub mod check;

use crate::ast::intern::InternID;
use crate::ast::span::Span;
use crate::ast::FileID;
use crate::ast::{ast::*, CompCtx};
use crate::err::error_new::{CompError, ErrorContext};
use crate::err::{ansi, span_fmt};
use std::collections::HashMap;

#[derive(Copy, Clone)]
pub struct SourceLoc {
    pub span: Span,
    pub file_id: FileID,
}

impl SourceLoc {
    pub fn new(span: Span, file_id: FileID) -> Self {
        Self { span, file_id }
    }
}

pub fn report_check_errors_cli(ctx: &CompCtx, errors: &[CompError]) {
    for error in errors {
        let ansi_red = ansi::Color::as_ansi_str(ansi::Color::BoldRed);
        let ansi_clear = "\x1B[0m";
        eprintln!("\n{}error:{} {}", ansi_red, ansi_clear, error.msg.as_str());
        span_fmt::print_simple(ctx.file(error.src.file_id), error.src.span, None, false);

        for context in error.context.iter() {
            match context {
                ErrorContext::Message { msg } => {
                    eprintln!("{}", msg.as_str());
                }
                ErrorContext::MessageSource { ctx_src, msg } => {
                    span_fmt::print_simple(
                        ctx.file(ctx_src.file_id),
                        ctx_src.span,
                        Some(msg.as_str()),
                        true,
                    );
                }
            }
        }
    }
}

/*

pub struct Context {
    scopes: Vec<Scope>,
    mods: Vec<ModData>,
    procs: Vec<ProcData>,
    enums: Vec<EnumData>,
    unions: Vec<UnionData>,
    structs: Vec<StructData>,
    consts: Vec<ConstData>,
    globals: Vec<GlobalData>,
}

pub struct Scope {
    pub module: Box<Module>,
    pub parent_id: Option<ScopeID>,
    symbols: HashMap<InternID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Declared { symbol_id: SymbolID },
    Imported { symbol_id: SymbolID, import: Span },
}

#[derive(Copy, Clone)]
pub enum SymbolID {
    Mod(ModID),
    Proc(ProcID),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
    Const(ConstID),
    Global(GlobalID),
}

#[derive(Copy, Clone, PartialEq)]
pub struct ScopeID(u32);

#[derive(Copy, Clone)]
pub struct ModID(u32);

#[derive(Copy, Clone)]
pub struct ProcID(u32);

#[derive(Copy, Clone, PartialEq, std::fmt::Debug)]
pub struct EnumID(u32);

#[derive(Copy, Clone, PartialEq, std::fmt::Debug)]
pub struct UnionID(u32);

#[derive(Copy, Clone, PartialEq, std::fmt::Debug)]
pub struct StructID(u32);

#[derive(Copy, Clone)]
pub struct ConstID(u32);

#[derive(Copy, Clone)]
pub struct GlobalID(u32);

pub struct ModData {
    pub from_id: ScopeID,
    pub decl: Box<ModDecl>,
    pub target_id: Option<ScopeID>,
}

pub struct ProcData {
    pub from_id: ScopeID,
    pub decl: Box<ProcDecl>,
}

pub struct EnumData {
    pub from_id: ScopeID,
    pub decl: Box<EnumDecl>,
}

pub struct UnionData {
    pub from_id: ScopeID,
    pub decl: Box<UnionDecl>,
    pub size: usize,
    pub align: u32,
}

pub struct StructData {
    pub from_id: ScopeID,
    pub decl: Box<StructDecl>,
    pub size: usize,
    pub align: u32,
}

pub struct ConstData {
    pub from_id: ScopeID,
    pub decl: Box<ConstDecl>,
}

pub struct GlobalData {
    pub from_id: ScopeID,
    pub decl: Box<GlobalDecl>,
}

pub struct ScopeIter {
    curr: u32,
    len: u32,
}

impl Iterator for ScopeIter {
    type Item = ScopeID;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr >= self.len {
            None
        } else {
            let scope_id = ScopeID(self.curr);
            self.curr += 1;
            Some(scope_id)
        }
    }
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
        $item_id:ident, $symbol_name:ident, $fn_get:ident, $fn_declared_get:ident;
    )+) => {
        $(
        #[must_use]
        pub fn $fn_get(&self, id: InternID) -> Result<$item_id, Option<SymbolID>> {
            match self.symbols.get(&id).cloned() {
                Some(Symbol::Declared { symbol_id }) => {
                    if let SymbolID::$symbol_name(id) = symbol_id {
                        Ok(id)
                    } else {
                        Err(Some(symbol_id))
                    }
                }
                Some(Symbol::Imported { symbol_id, .. }) => {
                    if let SymbolID::$symbol_name(id) = symbol_id {
                        Ok(id)
                    } else {
                        Err(Some(symbol_id))
                    }
                }
                None => Err(None),
            }
        }
        #[must_use]
        pub fn $fn_declared_get(&self, id: InternID) -> Result<$item_id, Option<SymbolID>> {
            match self.symbols.get(&id).cloned() {
                Some(Symbol::Declared { symbol_id }) => {
                    if let SymbolID::$symbol_name(id) = symbol_id {
                        Ok(id)
                    } else {
                        Err(Some(symbol_id))
                    }
                }
                _ => Err(None),
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
            mods: Vec::new(),
            procs: Vec::new(),
            enums: Vec::new(),
            unions: Vec::new(),
            structs: Vec::new(),
            consts: Vec::new(),
            globals: Vec::new(),
        }
    }

    impl_context_item! {
        Scope, ScopeID, scopes, add_scope, get_scope, get_scope_mut;
        ModData, ModID, mods, add_mod, get_mod, get_mod_mut;
        ProcData, ProcID, procs, add_proc, get_proc, get_proc_mut;
        EnumData, EnumID, enums, add_enum, get_enum, get_enum_mut;
        UnionData, UnionID, unions, add_union, get_union, get_union_mut;
        StructData, StructID, structs, add_struct, get_struct, get_struct_mut;
        ConstData, ConstID, consts, add_const, get_const, get_const_mut;
        GlobalData, GlobalID, globals, add_global, get_global, get_global_mut;
    }

    #[must_use]
    pub fn scope_iter(&self) -> ScopeIter {
        ScopeIter {
            curr: 0,
            len: self.scopes.len() as u32,
        }
    }

    pub fn get_symbol_src(&self, scope_id: ScopeID, symbol: Symbol) -> SourceLoc {
        match symbol {
            Symbol::Declared { symbol_id } => self.get_symbol_id_src(symbol_id),
            Symbol::Imported { import, .. } => {
                let scope = self.get_scope(scope_id);
                scope.src(import)
            }
        }
    }

    #[must_use]
    pub fn get_symbol_id_src(&self, symbol_id: SymbolID) -> SourceLoc {
        match symbol_id {
            SymbolID::Mod(id) => {
                let mod_data = self.get_mod(id);
                self.get_scope(mod_data.from_id)
                    .src(mod_data.decl.name.span)
            }
            SymbolID::Proc(id) => {
                let proc_data = self.get_proc(id);
                self.get_scope(proc_data.from_id)
                    .src(proc_data.decl.name.span)
            }
            SymbolID::Enum(id) => {
                let enum_data = self.get_enum(id);
                self.get_scope(enum_data.from_id)
                    .src(enum_data.decl.name.span)
            }
            SymbolID::Union(id) => {
                let union_data = self.get_union(id);
                self.get_scope(union_data.from_id)
                    .src(union_data.decl.name.span)
            }
            SymbolID::Struct(id) => {
                let struct_data = self.get_struct(id);
                self.get_scope(struct_data.from_id)
                    .src(struct_data.decl.name.span)
            }
            SymbolID::Const(id) => {
                let const_data = self.get_const(id);
                self.get_scope(const_data.from_id)
                    .src(const_data.decl.name.span)
            }
            SymbolID::Global(id) => {
                let global_data = self.get_global(id);
                self.get_scope(global_data.from_id)
                    .src(global_data.decl.name.span)
            }
        }
    }

    #[must_use]
    pub fn get_symbol_vis(&self, symbol_id: SymbolID) -> Vis {
        match symbol_id {
            SymbolID::Mod(id) => self.get_mod(id).decl.vis,
            SymbolID::Proc(id) => self.get_proc(id).decl.vis,
            SymbolID::Enum(id) => self.get_enum(id).decl.vis,
            SymbolID::Union(id) => self.get_union(id).decl.vis,
            SymbolID::Struct(id) => self.get_struct(id).decl.vis,
            SymbolID::Const(id) => self.get_const(id).decl.vis,
            SymbolID::Global(id) => self.get_global(id).decl.vis,
        }
    }
}

impl Scope {
    #[must_use]
    pub fn new(module: Box<Module>, parent_id: Option<ScopeID>) -> Self {
        let decls_len = module.decls.len();
        Self {
            module,
            parent_id,
            symbols: HashMap::with_capacity(decls_len),
        }
    }

    impl_scope_item! {
        ModID, Mod, get_mod, get_declared_mod;
        ProcID, Proc, get_proc, get_declared_proc;
        EnumID, Enum, get_enum, get_declared_enum;
        UnionID, Union, get_union, get_declared_union;
        StructID, Struct, get_struct, get_declared_struct;
        ConstID, Const, get_const, get_declared_const;
        GlobalID, Global, get_global, get_declared_global;
    }

    #[must_use]
    pub fn src(&self, span: Span) -> SourceLoc {
        SourceLoc {
            span,
            file_id: self.module.file_id,
        }
    }

    #[must_use]
    pub fn add_declared_symbol(&mut self, id: InternID, symbol_id: SymbolID) -> Result<(), Symbol> {
        match self.symbols.get(&id).cloned() {
            Some(existing) => Err(existing),
            None => {
                let symbol = Symbol::Declared { symbol_id };
                self.symbols.insert(id, symbol);
                Ok(())
            }
        }
    }

    #[must_use]
    pub fn add_imported_symbol(
        &mut self,
        id: InternID,
        symbol_id: SymbolID,
        import: Span,
    ) -> Result<(), Symbol> {
        match self.symbols.get(&id).cloned() {
            Some(existing) => Err(existing),
            None => {
                let symbol = Symbol::Imported { symbol_id, import };
                self.symbols.insert(id, symbol);
                Ok(())
            }
        }
    }

    #[must_use]
    pub fn get_symbol(&self, id: InternID) -> Option<SymbolID> {
        match self.symbols.get(&id).cloned() {
            Some(Symbol::Declared { symbol_id }) => Some(symbol_id),
            Some(Symbol::Imported { symbol_id, .. }) => Some(symbol_id),
            None => None,
        }
    }

    #[must_use]
    pub fn get_declared_symbol(&self, id: InternID) -> Option<SymbolID> {
        match self.symbols.get(&id).cloned() {
            Some(Symbol::Declared { symbol_id }) => Some(symbol_id),
            _ => None,
        }
    }
}
*/
