use super::proc_scope::ProcScope;
use crate::ast;
use crate::config::TargetTriple;
use crate::error::{Error, ErrorSink, ErrorWarningBuffer, SourceRange, WarningBuffer};
use crate::errors as err;
use crate::hir;
use crate::intern::{InternLit, InternName, InternPool};
use crate::session::{ModuleID, Session};
use crate::support::{Arena, IndexID, ID};
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirCtx<'hir, 'ast, 's_ref> {
    pub arena: Arena<'hir>,
    pub emit: ErrorWarningBuffer,
    pub proc: ProcScope<'hir>,
    pub scope: HirScope<'hir>,
    pub registry: Registry<'hir, 'ast>,
    pub const_intern: hir::ConstInternPool<'hir>,
    pub target: TargetTriple,
    pub session: &'s_ref Session<'ast>,
}

pub struct HirScope<'hir> {
    modules: Vec<ModuleScope<'hir>>,
}

struct ModuleScope<'hir> {
    name_id: ID<InternName>,
    symbols: HashMap<ID<InternName>, Symbol<'hir>>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Symbol<'hir> {
    Defined(SymbolKind<'hir>),
    Imported { kind: SymbolKind<'hir>, import_range: TextRange },
}

#[derive(Copy, Clone)]
pub enum SymbolKind<'hir> {
    Module(ModuleID),
    Proc(hir::ProcID<'hir>),
    Enum(hir::EnumID<'hir>),
    Struct(hir::StructID<'hir>),
    Const(hir::ConstID<'hir>),
    Global(hir::GlobalID<'hir>),
}

pub struct Registry<'hir, 'ast> {
    ast_procs: Vec<&'ast ast::ProcItem<'ast>>,
    ast_enums: Vec<&'ast ast::EnumItem<'ast>>,
    ast_structs: Vec<&'ast ast::StructItem<'ast>>,
    ast_consts: Vec<&'ast ast::ConstItem<'ast>>,
    ast_globals: Vec<&'ast ast::GlobalItem<'ast>>,
    ast_imports: Vec<&'ast ast::ImportItem<'ast>>,
    hir_procs: Vec<hir::ProcData<'hir>>,
    hir_enums: Vec<hir::EnumData<'hir>>,
    hir_structs: Vec<hir::StructData<'hir>>,
    hir_consts: Vec<hir::ConstData<'hir>>,
    hir_globals: Vec<hir::GlobalData<'hir>>,
    hir_imports: Vec<hir::ImportData>,
    const_evals: Vec<(hir::ConstEval<'hir, 'ast>, ModuleID)>,
    variant_evals: Vec<hir::VariantEval<'hir>>,
}

impl<'hir, 'ast, 's_ref> HirCtx<'hir, 'ast, 's_ref> {
    pub fn new(session: &'s_ref Session<'ast>) -> HirCtx<'hir, 'ast, 's_ref> {
        let arena = Arena::new();
        let emit = ErrorWarningBuffer::default();
        let proc = ProcScope::dummy();
        let scope = HirScope::new(session);
        let registry = Registry::new(session);
        let const_intern = hir::ConstInternPool::new();

        //@store in Session instead?
        // build triples will be different from host
        HirCtx {
            arena,
            emit,
            proc,
            scope,
            registry,
            const_intern,
            target: TargetTriple::host(),
            session,
        }
    }

    #[inline]
    pub fn name_str(&self, id: ID<InternName>) -> &'ast str {
        self.session.intern_name.get(id)
    }
    #[inline]
    pub fn intern_lit(&self) -> &InternPool<'ast, InternLit> {
        &self.session.intern_lit
    }
    #[inline]
    pub fn intern_name(&mut self) -> &InternPool<'ast, InternName> {
        &self.session.intern_name
    }
    #[inline]
    pub fn ast_items(&self, module_id: ModuleID) -> &'ast [ast::Item<'ast>] {
        self.session.module.get(module_id).ast_expect().items
    }

    pub fn hir_emit(self) -> Result<(hir::Hir<'hir>, WarningBuffer), ErrorWarningBuffer> {
        let ((), warnings) = self.emit.result(())?;

        let mut emit = ErrorWarningBuffer::default();
        emit.join_w(warnings);

        let mut const_values = Vec::with_capacity(self.registry.const_evals.len());
        let mut variant_tag_values = Vec::with_capacity(self.registry.const_evals.len());

        for (eval, origin_id) in self.registry.const_evals.iter() {
            //@can just use `get_resolved` but for now emitting internal error
            // just rely on unreachable! in `get_resolved` when compiler is stable
            match *eval {
                hir::ConstEval::Unresolved(expr) => {
                    emit.error(Error::new(
                        "internal: trying to emit hir with ConstEval::Unresolved expression",
                        SourceRange::new(*origin_id, expr.0.range),
                        None,
                    ));
                }
                hir::ConstEval::ResolvedError => {
                    emit.error(Error::message(
                        "internal: trying to emit hir with ConstEval::ResolvedError expression",
                    ));
                }
                hir::ConstEval::Resolved(value_id) => const_values.push(value_id),
            }
        }

        for eval in self.registry.variant_evals.iter() {
            match *eval {
                hir::VariantEval::Unresolved(()) => {
                    unreachable!("silent VariantEval::Unresolved")
                }
                hir::VariantEval::ResolvedError => {
                    unreachable!("silent VariantEval::ResolvedError")
                }
                hir::VariantEval::Resolved(value) => variant_tag_values.push(value),
            }
        }

        let ((), warnings) = emit.result(())?;

        let hir = hir::Hir {
            arena: self.arena,
            const_intern: self.const_intern,
            procs: self.registry.hir_procs,
            enums: self.registry.hir_enums,
            structs: self.registry.hir_structs,
            consts: self.registry.hir_consts,
            globals: self.registry.hir_globals,
            const_values,
            variant_tag_values,
        };
        Ok((hir, warnings))
    }
}

impl<'hir> HirScope<'hir> {
    pub fn new(session: &Session) -> HirScope<'hir> {
        let mut modules = Vec::with_capacity(session.module.ids().count());

        for module_id in session.module.ids() {
            let module = session.module.get(module_id);
            let ast = module.ast_expect();

            let mut symbol_count = 0;
            for item in ast.items {
                symbol_count += 1;
                if let ast::Item::Import(import) = item {
                    symbol_count += import.symbols.len();
                }
            }

            let symbols = HashMap::with_capacity(symbol_count);
            modules.push(ModuleScope {
                name_id: module.name(),
                symbols,
            });
        }

        HirScope { modules }
    }

    fn module(&self, module_id: ModuleID) -> &ModuleScope<'hir> {
        &self.modules[module_id.index()]
    }
    fn module_mut(&mut self, module_id: ModuleID) -> &mut ModuleScope<'hir> {
        &mut self.modules[module_id.index()]
    }

    pub fn add_symbol(&mut self, origin_id: ModuleID, id: ID<InternName>, symbol: Symbol<'hir>) {
        let origin = self.module_mut(origin_id);
        origin.symbols.insert(id, symbol);
    }

    pub fn already_defined_check(
        &self,
        registry: &Registry,
        origin_id: ModuleID,
        name: ast::Name,
    ) -> Result<(), ScopeError> {
        let origin = self.module(origin_id);
        let symbol = match origin.symbols.get(&name.id).cloned() {
            Some(symbol) => symbol,
            None => return Ok(()),
        };

        let existing = match symbol {
            Symbol::Defined(kind) => kind.src(registry),
            Symbol::Imported { import_range, .. } => SourceRange::new(origin_id, import_range),
        };
        Err(ScopeError::AlreadyDefined {
            origin_id,
            name,
            existing,
        })
    }

    pub fn symbol_from_scope(
        &self,
        registry: &Registry,
        origin_id: ModuleID,
        target_id: ModuleID,
        name: ast::Name,
    ) -> Result<SymbolKind<'hir>, ScopeError> {
        let target = self.module(target_id);

        match target.symbols.get(&name.id).copied() {
            Some(Symbol::Defined(kind)) => {
                let vis = if origin_id == target_id {
                    ast::Vis::Public
                } else {
                    kind.vis(registry)
                };

                return match vis {
                    ast::Vis::Public => Ok(kind),
                    ast::Vis::Private => Err(ScopeError::SymbolIsPrivate {
                        origin_id,
                        name,
                        defined_src: kind.src(registry),
                        symbol_kind: kind.kind_name(),
                    }),
                };
            }
            Some(Symbol::Imported { kind, .. }) => {
                if origin_id == target_id {
                    return Ok(kind);
                }
            }
            None => {}
        }

        let from_module = if origin_id != target_id {
            Some(target_id)
        } else {
            None
        };
        Err(ScopeError::SymbolNotFound {
            origin_id,
            name,
            from_module,
        })
    }
}

pub enum ScopeError {
    AlreadyDefined {
        origin_id: ModuleID,
        name: ast::Name,
        existing: SourceRange,
    },
    SymbolIsPrivate {
        origin_id: ModuleID,
        name: ast::Name,
        defined_src: SourceRange,
        symbol_kind: &'static str,
    },
    SymbolNotFound {
        origin_id: ModuleID,
        name: ast::Name,
        from_module: Option<ModuleID>,
    },
}

impl ScopeError {
    pub fn emit(self, ctx: &mut HirCtx) {
        match self {
            ScopeError::AlreadyDefined {
                origin_id,
                name,
                existing,
            } => {
                let name_src = SourceRange::new(origin_id, name.range);
                let name = ctx.name_str(name.id);
                err::scope_name_already_defined(&mut ctx.emit, name_src, existing, name);
            }
            ScopeError::SymbolIsPrivate {
                origin_id,
                name,
                defined_src,
                symbol_kind,
            } => {
                let name_src = SourceRange::new(origin_id, name.range);
                let name = ctx.name_str(name.id);
                err::scope_symbol_is_private(
                    &mut ctx.emit,
                    name_src,
                    defined_src,
                    symbol_kind,
                    name,
                )
            }
            ScopeError::SymbolNotFound {
                origin_id,
                name,
                from_module,
            } => {
                let name_src = SourceRange::new(origin_id, name.range);
                let name = ctx.name_str(name.id);
                let from_module = from_module.map(|id| {
                    let name_id = ctx.scope.module(id).name_id;
                    ctx.name_str(name_id)
                });
                err::scope_symbol_not_found(&mut ctx.emit, name_src, name, from_module);
            }
        }
    }
}

impl<'hir> SymbolKind<'hir> {
    pub fn kind_name(self) -> &'static str {
        match self {
            SymbolKind::Module(_) => "module",
            SymbolKind::Proc(_) => "procedure",
            SymbolKind::Enum(_) => "enum",
            SymbolKind::Struct(_) => "struct",
            SymbolKind::Const(_) => "constant",
            SymbolKind::Global(_) => "global",
        }
    }

    fn vis(self, registry: &Registry) -> ast::Vis {
        match self {
            SymbolKind::Module(..) => unreachable!(),
            SymbolKind::Proc(id) => registry.proc_data(id).vis,
            SymbolKind::Enum(id) => registry.enum_data(id).vis,
            SymbolKind::Struct(id) => registry.struct_data(id).vis,
            SymbolKind::Const(id) => registry.const_data(id).vis,
            SymbolKind::Global(id) => registry.global_data(id).vis,
        }
    }

    pub fn src(self, registry: &Registry) -> SourceRange {
        match self {
            SymbolKind::Module(..) => unreachable!(),
            SymbolKind::Proc(id) => registry.proc_data(id).src(),
            SymbolKind::Enum(id) => registry.enum_data(id).src(),
            SymbolKind::Struct(id) => registry.struct_data(id).src(),
            SymbolKind::Const(id) => registry.const_data(id).src(),
            SymbolKind::Global(id) => registry.global_data(id).src(),
        }
    }
}

impl<'hir, 'ast> Registry<'hir, 'ast> {
    pub fn new(session: &Session) -> Registry<'hir, 'ast> {
        let mut proc_count = 0;
        let mut enum_count = 0;
        let mut struct_count = 0;
        let mut const_count = 0;
        let mut global_count = 0;
        let mut import_count = 0;

        for module_id in session.module.ids() {
            let module = session.module.get(module_id);
            let ast = module.ast_expect();

            for item in ast.items {
                match item {
                    ast::Item::Proc(_) => proc_count += 1,
                    ast::Item::Enum(_) => enum_count += 1,
                    ast::Item::Struct(_) => struct_count += 1,
                    ast::Item::Const(_) => const_count += 1,
                    ast::Item::Global(_) => global_count += 1,
                    ast::Item::Import(_) => import_count += 1,
                }
            }
        }

        //@assumes enums have on average 4 explicit constants (depends on the project)
        let consteval_count = enum_count * 4 + const_count + global_count;

        Registry {
            ast_procs: Vec::with_capacity(proc_count),
            ast_enums: Vec::with_capacity(enum_count),
            ast_structs: Vec::with_capacity(struct_count),
            ast_consts: Vec::with_capacity(const_count),
            ast_globals: Vec::with_capacity(global_count),
            ast_imports: Vec::with_capacity(import_count),
            hir_procs: Vec::with_capacity(proc_count),
            hir_enums: Vec::with_capacity(enum_count),
            hir_structs: Vec::with_capacity(struct_count),
            hir_consts: Vec::with_capacity(const_count),
            hir_globals: Vec::with_capacity(global_count),
            hir_imports: Vec::with_capacity(import_count),
            const_evals: Vec::with_capacity(consteval_count),
            variant_evals: Vec::with_capacity(0), //@infer?
        }
    }

    pub fn add_proc(
        &mut self,
        item: &'ast ast::ProcItem<'ast>,
        data: hir::ProcData<'hir>,
    ) -> hir::ProcID<'hir> {
        let id = hir::ProcID::new(&self.hir_procs);
        self.ast_procs.push(item);
        self.hir_procs.push(data);
        id
    }

    pub fn add_enum(
        &mut self,
        item: &'ast ast::EnumItem<'ast>,
        data: hir::EnumData<'hir>,
    ) -> hir::EnumID<'hir> {
        let id = hir::EnumID::new(&self.hir_enums);
        self.ast_enums.push(item);
        self.hir_enums.push(data);
        id
    }

    pub fn add_struct(
        &mut self,
        item: &'ast ast::StructItem<'ast>,
        data: hir::StructData<'hir>,
    ) -> hir::StructID<'hir> {
        let id = hir::StructID::new(&self.hir_structs);
        self.ast_structs.push(item);
        self.hir_structs.push(data);
        id
    }

    pub fn add_const(
        &mut self,
        item: &'ast ast::ConstItem<'ast>,
        data: hir::ConstData<'hir>,
    ) -> hir::ConstID<'hir> {
        let id = hir::ConstID::new(&self.hir_consts);
        self.ast_consts.push(item);
        self.hir_consts.push(data);
        id
    }

    pub fn add_global(
        &mut self,
        item: &'ast ast::GlobalItem<'ast>,
        data: hir::GlobalData<'hir>,
    ) -> hir::GlobalID<'hir> {
        let id = hir::GlobalID::new(&self.hir_globals);
        self.ast_globals.push(item);
        self.hir_globals.push(data);
        id
    }

    pub fn add_import(
        &mut self,
        item: &'ast ast::ImportItem<'ast>,
        data: hir::ImportData,
    ) -> hir::ImportID {
        let id = hir::ImportID::new(&self.hir_imports);
        self.ast_imports.push(item);
        self.hir_imports.push(data);
        id
    }

    pub fn add_const_eval(
        &mut self,
        const_expr: ast::ConstExpr<'ast>,
        origin_id: ModuleID,
    ) -> hir::ConstEvalID {
        let id = hir::ConstEvalID::new_raw(self.const_evals.len());
        let eval = hir::ConstEval::Unresolved(const_expr);
        self.const_evals.push((eval, origin_id));
        id
    }

    pub fn add_variant_eval(&mut self) -> hir::VariantEvalID<'hir> {
        let id = hir::VariantEvalID::new(&self.variant_evals);
        let eval = hir::VariantEval::Unresolved(());
        self.variant_evals.push(eval);
        id
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID<'hir>> {
        (0..self.hir_procs.len()).map(hir::ProcID::new_raw)
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID<'hir>> {
        (0..self.hir_enums.len()).map(hir::EnumID::new_raw)
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID<'hir>> {
        (0..self.hir_structs.len()).map(hir::StructID::new_raw)
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID<'hir>> {
        (0..self.hir_consts.len()).map(hir::ConstID::new_raw)
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID<'hir>> {
        (0..self.hir_globals.len()).map(hir::GlobalID::new_raw)
    }
    pub fn import_ids(&self) -> impl Iterator<Item = hir::ImportID> {
        (0..self.hir_imports.len()).map(hir::ImportID::new_raw)
    }
    pub fn const_eval_ids(&self) -> impl Iterator<Item = hir::ConstEvalID> {
        (0..self.const_evals.len()).map(hir::ConstEvalID::new_raw)
    }

    pub fn proc_item(&self, id: hir::ProcID<'hir>) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.raw_index()]
    }
    pub fn enum_item(&self, id: hir::EnumID) -> &'ast ast::EnumItem<'ast> {
        self.ast_enums[id.raw_index()]
    }
    pub fn struct_item(&self, id: hir::StructID) -> &'ast ast::StructItem<'ast> {
        self.ast_structs[id.raw_index()]
    }
    pub fn const_item(&self, id: hir::ConstID) -> &'ast ast::ConstItem<'ast> {
        self.ast_consts[id.raw_index()]
    }
    pub fn global_item(&self, id: hir::GlobalID) -> &'ast ast::GlobalItem<'ast> {
        self.ast_globals[id.raw_index()]
    }
    pub fn import_item(&self, id: hir::ImportID) -> &'ast ast::ImportItem<'ast> {
        self.ast_imports[id.raw_index()]
    }

    pub fn proc_data(&self, id: hir::ProcID<'hir>) -> &hir::ProcData<'hir> {
        self.hir_procs.id_get(id)
    }
    pub fn enum_data(&self, id: hir::EnumID<'hir>) -> &hir::EnumData<'hir> {
        self.hir_enums.id_get(id)
    }
    pub fn struct_data(&self, id: hir::StructID<'hir>) -> &hir::StructData<'hir> {
        self.hir_structs.id_get(id)
    }
    pub fn const_data(&self, id: hir::ConstID<'hir>) -> &hir::ConstData<'hir> {
        self.hir_consts.id_get(id)
    }
    pub fn global_data(&self, id: hir::GlobalID<'hir>) -> &hir::GlobalData<'hir> {
        self.hir_globals.id_get(id)
    }
    pub fn import_data(&self, id: hir::ImportID) -> &hir::ImportData {
        self.hir_imports.id_get(id)
    }
    pub fn const_eval(&self, id: hir::ConstEvalID) -> &(hir::ConstEval<'hir, 'ast>, ModuleID) {
        &self.const_evals[id.raw_index()]
    }
    pub fn variant_eval(&self, id: hir::VariantEvalID<'hir>) -> &hir::VariantEval<'hir> {
        self.variant_evals.id_get(id)
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID<'hir>) -> &mut hir::ProcData<'hir> {
        self.hir_procs.id_get_mut(id)
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID<'hir>) -> &mut hir::EnumData<'hir> {
        self.hir_enums.id_get_mut(id)
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID<'hir>) -> &mut hir::StructData<'hir> {
        self.hir_structs.id_get_mut(id)
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID<'hir>) -> &mut hir::ConstData<'hir> {
        self.hir_consts.id_get_mut(id)
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID<'hir>) -> &mut hir::GlobalData<'hir> {
        self.hir_globals.id_get_mut(id)
    }
    pub fn const_eval_mut(
        &mut self,
        id: hir::ConstEvalID,
    ) -> &mut (hir::ConstEval<'hir, 'ast>, ModuleID) {
        &mut self.const_evals[id.raw_index()]
    }
    pub fn variant_eval_mut(
        &mut self,
        id: hir::VariantEvalID<'hir>,
    ) -> &mut hir::VariantEval<'hir> {
        self.variant_evals.id_get_mut(id)
    }
}

//@move this?
impl hir::ArrayStaticLen {
    pub fn get_resolved(self, ctx: &HirCtx) -> Result<u64, ()> {
        match self {
            hir::ArrayStaticLen::Immediate(len) => Ok(len),
            hir::ArrayStaticLen::ConstEval(eval_id) => {
                let (eval, _) = *ctx.registry.const_eval(eval_id);
                let value_id = eval.get_resolved()?;

                match ctx.const_intern.get(value_id) {
                    hir::ConstValue::Int { val, .. } => Ok(val),
                    _ => unreachable!(),
                }
            }
        }
    }
}
