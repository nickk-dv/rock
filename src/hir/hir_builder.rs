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
    ctx: &'ctx CompCtx,
    ast: ast::Ast<'ast>,
    mods: Vec<ModData>,
    scopes: Vec<Scope<'ast>>,
    hir: hir::Hir<'hir>,
    ast_procs: Vec<&'ast ast::ProcDecl<'ast>>,
    ast_enums: Vec<&'ast ast::EnumDecl<'ast>>,
    ast_unions: Vec<&'ast ast::UnionDecl<'ast>>,
    ast_structs: Vec<&'ast ast::StructDecl<'ast>>,
    ast_consts: Vec<&'ast ast::ConstDecl<'ast>>,
    ast_globals: Vec<&'ast ast::GlobalDecl<'ast>>,
    ast_const_exprs: Vec<ast::ConstExpr<'ast>>,
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
pub const DUMMY_CONST_EXPR_ID: hir::ConstExprID = hir::ConstExprID(u32::MAX);

pub struct Scope<'ast> {
    parent: Option<ScopeID>,
    module: ast::Module<'ast>,
    symbols: HashMap<InternID, Symbol>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Symbol {
    Defined  { kind: SymbolKind, },
    Imported { kind: SymbolKind, use_range: TextRange },
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

//@revisit
//pub struct SymbolQuery<'ast> {
//    kind: SymbolKind<'ast>,
//    source: SourceRange,
//}

pub struct ScopeIter {
    curr: u32,
    len: u32,
}

impl<'ctx, 'ast, 'hir> HirBuilder<'ctx, 'ast, 'hir> {
    pub fn new(ctx: &'ctx CompCtx, ast: ast::Ast<'ast>) -> HirBuilder<'ctx, 'ast, 'hir> {
        HirBuilder {
            ctx,
            ast,
            mods: Vec::new(),
            scopes: Vec::new(),
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
            ast_procs: Vec::new(),
            ast_enums: Vec::new(),
            ast_unions: Vec::new(),
            ast_structs: Vec::new(),
            ast_consts: Vec::new(),
            ast_globals: Vec::new(),
            ast_const_exprs: Vec::new(),
        }
    }

    pub fn finish(self) -> hir::Hir<'hir> {
        self.hir
    }

    pub fn ctx(&self) -> &'ctx CompCtx {
        self.ctx
    }

    pub fn name_str(&self, id: InternID) -> &str {
        self.ctx.intern().get_str(id)
    }

    pub fn arena(&mut self) -> &mut Arena<'hir> {
        &mut self.hir.arena
    }

    pub fn ast_modules(&self) -> impl Iterator<Item = &ast::Module<'ast>> {
        self.ast.modules.iter()
    }

    pub fn add_mod(&mut self, data: ModData) -> (Symbol, ModID) {
        let id = ModID(self.mods.len() as u32);
        self.mods.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Mod(id),
        };
        (symbol, id)
    }

    pub fn add_proc(
        &mut self,
        decl: &'ast ast::ProcDecl<'ast>,
        data: hir::ProcData<'hir>,
    ) -> Symbol {
        let id = hir::ProcID(self.ast_procs.len() as u32);
        self.ast_procs.push(decl);
        self.hir.procs.push(data);
        Symbol::Defined {
            kind: SymbolKind::Proc(id),
        }
    }

    pub fn add_enum(
        &mut self,
        decl: &'ast ast::EnumDecl<'ast>,
        data: hir::EnumData<'hir>,
    ) -> Symbol {
        let id = hir::EnumID(self.ast_enums.len() as u32);
        self.ast_enums.push(decl);
        self.hir.enums.push(data);
        Symbol::Defined {
            kind: SymbolKind::Enum(id),
        }
    }

    pub fn add_union(
        &mut self,
        decl: &'ast ast::UnionDecl<'ast>,
        data: hir::UnionData<'hir>,
    ) -> Symbol {
        let id = hir::UnionID(self.ast_unions.len() as u32);
        self.ast_unions.push(decl);
        self.hir.unions.push(data);
        Symbol::Defined {
            kind: SymbolKind::Union(id),
        }
    }

    pub fn add_struct(
        &mut self,
        decl: &'ast ast::StructDecl<'ast>,
        data: hir::StructData<'hir>,
    ) -> Symbol {
        let id = hir::StructID(self.ast_structs.len() as u32);
        self.ast_structs.push(decl);
        self.hir.structs.push(data);
        Symbol::Defined {
            kind: SymbolKind::Struct(id),
        }
    }

    pub fn add_const(
        &mut self,
        decl: &'ast ast::ConstDecl<'ast>,
        data: hir::ConstData<'hir>,
    ) -> Symbol {
        let id = hir::ConstID(self.ast_consts.len() as u32);
        self.ast_consts.push(decl);
        self.hir.consts.push(data);
        Symbol::Defined {
            kind: SymbolKind::Const(id),
        }
    }

    pub fn add_global(
        &mut self,
        decl: &'ast ast::GlobalDecl<'ast>,
        data: hir::GlobalData<'hir>,
    ) -> Symbol {
        let id = hir::GlobalID(self.ast_globals.len() as u32);
        self.ast_globals.push(decl);
        self.hir.globals.push(data);
        Symbol::Defined {
            kind: SymbolKind::Global(id),
        }
    }

    pub fn scope_ids(&self) -> ScopeIter {
        ScopeIter {
            curr: 0,
            len: self.scopes.len() as u32,
        }
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

    pub fn symbol_range(&self, symbol: Symbol) -> TextRange {
        match symbol {
            Symbol::Defined { kind } => self.symbol_kind_range(kind),
            Symbol::Imported { use_range, .. } => use_range,
        }
    }

    fn symbol_kind_range(&self, kind: SymbolKind) -> TextRange {
        match kind {
            SymbolKind::Mod(id) => self.get_mod(id).name.range,
            SymbolKind::Proc(id) => self.hir.get_proc(id).name.range,
            SymbolKind::Enum(id) => self.hir.get_enum(id).name.range,
            SymbolKind::Union(id) => self.hir.get_union(id).name.range,
            SymbolKind::Struct(id) => self.hir.get_struct(id).name.range,
            SymbolKind::Const(id) => self.hir.get_const(id).name.range,
            SymbolKind::Global(id) => self.hir.get_global(id).name.range,
        }
    }

    //@revisit the concept of single query system
    // (need to specify if its in path) to not allow access to imports
    /*
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
    */
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

    pub fn file_id(&self) -> crate::ast::FileID {
        self.module.file_id
    }

    pub fn ast_decls(&self) -> impl Iterator<Item = ast::Decl<'ast>> {
        self.module.decls.into_iter()
    }

    pub fn add_symbol(&mut self, id: InternID, symbol: Symbol) {
        assert!(self.get_symbol(id).is_none());
        self.symbols.insert(id, symbol);
    }

    pub fn get_symbol(&self, id: InternID) -> Option<Symbol> {
        self.symbols.get(&id).cloned()
    }

    pub fn source(&self, range: TextRange) -> SourceRange {
        SourceRange::new(range, self.file_id())
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
