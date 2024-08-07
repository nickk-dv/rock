use crate::arena::Arena;
use crate::ast;
use crate::error::{DiagnosticCollection, ErrorComp, Info, ResultComp, SourceRange, WarningComp};
use crate::hir;
use crate::hir::intern::ConstInternPool;
use crate::intern::{InternID, InternPool};
use crate::session::ModuleID;
use crate::text::TextRange;
use std::collections::HashMap;

/// prevents allocation of `hir::Expr::Error` during typechecking
pub const EXPR_ERROR: &hir::Expr = &hir::Expr::Error;

pub struct HirData<'hir, 'ast, 'intern> {
    modules: Vec<Module>,
    registry: Registry<'hir, 'ast>,
    ast: ast::Ast<'ast, 'intern>,
}

pub struct Module {
    symbols: HashMap<InternID, Symbol>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Symbol {
    Defined  { kind: SymbolKind, },
    Imported { kind: SymbolKind, import_range: TextRange },
}

#[derive(Copy, Clone)]
pub enum SymbolKind {
    Module(ModuleID),
    Proc(hir::ProcID),
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
    hir_procs: Vec<hir::ProcData<'hir>>,
    hir_enums: Vec<hir::EnumData<'hir>>,
    hir_structs: Vec<hir::StructData<'hir>>,
    hir_consts: Vec<hir::ConstData<'hir>>,
    hir_globals: Vec<hir::GlobalData<'hir>>,
    const_evals: Vec<(hir::ConstEval<'ast>, ModuleID)>,
}

pub struct HirEmit<'hir> {
    pub arena: Arena<'hir>,
    pub const_intern: ConstInternPool<'hir>,
    diagnostics: DiagnosticCollection,
}

impl<'hir, 'ast, 'intern> HirData<'hir, 'ast, 'intern> {
    pub fn new(ast: ast::Ast<'ast, 'intern>) -> Self {
        let mut modules = Vec::with_capacity(ast.modules.len());
        for _ in ast.modules.iter() {
            modules.push(Module {
                symbols: HashMap::with_capacity(64),
            });
        }
        let registry = Registry::new(&ast.modules);

        HirData {
            modules,
            registry,
            ast,
        }
    }

    fn module(&self, id: ModuleID) -> &Module {
        &self.modules[id.index()]
    }
    fn module_mut(&mut self, id: ModuleID) -> &mut Module {
        &mut self.modules[id.index()]
    }

    pub fn registry(&self) -> &Registry<'hir, 'ast> {
        &self.registry
    }
    pub fn registry_mut(&mut self) -> &mut Registry<'hir, 'ast> {
        &mut self.registry
    }

    pub fn name_str(&self, id: InternID) -> &str {
        self.ast.intern_name.get_str(id)
    }
    pub fn intern_name(&mut self) -> &mut InternPool<'intern> {
        &mut self.ast.intern_name
    }
    pub fn intern_string(&self) -> &InternPool<'intern> {
        &self.ast.intern_string
    }
    pub fn ast_module(&self, module_id: ModuleID) -> ast::Module<'ast> {
        self.ast.modules[module_id.index()]
    }

    pub fn add_symbol(&mut self, origin_id: ModuleID, id: InternID, symbol: Symbol) {
        let origin = self.module_mut(origin_id);
        origin.symbols.insert(id, symbol);
    }

    pub fn symbol_defined(&self, origin_id: ModuleID, id: InternID) -> Option<SymbolKind> {
        let origin = self.module(origin_id);

        match origin.symbols.get(&id).cloned() {
            Some(Symbol::Defined { kind }) => Some(kind),
            _ => None,
        }
    }

    pub fn symbol_in_scope_source(&self, origin_id: ModuleID, id: InternID) -> Option<SourceRange> {
        let origin = self.module(origin_id);
        let symbol = origin.symbols.get(&id).cloned()?;

        match symbol {
            Symbol::Defined { kind } => {
                Some(SourceRange::new(origin_id, kind.name_range(&self.registry)))
            }
            Symbol::Imported { import_range, .. } => {
                Some(SourceRange::new(origin_id, import_range))
            }
        }
    }

    pub fn symbol_from_scope(
        &self,
        origin_id: ModuleID,
        target_id: ModuleID,
        name: ast::Name,
    ) -> Result<(SymbolKind, SourceRange), ErrorComp> {
        let target = self.module(target_id);

        match target.symbols.get(&name.id).copied() {
            Some(Symbol::Defined { kind }) => {
                let source = SourceRange::new(target_id, kind.name_range(&self.registry));

                let vis = if origin_id == target_id {
                    ast::Vis::Public
                } else {
                    kind.vis(&self.registry)
                };

                return match vis {
                    ast::Vis::Public => Ok((kind, source)),
                    ast::Vis::Private => Err(ErrorComp::new(
                        format!(
                            "{} `{}` is private",
                            kind.kind_name(),
                            self.name_str(name.id)
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
            format!("name `{}` is not found in module", self.name_str(name.id)),
            SourceRange::new(origin_id, name.range),
            None,
        ))
    }
}

impl SymbolKind {
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
    pub fn new(modules: &[ast::Module<'ast>]) -> Registry<'hir, 'ast> {
        let mut proc_count = 0;
        let mut enum_count = 0;
        let mut struct_count = 0;
        let mut const_count = 0;
        let mut global_count = 0;

        for module in modules {
            for item in module.items {
                match item {
                    ast::Item::Proc(_) => proc_count += 1,
                    ast::Item::Enum(_) => enum_count += 1,
                    ast::Item::Struct(_) => struct_count += 1,
                    ast::Item::Const(_) => const_count += 1,
                    ast::Item::Global(_) => global_count += 1,
                    ast::Item::Import(_) => {}
                }
            }
        }

        let consteval_count = enum_count * 4 + const_count + global_count;

        Registry {
            ast_procs: Vec::with_capacity(proc_count),
            ast_enums: Vec::with_capacity(enum_count),
            ast_structs: Vec::with_capacity(struct_count),
            ast_consts: Vec::with_capacity(const_count),
            ast_globals: Vec::with_capacity(global_count),
            hir_procs: Vec::with_capacity(proc_count),
            hir_enums: Vec::with_capacity(enum_count),
            hir_structs: Vec::with_capacity(struct_count),
            hir_consts: Vec::with_capacity(const_count),
            hir_globals: Vec::with_capacity(global_count),
            const_evals: Vec::with_capacity(consteval_count),
        }
    }

    pub fn add_proc(
        &mut self,
        item: &'ast ast::ProcItem<'ast>,
        data: hir::ProcData<'hir>,
    ) -> hir::ProcID {
        let id = hir::ProcID::new(self.hir_procs.len());
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

    pub fn add_const_eval(
        &mut self,
        const_expr: ast::ConstExpr<'ast>,
        origin_id: ModuleID,
    ) -> hir::ConstEvalID {
        let id = hir::ConstEvalID::new(self.const_evals.len());
        self.const_evals
            .push((hir::ConstEval::Unresolved(const_expr), origin_id));
        id
    }

    pub fn proc_ids(&self) -> impl Iterator<Item = hir::ProcID> {
        (0..self.hir_procs.len()).map(hir::ProcID::new)
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
    pub fn const_eval_ids(&self) -> impl Iterator<Item = hir::ConstEvalID> {
        (0..self.const_evals.len()).map(hir::ConstEvalID::new)
    }

    pub fn proc_item(&self, id: hir::ProcID) -> &'ast ast::ProcItem<'ast> {
        self.ast_procs[id.index()]
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

    pub fn proc_data(&self, id: hir::ProcID) -> &hir::ProcData<'hir> {
        &self.hir_procs[id.index()]
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
    pub fn const_eval(&self, id: hir::ConstEvalID) -> &(hir::ConstEval<'ast>, ModuleID) {
        &self.const_evals[id.index()]
    }

    pub fn proc_data_mut(&mut self, id: hir::ProcID) -> &mut hir::ProcData<'hir> {
        &mut self.hir_procs[id.index()]
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
}

impl<'hir> HirEmit<'hir> {
    pub fn new() -> HirEmit<'hir> {
        HirEmit {
            arena: Arena::new(),
            const_intern: ConstInternPool::new(),
            diagnostics: DiagnosticCollection::new(),
        }
    }

    #[inline]
    pub fn error(&mut self, error: ErrorComp) {
        self.diagnostics.error(error);
    }
    #[inline]
    pub fn warning(&mut self, warning: WarningComp) {
        self.diagnostics.warning(warning);
    }
    #[inline]
    pub fn error_count(&self) -> usize {
        self.diagnostics.errors().len()
    }
    #[inline]
    pub fn did_error(&self, error_count: usize) -> bool {
        self.error_count() > error_count
    }

    pub fn emit<'ast, 'intern: 'hir>(
        self,
        hir: HirData<'hir, 'ast, 'intern>,
    ) -> ResultComp<hir::Hir<'hir>> {
        if !self.diagnostics.errors().is_empty() {
            return ResultComp::Err(self.diagnostics);
        }

        let mut const_values = Vec::with_capacity(hir.registry.const_evals.len());
        let mut errors = Vec::new();

        for (eval, origin_id) in hir.registry.const_evals.iter() {
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
                hir::ConstEval::ResolvedValue(value_id) => const_values.push(value_id),
            }
        }

        if errors.is_empty() {
            let hir = hir::Hir {
                arena: self.arena,
                intern_name: hir.ast.intern_name,
                intern_string: hir.ast.intern_string,
                string_is_cstr: hir.ast.string_is_cstr,
                const_intern: self.const_intern,
                procs: hir.registry.hir_procs,
                enums: hir.registry.hir_enums,
                structs: hir.registry.hir_structs,
                consts: hir.registry.hir_consts,
                globals: hir.registry.hir_globals,
                const_values,
            };
            ResultComp::Ok((hir, self.diagnostics.warnings_moveout()))
        } else {
            ResultComp::Err(self.diagnostics.join_errors(errors))
        }
    }
}
