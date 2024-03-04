use super::hir;
use crate::ast::ast;
use crate::ast::intern;
use crate::ast::span::Span;
use std::collections::HashMap;

pub struct HirTemp<'ast> {
    ast: ast::Ast<'ast>,
    scopes_temp: Vec<ScopeTemp<'ast>>,
    const_exprs: Vec<ConstExprTemp<'ast>>,
    mods: Vec<ModData>,
    scopes: Vec<Scope>,
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
    Imported { kind: SymbolTempKind<'ast>, import: Span },
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

#[derive(Copy, Clone)]
pub struct ScopeID(u32);
pub struct Scope {
    parent: Option<ScopeID>,
    symbols: HashMap<intern::InternID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Defined { kind: SymbolKind },
    Imported { kind: SymbolKind, import: Span },
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

pub struct ConstExprTemp<'ast> {
    pub from_id: ScopeID,
    pub source: &'ast ast::Expr<'ast>,
}

#[derive(Copy, Clone)]
pub struct ModID(u32);
pub struct ModData {
    pub from_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub target: Option<ScopeID>,
}

impl<'ast> HirTemp<'ast> {
    pub fn new(ast: ast::Ast<'ast>) -> Self {
        Self {
            ast,
            scopes_temp: Vec::new(),
            const_exprs: Vec::new(),
            mods: Vec::new(),
            scopes: Vec::new(),
        }
    }

    pub fn modules(&self) -> impl Iterator<Item = &ast::Module<'ast>> {
        self.ast.modules.iter()
    }
}
