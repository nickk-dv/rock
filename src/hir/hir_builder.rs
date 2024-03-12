use crate::ast::ast;
use crate::ast::intern::InternID;
use crate::ast::CompCtx;
use crate::err::error_new::ErrorComp;
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

//@ 3/12/24
//@its possible to threat scopes / modules the same way
// the other declarations are used
// we can have some hir data for module (like file_id which are needed later on)
// and hir_builder would store scope information like Symbols + Parent + ast::Module
// this would make Module and Scope relation much simpler and coherent with the way other items are used

pub struct HirBuilder<'ctx, 'ast, 'hir> {
    ctx: &'ctx CompCtx,
    ast: ast::Ast<'ast>,
    mods: Vec<ModData>,
    scopes: Vec<Scope<'ast>>,
    errors: Vec<ErrorComp>,
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

impl<'ctx, 'ast, 'hir> HirBuilder<'ctx, 'ast, 'hir> {
    pub fn new(ctx: &'ctx CompCtx, ast: ast::Ast<'ast>) -> HirBuilder<'ctx, 'ast, 'hir> {
        HirBuilder {
            ctx,
            ast,
            mods: Vec::new(),
            scopes: Vec::new(),
            errors: Vec::new(),
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

    pub fn finish(self) -> Result<hir::Hir<'hir>, Vec<ErrorComp>> {
        if self.errors.is_empty() {
            Ok(self.hir)
        } else {
            Err(self.errors)
        }
    }

    pub fn ctx(&self) -> &'ctx CompCtx {
        self.ctx
    }
    pub fn name_str(&self, id: InternID) -> &str {
        self.ctx.intern().get_str(id)
    }
    pub fn ast_modules(&self) -> impl Iterator<Item = &ast::Module<'ast>> {
        self.ast.modules.iter()
    }
    pub fn error(&mut self, error: ErrorComp) {
        self.errors.push(error);
    }
    pub fn arena(&mut self) -> &mut Arena<'hir> {
        &mut self.hir.arena
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID> {
        (0..self.hir.procs.len()).map(|it| hir::ProcID(it as u32))
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID> {
        (0..self.hir.enums.len()).map(|it| hir::EnumID(it as u32))
    }
    pub fn union_ids(&self) -> impl Iterator<Item = hir::UnionID> {
        (0..self.hir.unions.len()).map(|it| hir::UnionID(it as u32))
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID> {
        (0..self.hir.structs.len()).map(|it| hir::StructID(it as u32))
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID> {
        (0..self.hir.consts.len()).map(|it| hir::ConstID(it as u32))
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID> {
        (0..self.hir.globals.len()).map(|it| hir::GlobalID(it as u32))
    }

    pub fn proc_ast(&self, id: hir::ProcID) -> &'ast ast::ProcDecl<'ast> {
        self.ast_procs.get(id.0 as usize).unwrap()
    }
    pub fn enum_ast(&self, id: hir::EnumID) -> &'ast ast::EnumDecl<'ast> {
        self.ast_enums.get(id.0 as usize).unwrap()
    }
    pub fn union_ast(&self, id: hir::UnionID) -> &'ast ast::UnionDecl<'ast> {
        self.ast_unions.get(id.0 as usize).unwrap()
    }
    pub fn struct_ast(&self, id: hir::StructID) -> &'ast ast::StructDecl<'ast> {
        self.ast_structs.get(id.0 as usize).unwrap()
    }
    pub fn const_ast(&self, id: hir::ConstID) -> &'ast ast::ConstDecl<'ast> {
        self.ast_consts.get(id.0 as usize).unwrap()
    }
    pub fn global_ast(&self, id: hir::GlobalID) -> &'ast ast::GlobalDecl<'ast> {
        self.ast_globals.get(id.0 as usize).unwrap()
    }
    pub fn const_expr_ast(&self, id: hir::ConstExprID) -> &'ast ast::Expr<'ast> {
        self.ast_const_exprs.get(id.0 as usize).unwrap().0
    }

    pub fn proc_data(&self, id: hir::ProcID) -> &hir::ProcData<'hir> {
        self.hir.procs.get(id.0 as usize).unwrap()
    }
    pub fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        self.hir.enums.get(id.0 as usize).unwrap()
    }
    pub fn union_data(&self, id: hir::UnionID) -> &hir::UnionData<'hir> {
        self.hir.unions.get(id.0 as usize).unwrap()
    }
    pub fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        self.hir.structs.get(id.0 as usize).unwrap()
    }
    pub fn const_data(&self, id: hir::ConstID) -> &hir::ConstData<'hir> {
        self.hir.consts.get(id.0 as usize).unwrap()
    }
    pub fn global_data(&self, id: hir::GlobalID) -> &hir::GlobalData<'hir> {
        self.hir.globals.get(id.0 as usize).unwrap()
    }
    pub fn const_expr_data(&self, id: hir::ConstExprID) -> &hir::ConstExprData<'hir> {
        self.hir.const_exprs.get(id.0 as usize).unwrap()
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID) -> &mut hir::ProcData<'hir> {
        self.hir.procs.get_mut(id.0 as usize).unwrap()
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID) -> &mut hir::EnumData<'hir> {
        self.hir.enums.get_mut(id.0 as usize).unwrap()
    }
    pub fn union_data_mut(&mut self, id: hir::UnionID) -> &mut hir::UnionData<'hir> {
        self.hir.unions.get_mut(id.0 as usize).unwrap()
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID) -> &mut hir::StructData<'hir> {
        self.hir.structs.get_mut(id.0 as usize).unwrap()
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID) -> &mut hir::ConstData<'hir> {
        self.hir.consts.get_mut(id.0 as usize).unwrap()
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID) -> &mut hir::GlobalData<'hir> {
        self.hir.globals.get_mut(id.0 as usize).unwrap()
    }
    pub fn const_expr_data_mut(&mut self, id: hir::ConstExprID) -> &mut hir::ConstExprData<'hir> {
        self.hir.const_exprs.get_mut(id.0 as usize).unwrap()
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

    pub fn add_const_expr(
        &mut self,
        from_id: ScopeID,
        const_expr: ast::ConstExpr<'ast>,
    ) -> hir::ConstExprID {
        let id = hir::ConstExprID(self.ast_const_exprs.len() as u32);
        self.ast_const_exprs.push(const_expr);
        self.hir.const_exprs.push(hir::ConstExprData {
            from_id,
            value: None,
        });
        id
    }

    pub fn add_mod(&mut self, data: ModData) -> (Symbol, ModID) {
        let id = ModID(self.mods.len() as u32);
        self.mods.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Mod(id),
        };
        (symbol, id)
    }

    pub fn scope_ids(&self) -> impl Iterator<Item = ScopeID> {
        (0..self.scopes.len()).map(|it| ScopeID(it as u32))
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
