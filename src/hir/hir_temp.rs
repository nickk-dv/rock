use super::hir;
use crate::ast::ast;
use crate::ast::intern;
use crate::text_range::TextRange;
use std::collections::HashMap;

pub struct HirTemp<'ast> {
    ast: ast::Ast<'ast>,
    mods: Vec<ModData>,
    scopes: Vec<Scope>,
    scopes_temp: Vec<ScopeTemp<'ast>>,
    const_exprs: Vec<ConstExprTemp<'ast>>,
}

#[derive(Copy, Clone)]
pub struct ModID(u32);
pub struct ModData {
    pub from_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub target: Option<ScopeID>,
}

#[derive(Copy, Clone)]
pub struct ScopeID(u32);
pub struct Scope {
    parent: Option<ScopeID>,
    symbols: HashMap<intern::InternID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Defined { kind: SymbolKind },
    Imported { kind: SymbolKind, import: TextRange },
}

#[derive(Copy, Clone)]
pub enum SymbolKind {
    Mod(ModID),
    Proc(hir::ProcID),
    Enum(hir::EnumID),
    Union(hir::UnionID),
    Struct(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
}

pub struct ScopeTemp<'ast> {
    parent: Option<ScopeID>,
    module: ast::Module<'ast>,
    symbols: HashMap<intern::InternID, SymbolTemp<'ast>>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum SymbolTemp<'ast> {
    Defined  { kind: SymbolTempKind<'ast> },
    Imported { kind: SymbolTempKind<'ast>, import: TextRange },
}

#[derive(Copy, Clone)]
pub enum SymbolTempKind<'ast> {
    Mod(ModID),
    Proc(&'ast ast::ProcDecl<'ast>),
    Enum(&'ast ast::EnumDecl<'ast>),
    Union(&'ast ast::UnionDecl<'ast>),
    Struct(&'ast ast::StructDecl<'ast>),
    Const(&'ast ast::ConstDecl<'ast>),
    Global(&'ast ast::GlobalDecl<'ast>),
}

pub struct ConstExprTemp<'ast> {
    pub from_id: ScopeID,
    pub source: &'ast ast::Expr<'ast>,
}

pub struct ScopeIter {
    curr: u32,
    len: u32,
}

impl<'ast> HirTemp<'ast> {
    pub fn new(ast: ast::Ast<'ast>) -> HirTemp {
        HirTemp {
            ast,
            mods: Vec::new(),
            scopes: Vec::new(),
            scopes_temp: Vec::new(),
            const_exprs: Vec::new(),
        }
    }

    pub fn ast_modules(&self) -> impl Iterator<Item = &ast::Module<'ast>> {
        self.ast.modules.iter()
    }

    pub fn scope_ids(&self) -> ScopeIter {
        ScopeIter {
            curr: 0,
            len: self.scopes.len() as u32,
        }
    }
    pub fn scope_temp_ids(&self) -> ScopeIter {
        ScopeIter {
            curr: 0,
            len: self.scopes_temp.len() as u32,
        }
    }

    pub fn add_mod(&mut self, data: ModData) -> ModID {
        self.mods.push(data);
        ModID((self.mods.len() - 1) as u32)
    }
    pub fn get_mod(&self, id: ModID) -> &ModData {
        self.mods.get(id.0 as usize).unwrap()
    }
    pub fn get_mod_mut(&mut self, id: ModID) -> &mut ModData {
        self.mods.get_mut(id.0 as usize).unwrap()
    }

    pub fn add_scope(&mut self, scope: Scope) {
        self.scopes.push(scope);
    }
    pub fn get_scope(&self, id: ScopeID) -> &Scope {
        self.scopes.get(id.0 as usize).unwrap()
    }
    pub fn get_scope_mut(&mut self, id: ScopeID) -> &mut Scope {
        self.scopes.get_mut(id.0 as usize).unwrap()
    }

    pub fn add_scope_temp(&mut self, scope: ScopeTemp<'ast>) -> ScopeID {
        self.scopes_temp.push(scope);
        ScopeID((self.scopes_temp.len() - 1) as u32)
    }
    pub fn get_scope_temp(&self, id: ScopeID) -> &ScopeTemp<'ast> {
        self.scopes_temp.get(id.0 as usize).unwrap()
    }
    pub fn get_scope_temp_mut(&mut self, id: ScopeID) -> &mut ScopeTemp<'ast> {
        self.scopes_temp.get_mut(id.0 as usize).unwrap()
    }
}

impl<'ast> ScopeTemp<'ast> {
    pub fn new(parent: Option<ScopeID>, module: ast::Module<'ast>) -> ScopeTemp {
        ScopeTemp {
            parent,
            module,
            symbols: HashMap::new(),
        }
    }

    pub fn parent(&self) -> Option<ScopeID> {
        self.parent
    }

    pub fn module_file_id(&self) -> crate::ast::FileID {
        self.module.file_id
    }

    pub fn module_decls(&self) -> impl Iterator<Item = ast::Decl<'ast>> {
        self.module.decls.into_iter()
    }

    pub fn add_symbol(
        &mut self,
        id: intern::InternID,
        symbol: SymbolTemp<'ast>,
    ) -> Result<(), SymbolTemp<'ast>> {
        match self.symbols.get(&id).cloned() {
            Some(existing) => Err(existing),
            None => {
                self.symbols.insert(id, symbol);
                Ok(())
            }
        }
    }

    pub fn get_symbol(&self, id: intern::InternID) -> Option<SymbolTemp<'ast>> {
        self.symbols.get(&id).cloned()
    }
}

impl Scope {
    pub fn new(parent: Option<ScopeID>) -> Scope {
        Scope {
            parent,
            symbols: HashMap::new(),
        }
    }

    pub fn parent(&self) -> Option<ScopeID> {
        self.parent
    }

    pub fn add_symbol(&mut self, id: intern::InternID, symbol: Symbol) -> Result<(), Symbol> {
        match self.symbols.get(&id).cloned() {
            Some(existing) => Err(existing),
            None => {
                self.symbols.insert(id, symbol);
                Ok(())
            }
        }
    }

    pub fn get_symbol(&self, id: intern::InternID) -> Option<Symbol> {
        self.symbols.get(&id).cloned()
    }
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
