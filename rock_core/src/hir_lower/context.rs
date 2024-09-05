use super::proc_scope::ProcScope;
use crate::arena::Arena;
use crate::ast;
use crate::config::TargetTriple;
use crate::error::{DiagnosticCollection, ErrorComp, ErrorSink, Info, ResultComp, SourceRange};
use crate::hir;
use crate::intern::{InternLit, InternName, InternPool};
use crate::macros::{IndexID, ID};
use crate::session::ModuleID;
use crate::text::TextRange;
use std::collections::HashMap;

pub struct HirCtx<'hir, 'ast> {
    pub ast: ast::Ast<'ast>,
    pub arena: Arena<'hir>,
    pub emit: HirEmit,
    pub proc: ProcScope<'hir>,
    pub scope: HirScope<'hir>,
    pub registry: Registry<'hir, 'ast>,
    pub const_intern: hir::ConstInternPool<'hir>,
    pub target: TargetTriple,
}

pub struct HirEmit {
    diagnostics: DiagnosticCollection,
}

pub struct HirScope<'hir> {
    modules: Vec<Module<'hir>>,
}

struct Module<'hir> {
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
    Enum(hir::EnumID),
    Struct(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
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
    const_evals: Vec<(hir::ConstEval<'ast>, ModuleID)>,
    variant_evals: Vec<hir::VariantEval<'hir>>,
}

impl<'hir, 'ast: 'hir> HirCtx<'hir, 'ast> {
    pub fn new(ast: ast::Ast<'ast>) -> HirCtx<'hir, 'ast> {
        let arena = Arena::new();
        let emit = HirEmit::new();
        let proc = ProcScope::dummy();
        let scope = HirScope::new(&ast);
        let registry = Registry::new(&ast);
        let const_intern = hir::ConstInternPool::new();

        //@store in Session instead?
        // build triples will be different from host
        HirCtx {
            ast,
            arena,
            emit,
            proc,
            scope,
            registry,
            const_intern,
            target: TargetTriple::host(),
        }
    }

    #[inline]
    pub fn name_str(&self, id: ID<InternName>) -> &'ast str {
        self.ast.intern_name.get(id)
    }
    #[inline]
    pub fn intern_lit(&self) -> &InternPool<'ast, InternLit> {
        &self.ast.intern_lit
    }
    #[inline]
    pub fn intern_name(&mut self) -> &mut InternPool<'ast, InternName> {
        &mut self.ast.intern_name
    }
    #[inline]
    pub fn ast_module(&self, module_id: ModuleID) -> ast::Module<'ast> {
        self.ast.modules[module_id.index()]
    }

    pub fn hir_emit(self) -> ResultComp<hir::Hir<'hir>> {
        if !self.emit.diagnostics.errors().is_empty() {
            return ResultComp::Err(self.emit.diagnostics);
        }

        let mut const_values = Vec::with_capacity(self.registry.const_evals.len());
        let mut errors = Vec::new();

        for (eval, origin_id) in self.registry.const_evals.iter() {
            //@can just use `get_resolved` but for now emitting internal error
            // just rely on unreachable! in `get_resolve` when compiler is stable
            match *eval {
                hir::ConstEval::Unresolved(expr) => {
                    errors.push(ErrorComp::new(
                        "internal: trying to emit hir with ConstEval::Unresolved expression",
                        SourceRange::new(*origin_id, expr.0.range),
                        None,
                    ));
                }
                hir::ConstEval::ResolvedError => {
                    errors.push(ErrorComp::message(
                        "internal: trying to emit hir with ConstEval::ResolvedError expression",
                    ));
                }
                hir::ConstEval::Resolved(value_id) => const_values.push(value_id),
            }
        }

        if errors.is_empty() {
            let hir = hir::Hir {
                arena: self.arena,
                string_is_cstr: self.ast.string_is_cstr,
                intern_lit: self.ast.intern_lit,
                intern_name: self.ast.intern_name,
                const_intern: self.const_intern,
                procs: self.registry.hir_procs,
                enums: self.registry.hir_enums,
                structs: self.registry.hir_structs,
                consts: self.registry.hir_consts,
                globals: self.registry.hir_globals,
                const_values,
            };
            ResultComp::Ok((hir, self.emit.diagnostics.warnings_moveout()))
        } else {
            ResultComp::Err(self.emit.diagnostics.join_errors(errors))
        }
    }
}

impl ErrorSink for HirEmit {
    fn diagnostics(&self) -> &DiagnosticCollection {
        &self.diagnostics
    }
    fn diagnostics_mut(&mut self) -> &mut DiagnosticCollection {
        &mut self.diagnostics
    }
}

impl HirEmit {
    fn new() -> HirEmit {
        HirEmit {
            diagnostics: DiagnosticCollection::new(),
        }
    }
}

impl<'hir> HirScope<'hir> {
    pub fn new(ast: &ast::Ast) -> HirScope<'hir> {
        let mut modules = Vec::with_capacity(ast.modules.len());

        for module in ast.modules.iter() {
            let mut symbol_count = 0;
            for item in module.items {
                symbol_count += 1;
                if let ast::Item::Import(import) = item {
                    symbol_count += import.symbols.len();
                }
            }
            let symbols = HashMap::with_capacity(symbol_count);
            modules.push(Module { symbols });
        }

        HirScope { modules }
    }

    fn module(&self, id: ModuleID) -> &Module<'hir> {
        &self.modules[id.index()]
    }
    fn module_mut(&mut self, id: ModuleID) -> &mut Module<'hir> {
        &mut self.modules[id.index()]
    }

    pub fn add_symbol(&mut self, origin_id: ModuleID, id: ID<InternName>, symbol: Symbol<'hir>) {
        let origin = self.module_mut(origin_id);
        origin.symbols.insert(id, symbol);
    }

    pub fn symbol_defined(
        &self,
        origin_id: ModuleID,
        id: ID<InternName>,
    ) -> Option<SymbolKind<'hir>> {
        let origin = self.module(origin_id);

        match origin.symbols.get(&id).cloned() {
            Some(Symbol::Defined(kind)) => Some(kind),
            _ => None,
        }
    }

    pub fn symbol_defined_src(
        &self,
        registry: &Registry,
        origin_id: ModuleID,
        id: ID<InternName>,
    ) -> Option<SourceRange> {
        let origin = self.module(origin_id);
        let symbol = origin.symbols.get(&id).cloned()?;

        match symbol {
            Symbol::Defined(kind) => Some(SourceRange::new(origin_id, kind.name_range(registry))),
            Symbol::Imported { import_range, .. } => {
                Some(SourceRange::new(origin_id, import_range))
            }
        }
    }

    pub fn symbol_from_scope(
        &self,
        ctx: &HirCtx,
        registry: &Registry,
        origin_id: ModuleID,
        target_id: ModuleID,
        name: ast::Name,
    ) -> Result<(SymbolKind<'hir>, SourceRange), ErrorComp> {
        let target = self.module(target_id);

        match target.symbols.get(&name.id).copied() {
            Some(Symbol::Defined(kind)) => {
                let source = SourceRange::new(target_id, kind.name_range(registry));

                let vis = if origin_id == target_id {
                    ast::Vis::Public
                } else {
                    kind.vis(registry)
                };

                return match vis {
                    ast::Vis::Public => Ok((kind, source)),
                    ast::Vis::Private => Err(ErrorComp::new(
                        format!(
                            "{} `{}` is private",
                            kind.kind_name(),
                            ctx.name_str(name.id)
                        ),
                        SourceRange::new(origin_id, name.range),
                        Info::new("defined here", source),
                    )),
                };
            }
            Some(Symbol::Imported { kind, import_range }) => {
                let source = SourceRange::new(target_id, import_range);

                if origin_id == target_id {
                    return Ok((kind, source));
                }
            }
            None => {}
        }

        Err(ErrorComp::new(
            format!("name `{}` is not found in module", ctx.name_str(name.id)),
            SourceRange::new(origin_id, name.range),
            None,
        ))
    }
}

impl<'hir> SymbolKind<'hir> {
    pub const fn kind_name(self) -> &'static str {
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

    fn name_range(self, registry: &Registry) -> TextRange {
        match self {
            SymbolKind::Module(..) => unreachable!(),
            SymbolKind::Proc(id) => registry.proc_data(id).name.range,
            SymbolKind::Enum(id) => registry.enum_data(id).name.range,
            SymbolKind::Struct(id) => registry.struct_data(id).name.range,
            SymbolKind::Const(id) => registry.const_data(id).name.range,
            SymbolKind::Global(id) => registry.global_data(id).name.range,
        }
    }
}

impl<'hir, 'ast> Registry<'hir, 'ast> {
    pub fn new(ast: &ast::Ast<'ast>) -> Registry<'hir, 'ast> {
        let mut proc_count = 0;
        let mut enum_count = 0;
        let mut struct_count = 0;
        let mut const_count = 0;
        let mut global_count = 0;
        let mut import_count = 0;

        for module in ast.modules.iter() {
            for item in module.items {
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
    ) -> hir::EnumID {
        let id = hir::EnumID::new(self.hir_enums.len());
        self.ast_enums.push(item);
        self.hir_enums.push(data);
        id
    }

    pub fn add_struct(
        &mut self,
        item: &'ast ast::StructItem<'ast>,
        data: hir::StructData<'hir>,
    ) -> hir::StructID {
        let id = hir::StructID::new(self.hir_structs.len());
        self.ast_structs.push(item);
        self.hir_structs.push(data);
        id
    }

    pub fn add_const(
        &mut self,
        item: &'ast ast::ConstItem<'ast>,
        data: hir::ConstData<'hir>,
    ) -> hir::ConstID {
        let id = hir::ConstID::new(self.hir_consts.len());
        self.ast_consts.push(item);
        self.hir_consts.push(data);
        id
    }

    pub fn add_global(
        &mut self,
        item: &'ast ast::GlobalItem<'ast>,
        data: hir::GlobalData<'hir>,
    ) -> hir::GlobalID {
        let id = hir::GlobalID::new(self.hir_globals.len());
        self.ast_globals.push(item);
        self.hir_globals.push(data);
        id
    }

    pub fn add_import(
        &mut self,
        item: &'ast ast::ImportItem<'ast>,
        data: hir::ImportData,
    ) -> hir::ImportID {
        let id = hir::ImportID::new(self.hir_imports.len());
        self.ast_imports.push(item);
        self.hir_imports.push(data);
        id
    }

    pub fn add_const_eval(
        &mut self,
        const_expr: ast::ConstExpr<'ast>,
        origin_id: ModuleID,
    ) -> hir::ConstEvalID {
        let id = hir::ConstEvalID::new(self.const_evals.len());
        let eval = hir::ConstEval::Unresolved(const_expr);
        self.const_evals.push((eval, origin_id));
        id
    }

    pub fn add_variant_eval(&mut self) -> hir::VariantEvalID {
        let id = hir::VariantEvalID::new(self.variant_evals.len());
        let eval = hir::VariantEval::Unresolved(());
        self.variant_evals.push(eval);
        id
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID<'hir>> {
        (0..self.hir_procs.len()).map(hir::ProcID::new_raw)
    }
    pub fn enum_ids(&self) -> impl Iterator<Item = hir::EnumID> {
        (0..self.hir_enums.len()).map(hir::EnumID::new)
    }
    pub fn struct_ids(&self) -> impl Iterator<Item = hir::StructID> {
        (0..self.hir_structs.len()).map(hir::StructID::new)
    }
    pub fn const_ids(&self) -> impl Iterator<Item = hir::ConstID> {
        (0..self.hir_consts.len()).map(hir::ConstID::new)
    }
    pub fn global_ids(&self) -> impl Iterator<Item = hir::GlobalID> {
        (0..self.hir_globals.len()).map(hir::GlobalID::new)
    }
    pub fn import_ids(&self) -> impl Iterator<Item = hir::ImportID> {
        (0..self.hir_imports.len()).map(hir::ImportID::new)
    }
    pub fn const_eval_ids(&self) -> impl Iterator<Item = hir::ConstEvalID> {
        (0..self.const_evals.len()).map(hir::ConstEvalID::new)
    }

    pub fn proc_item(&self, id: hir::ProcID<'hir>) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.raw_index()]
    }
    pub fn enum_item(&self, id: hir::EnumID) -> &'ast ast::EnumItem<'ast> {
        self.ast_enums[id.index()]
    }
    pub fn struct_item(&self, id: hir::StructID) -> &'ast ast::StructItem<'ast> {
        self.ast_structs[id.index()]
    }
    pub fn const_item(&self, id: hir::ConstID) -> &'ast ast::ConstItem<'ast> {
        self.ast_consts[id.index()]
    }
    pub fn global_item(&self, id: hir::GlobalID) -> &'ast ast::GlobalItem<'ast> {
        self.ast_globals[id.index()]
    }
    pub fn import_item(&self, id: hir::ImportID) -> &'ast ast::ImportItem<'ast> {
        self.ast_imports[id.index()]
    }

    pub fn proc_data(&self, id: hir::ProcID<'hir>) -> &hir::ProcData<'hir> {
        self.hir_procs.id_get(id)
    }
    pub fn enum_data(&self, id: hir::EnumID) -> &hir::EnumData<'hir> {
        &self.hir_enums[id.index()]
    }
    pub fn struct_data(&self, id: hir::StructID) -> &hir::StructData<'hir> {
        &self.hir_structs[id.index()]
    }
    pub fn const_data(&self, id: hir::ConstID) -> &hir::ConstData<'hir> {
        &self.hir_consts[id.index()]
    }
    pub fn global_data(&self, id: hir::GlobalID) -> &hir::GlobalData<'hir> {
        &self.hir_globals[id.index()]
    }
    pub fn import_data(&self, id: hir::ImportID) -> &hir::ImportData {
        &self.hir_imports[id.index()]
    }
    pub fn const_eval(&self, id: hir::ConstEvalID) -> &(hir::ConstEval<'ast>, ModuleID) {
        &self.const_evals[id.index()]
    }
    pub fn variant_eval(&self, id: hir::VariantEvalID) -> &hir::VariantEval<'hir> {
        &self.variant_evals[id.index()]
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID<'hir>) -> &mut hir::ProcData<'hir> {
        self.hir_procs.id_get_mut(id)
    }
    pub fn enum_data_mut(&mut self, id: hir::EnumID) -> &mut hir::EnumData<'hir> {
        &mut self.hir_enums[id.index()]
    }
    pub fn struct_data_mut(&mut self, id: hir::StructID) -> &mut hir::StructData<'hir> {
        &mut self.hir_structs[id.index()]
    }
    pub fn const_data_mut(&mut self, id: hir::ConstID) -> &mut hir::ConstData<'hir> {
        &mut self.hir_consts[id.index()]
    }
    pub fn global_data_mut(&mut self, id: hir::GlobalID) -> &mut hir::GlobalData<'hir> {
        &mut self.hir_globals[id.index()]
    }
    pub fn const_eval_mut(
        &mut self,
        id: hir::ConstEvalID,
    ) -> &mut (hir::ConstEval<'ast>, ModuleID) {
        &mut self.const_evals[id.index()]
    }
    pub fn variant_eval_mut(&mut self, id: hir::VariantEvalID) -> &mut hir::VariantEval<'hir> {
        &mut self.variant_evals[id.index()]
    }
}
