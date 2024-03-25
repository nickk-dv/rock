use crate::arena::Arena;
use crate::ast;
use crate::ast_parse::CompCtx;
use crate::error::{ErrorComp, SourceRange};
use crate::hir;
use crate::intern::InternID;
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirBuilder<'ctx, 'ast, 'hir> {
    ctx: &'ctx mut CompCtx,
    ast: ast::Ast<'ast>,
    mods: Vec<ModData>,
    scopes: Vec<Scope<'ast>>,
    errors: Vec<ErrorComp>,
    hir: hir::Hir<'hir>,
    ast_procs: Vec<&'ast ast::ProcItem<'ast>>,
    ast_enums: Vec<&'ast ast::EnumItem<'ast>>,
    ast_unions: Vec<&'ast ast::UnionItem<'ast>>,
    ast_structs: Vec<&'ast ast::StructItem<'ast>>,
    ast_consts: Vec<&'ast ast::ConstItem<'ast>>,
    ast_globals: Vec<&'ast ast::GlobalItem<'ast>>,
    ast_const_exprs: Vec<ast::ConstExpr<'ast>>,
}

#[derive(Copy, Clone)]
pub struct ModID(u32);
pub struct ModData {
    pub origin_id: hir::ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub target: Option<hir::ScopeID>,
}

pub const ROOT_SCOPE_ID: hir::ScopeID = hir::ScopeID::new(0);
pub const DUMMY_CONST_EXPR_ID: hir::ConstExprID = hir::ConstExprID::new(u32::MAX as usize);

pub struct Scope<'ast> {
    parent: Option<hir::ScopeID>,
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

impl<'ctx, 'ast, 'hir> HirBuilder<'ctx, 'ast, 'hir> {
    pub fn new(ctx: &'ctx mut CompCtx, ast: ast::Ast<'ast>) -> HirBuilder<'ctx, 'ast, 'hir> {
        HirBuilder {
            ctx,
            ast,
            mods: Vec::new(),
            scopes: Vec::new(),
            errors: Vec::new(),
            hir: hir::Hir {
                arena: Arena::new(),
                scopes: Vec::new(),
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

    pub fn ctx(&self) -> &CompCtx {
        self.ctx
    }
    pub fn ctx_mut(&mut self) -> &mut CompCtx {
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

    pub fn get_mod(&self, id: ModID) -> &ModData {
        self.mods.get(id.0 as usize).unwrap()
    }
    pub fn get_mod_mut(&mut self, id: ModID) -> &mut ModData {
        self.mods.get_mut(id.0 as usize).unwrap()
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID> {
        (0..self.hir.procs.len()).map(hir::ProcID::new)
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID> {
        (0..self.hir.enums.len()).map(hir::EnumID::new)
    }
    pub fn union_ids(&self) -> impl Iterator<Item = hir::UnionID> {
        (0..self.hir.unions.len()).map(hir::UnionID::new)
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID> {
        (0..self.hir.structs.len()).map(hir::StructID::new)
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID> {
        (0..self.hir.consts.len()).map(hir::ConstID::new)
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID> {
        (0..self.hir.globals.len()).map(hir::GlobalID::new)
    }

    pub fn proc_ast(&self, id: hir::ProcID) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.index()]
    }
    pub fn enum_ast(&self, id: hir::EnumID) -> &'ast ast::EnumItem<'ast> {
        self.ast_enums[id.index()]
    }
    pub fn union_ast(&self, id: hir::UnionID) -> &'ast ast::UnionItem<'ast> {
        self.ast_unions[id.index()]
    }
    pub fn struct_ast(&self, id: hir::StructID) -> &'ast ast::StructItem<'ast> {
        self.ast_structs[id.index()]
    }
    pub fn const_ast(&self, id: hir::ConstID) -> &'ast ast::ConstItem<'ast> {
        self.ast_consts[id.index()]
    }
    pub fn global_ast(&self, id: hir::GlobalID) -> &'ast ast::GlobalItem<'ast> {
        self.ast_globals[id.index()]
    }
    pub fn const_expr_ast(&self, id: hir::ConstExprID) -> &'ast ast::Expr<'ast> {
        self.ast_const_exprs[id.index()].0
    }

    pub fn proc_data(&self, id: hir::ProcID) -> &hir::ProcData<'hir> {
        &self.hir.procs[id.index()]
    }
    pub fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        &self.hir.enums[id.index()]
    }
    pub fn union_data(&self, id: hir::UnionID) -> &hir::UnionData<'hir> {
        &self.hir.unions[id.index()]
    }
    pub fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        &self.hir.structs[id.index()]
    }
    pub fn const_data(&self, id: hir::ConstID) -> &hir::ConstData<'hir> {
        &self.hir.consts[id.index()]
    }
    pub fn global_data(&self, id: hir::GlobalID) -> &hir::GlobalData<'hir> {
        &self.hir.globals[id.index()]
    }
    pub fn const_expr_data(&self, id: hir::ConstExprID) -> &hir::ConstExprData<'hir> {
        &self.hir.const_exprs[id.index()]
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID) -> &mut hir::ProcData<'hir> {
        self.hir.procs.get_mut(id.index()).unwrap()
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID) -> &mut hir::EnumData<'hir> {
        self.hir.enums.get_mut(id.index()).unwrap()
    }
    pub fn union_data_mut(&mut self, id: hir::UnionID) -> &mut hir::UnionData<'hir> {
        self.hir.unions.get_mut(id.index()).unwrap()
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID) -> &mut hir::StructData<'hir> {
        self.hir.structs.get_mut(id.index()).unwrap()
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID) -> &mut hir::ConstData<'hir> {
        self.hir.consts.get_mut(id.index()).unwrap()
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID) -> &mut hir::GlobalData<'hir> {
        self.hir.globals.get_mut(id.index()).unwrap()
    }
    pub fn const_expr_data_mut(&mut self, id: hir::ConstExprID) -> &mut hir::ConstExprData<'hir> {
        self.hir.const_exprs.get_mut(id.index()).unwrap()
    }

    pub fn add_mod(&mut self, origin_id: hir::ScopeID, data: ModData) -> ModID {
        let id = ModID(self.mods.len() as u32);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Mod(id),
        };
        self.scope_add_symbol(origin_id, data.name.id, symbol);
        self.mods.push(data);
        id
    }
    pub fn add_proc(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::ProcItem<'ast>,
        data: hir::ProcData<'hir>,
    ) {
        let id = hir::ProcID::new(self.ast_procs.len());
        self.ast_procs.push(item);
        self.hir.procs.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Proc(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_enum(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::EnumItem<'ast>,
        data: hir::EnumData<'hir>,
    ) {
        let id = hir::EnumID::new(self.ast_enums.len());
        self.ast_enums.push(item);
        self.hir.enums.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Enum(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_union(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::UnionItem<'ast>,
        data: hir::UnionData<'hir>,
    ) {
        let id = hir::UnionID::new(self.ast_unions.len());
        self.ast_unions.push(item);
        self.hir.unions.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Union(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_struct(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::StructItem<'ast>,
        data: hir::StructData<'hir>,
    ) {
        let id = hir::StructID::new(self.ast_structs.len());
        self.ast_structs.push(item);
        self.hir.structs.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Struct(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_const(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::ConstItem<'ast>,
        data: hir::ConstData<'hir>,
    ) {
        let id = hir::ConstID::new(self.ast_consts.len());
        self.ast_consts.push(item);
        self.hir.consts.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Const(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_global(
        &mut self,
        origin_id: hir::ScopeID,
        item: &'ast ast::GlobalItem<'ast>,
        data: hir::GlobalData<'hir>,
    ) {
        let id = hir::GlobalID::new(self.ast_globals.len());
        self.ast_globals.push(item);
        self.hir.globals.push(data);
        let symbol = Symbol::Defined {
            kind: SymbolKind::Global(id),
        };
        self.scope_add_symbol(origin_id, item.name.id, symbol);
    }
    pub fn add_const_expr(
        &mut self,
        origin_id: hir::ScopeID,
        const_expr: ast::ConstExpr<'ast>,
    ) -> hir::ConstExprID {
        let id = hir::ConstExprID::new(self.ast_const_exprs.len());
        self.ast_const_exprs.push(const_expr);
        self.hir.const_exprs.push(hir::ConstExprData {
            origin_id,
            value: None,
        });
        id
    }

    pub fn scope_ids(&self) -> impl Iterator<Item = hir::ScopeID> {
        (0..self.scopes.len()).map(hir::ScopeID::new)
    }

    pub fn src(&self, id: hir::ScopeID, range: TextRange) -> SourceRange {
        SourceRange::new(range, self.scope(id).module.file_id)
    }

    pub fn scope_add_imported(
        &mut self,
        origin_id: hir::ScopeID,
        use_name: ast::Ident,
        kind: SymbolKind,
    ) {
        self.scope_add_symbol(
            origin_id,
            use_name.id,
            Symbol::Imported {
                kind,
                use_range: use_name.range,
            },
        );
    }

    pub fn scope_file_path(&self, id: hir::ScopeID) -> std::path::PathBuf {
        self.ctx
            .vfs
            .file(self.scope(id).module.file_id)
            .path
            .clone()
    }

    pub fn scope_name_defined(&self, origin_id: hir::ScopeID, id: InternID) -> Option<SourceRange> {
        let origin = self.scope(origin_id);
        if let Some(symbol) = origin.symbols.get(&id).cloned() {
            let file_id = origin.module.file_id;
            match symbol {
                Symbol::Defined { kind } => {
                    Some(SourceRange::new(self.symbol_kind_range(kind), file_id))
                }
                Symbol::Imported { use_range, .. } => Some(SourceRange::new(use_range, file_id)),
            }
        } else {
            None
        }
    }

    pub fn scope_parent(&self, id: hir::ScopeID) -> Option<hir::ScopeID> {
        self.scope(id).parent
    }

    pub fn scope_ast_items(&self, id: hir::ScopeID) -> impl Iterator<Item = ast::Item<'ast>> {
        self.scope(id).module.items.iter().cloned()
    }

    pub fn add_scope(
        &mut self,
        parent: Option<hir::ScopeID>,
        module: ast::Module<'ast>,
    ) -> hir::ScopeID {
        let id = hir::ScopeID::new(self.scopes.len());
        let scope = Scope {
            parent,
            module,
            symbols: HashMap::new(),
        };
        self.scopes.push(scope);
        self.hir.scopes.push(hir::ScopeData {
            file_id: module.file_id,
        });
        id
    }

    pub fn symbol_from_scope(
        &self,
        origin_id: hir::ScopeID,
        target_id: hir::ScopeID,
        path_kind: ast::PathKind,
        id: InternID,
    ) -> Option<(SymbolKind, SourceRange)> {
        let target = self.scope(target_id);
        match target.symbols.get(&id).cloned() {
            Some(symbol) => match symbol {
                Symbol::Defined { kind } => {
                    let source =
                        SourceRange::new(self.symbol_kind_range(kind), target.module.file_id);
                    Some((kind, source))
                }
                Symbol::Imported { kind, use_range } => {
                    let allow_use = path_kind == ast::PathKind::None && (origin_id == target_id);
                    if allow_use {
                        let source = SourceRange::new(use_range, target.module.file_id);
                        Some((kind, source))
                    } else {
                        None
                    }
                }
            },
            None => None,
        }
    }

    fn scope(&self, id: hir::ScopeID) -> &Scope<'ast> {
        &self.scopes[id.index()]
    }

    fn scope_add_symbol(&mut self, origin_id: hir::ScopeID, id: InternID, symbol: Symbol) {
        let scope = &mut self.scopes[origin_id.index()];
        scope.symbols.insert(id, symbol);
    }

    fn symbol_kind_range(&self, kind: SymbolKind) -> TextRange {
        match kind {
            SymbolKind::Mod(id) => self.get_mod(id).name.range,
            SymbolKind::Proc(id) => self.proc_data(id).name.range,
            SymbolKind::Enum(id) => self.enum_data(id).name.range,
            SymbolKind::Union(id) => self.union_data(id).name.range,
            SymbolKind::Struct(id) => self.struct_data(id).name.range,
            SymbolKind::Const(id) => self.const_data(id).name.range,
            SymbolKind::Global(id) => self.global_data(id).name.range,
        }
    }
}
