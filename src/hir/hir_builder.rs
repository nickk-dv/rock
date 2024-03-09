use crate::ast::ast;
use crate::ast::intern::InternID;
use crate::ast::CompCtx;
use crate::err::error_new::SourceRange;
use crate::hir;
use crate::mem::Arena;
use crate::text_range::TextRange;
use std::collections::HashMap;

// @Hir lowering design direction 07.03.24
// 1st pass would create:
// a single Scopes array:
// where each symbol holds the ID of the uniquely named symbol
// and its corresponding ast node.
// types and blocks would be set to Error / None by default

// 2nd importing pass would add references to symbols
// resolving all use declarations in each scope's ast module

// 3rd later pass would go though the data arrays and resolve:
// types & constant expressions of the declarations

// 4th pass would perform translation of Ast procedure blocks
// into hir form, and assign this top block to the ProcData
// this pass is isolated and only references already resolved declarations
// and its scope's namespace.

// Output: hir is similar to ast in structure
// but contains linear representation of package contents
// which is fully typechecked and name-resolved
// no errors would mean that its ready to be passed to LLVM-IR gen
// or any other low level IR backend

// - minor changes:
// use free functions for passes, with PassContext named `p` passed in (for read-ability)

pub struct HirBuilder<'ctx, 'ast, 'hir> {
    pub ctx: &'ctx CompCtx,
    ast: ast::Ast<'ast>,
    hir: hir::Hir<'hir>,
    mods: Vec<ModData>,
    scopes: Vec<Scope<'ast>>,
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

// @in a single package project ROOT_SCOPE_ID is always 0 since its the first module to be added
pub const ROOT_SCOPE_ID: ScopeID = ScopeID(0);

pub struct Scope<'ast> {
    parent: Option<ScopeID>,
    module: ast::Module<'ast>,
    symbols: HashMap<InternID, Symbol<'ast>>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Symbol<'ast> {
    Defined  { kind: SymbolKind<'ast> },
    Imported { kind: SymbolKind<'ast>, import: TextRange },
}

#[derive(Copy, Clone)]
pub enum SymbolKind<'ast> {
    Mod(ModID),
    Proc(hir::ProcID, &'ast ast::ProcDecl<'ast>),
    Enum(hir::EnumID, &'ast ast::EnumDecl<'ast>),
    Union(hir::UnionID, &'ast ast::UnionDecl<'ast>),
    Struct(hir::StructID, &'ast ast::StructDecl<'ast>),
    Const(hir::ConstID, &'ast ast::ConstDecl<'ast>),
    Global(hir::GlobalID, &'ast ast::GlobalDecl<'ast>),
}

pub struct SymbolQuery<'ast> {
    kind: SymbolKind<'ast>,
    source: SourceRange,
}

pub struct ConstExprTemp<'ast> {
    pub from_id: ScopeID,
    pub source: &'ast ast::Expr<'ast>,
}

pub struct ScopeIter {
    curr: u32,
    len: u32,
}

impl<'ctx, 'ast, 'hir> HirBuilder<'ctx, 'ast, 'hir> {
    pub fn new(ctx: &'ctx CompCtx, ast: ast::Ast<'ast>) -> HirBuilder<'ctx, 'ast, 'hir> {
        HirBuilder {
            ctx,
            ast,
            hir: hir::Hir {
                arena: Arena::new(),
                procs: Vec::new(),
                enums: Vec::new(),
                unions: Vec::new(),
                structs: Vec::new(),
                consts: Vec::new(),
                globals: Vec::new(),
                const_exprs: Vec::new(),
            },
            mods: Vec::new(),
            scopes: Vec::new(),
            const_exprs: Vec::new(),
        }
    }

    pub fn finish(self) -> hir::Hir<'hir> {
        self.hir
    }

    pub fn arena(&mut self) -> &mut Arena<'hir> {
        &mut self.hir.arena
    }

    pub fn add_proc(&mut self, data: hir::ProcData<'hir>) -> hir::ProcID {
        self.hir.procs.push(data);
        hir::ProcID((self.hir.procs.len() - 1) as u32)
    }
    pub fn add_enum(&mut self, data: hir::EnumData<'hir>) -> hir::EnumID {
        self.hir.enums.push(data);
        hir::EnumID((self.hir.enums.len() - 1) as u32)
    }
    pub fn add_union(&mut self, data: hir::UnionData<'hir>) -> hir::UnionID {
        self.hir.unions.push(data);
        hir::UnionID((self.hir.unions.len() - 1) as u32)
    }
    pub fn add_struct(&mut self, data: hir::StructData<'hir>) -> hir::StructID {
        self.hir.structs.push(data);
        hir::StructID((self.hir.structs.len() - 1) as u32)
    }
    pub fn add_const(&mut self, data: hir::ConstData<'hir>) -> hir::ConstID {
        self.hir.consts.push(data);
        hir::ConstID((self.hir.consts.len() - 1) as u32)
    }
    pub fn add_global(&mut self, data: hir::GlobalData<'hir>) -> hir::GlobalID {
        self.hir.globals.push(data);
        hir::GlobalID((self.hir.globals.len() - 1) as u32)
    }
    pub fn add_const_expr(&mut self, data: hir::ConstExpr<'hir>) -> hir::ConstExprID {
        self.hir.const_exprs.push(data);
        // @self const exprs not the hir const_exprs
        hir::ConstExprID((self.const_exprs.len() - 1) as u32)
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

    pub fn add_scope(&mut self, scope: Scope<'ast>) -> ScopeID {
        self.scopes.push(scope);
        ScopeID((self.scopes.len() - 1) as u32)
    }
    pub fn get_scope(&self, id: ScopeID) -> &Scope<'ast> {
        self.scopes.get(id.0 as usize).unwrap()
    }
    pub fn get_scope_mut(&mut self, id: ScopeID) -> &mut Scope<'ast> {
        self.scopes.get_mut(id.0 as usize).unwrap()
    }

    fn symbol_kind_range(&self, kind: SymbolKind<'ast>) -> TextRange {
        match kind {
            SymbolKind::Mod(id) => self.get_mod(id).name.range,
            SymbolKind::Proc(_, decl) => decl.name.range,
            SymbolKind::Enum(_, decl) => decl.name.range,
            SymbolKind::Union(_, decl) => decl.name.range,
            SymbolKind::Struct(_, decl) => decl.name.range,
            SymbolKind::Const(_, decl) => decl.name.range,
            SymbolKind::Global(_, decl) => decl.name.range,
        }
    }

    pub fn get_symbol(
        &self,
        id: InternID,
        scope_id: ScopeID,
        from_id: ScopeID,
    ) -> Option<SymbolQuery<'ast>> {
        // @doesnt work if scope_id is in path
        // in that case imported will be able to be accessed
        // allow it for now probably... not a big deal.
        let scope = self.get_scope(from_id);
        if scope_id.0 == from_id.0 {
            // can take defined and imported symbols
            match scope.symbols.get(&id).cloned() {
                Some(Symbol::Defined { kind }) => Some(SymbolQuery {
                    kind,
                    source: scope.source(self.symbol_kind_range(kind)),
                }),
                Some(Symbol::Imported { kind, import }) => Some(SymbolQuery {
                    kind,
                    source: scope.source(import),
                }),
                _ => None,
            }
        } else {
            match scope.symbols.get(&id).cloned() {
                Some(Symbol::Defined { kind }) => Some(SymbolQuery {
                    kind,
                    source: scope.source(self.symbol_kind_range(kind)),
                }),
                _ => None,
            }
        }
    }
}

impl<'ast> Scope<'ast> {
    pub fn new(parent: Option<ScopeID>, module: ast::Module<'ast>) -> Scope {
        Scope {
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

    pub fn add_symbol(&mut self, id: InternID, symbol: Symbol<'ast>) {
        assert!(self.get_symbol(id).is_none());
        self.symbols.insert(id, symbol);
    }

    pub fn get_symbol(&self, id: InternID) -> Option<Symbol<'ast>> {
        self.symbols.get(&id).cloned()
    }

    pub fn source(&self, range: TextRange) -> SourceRange {
        SourceRange::new(range, self.module_file_id())
    }

    pub fn get_defined_symbol_source<'ctx, 'hir>(
        &self,
        hb: &HirBuilder<'ctx, 'ast, 'hir>,
        symbol: Symbol<'ast>,
    ) -> SourceRange {
        let range = match symbol {
            Symbol::Defined { kind } => match kind {
                SymbolKind::Mod(id) => hb.get_mod(id).name.range,
                SymbolKind::Proc(.., decl) => decl.name.range,
                SymbolKind::Enum(.., decl) => decl.name.range,
                SymbolKind::Union(.., decl) => decl.name.range,
                SymbolKind::Struct(.., decl) => decl.name.range,
                SymbolKind::Const(.., decl) => decl.name.range,
                SymbolKind::Global(.., decl) => decl.name.range,
            },
            Symbol::Imported { import, .. } => import,
        };
        self.source(range)
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
