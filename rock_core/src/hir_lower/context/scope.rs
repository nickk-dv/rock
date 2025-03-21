use super::registry::Registry;
use super::HirCtx;
use crate::ast;
use crate::error::{ErrorWarningBuffer, SourceRange};
use crate::errors as err;
use crate::hir;
use crate::hir_lower::pass_5::Expectation;
use crate::intern::NameID;
use crate::session::{ModuleID, Session};
use crate::text::TextRange;
use std::collections::HashMap;

pub struct Scope<'hir> {
    origin_id: ModuleID,
    pub global: GlobalScope,
    pub local: LocalScope<'hir>,
    pub poly: PolyScope,
}

//==================== GLOBAL SCOPE ====================

pub struct GlobalScope {
    modules: Vec<ModuleScope>,
}

struct ModuleScope {
    name_id: NameID,
    symbols: HashMap<NameID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Defined(SymbolID),
    Imported(SymbolID, TextRange),
    ImportedModule(ModuleID, TextRange),
}

#[derive(Copy, Clone)]
pub enum SymbolID {
    Proc(hir::ProcID),
    Enum(hir::EnumID),
    Struct(hir::StructID),
    Const(hir::ConstID),
    Global(hir::GlobalID),
}

#[derive(Copy, Clone)]
pub enum SymbolOrModule {
    Symbol(SymbolID),
    Module(ModuleID),
}

//==================== LOCAL SCOPE ====================

pub struct LocalScope<'hir> {
    proc_id: Option<hir::ProcID>,
    params: &'hir [hir::Param<'hir>],
    return_expect: Expectation<'hir>,
    blocks: Vec<BlockData>,
    variables: Vec<hir::Variable<'hir>>,
    variables_in_scope: Vec<hir::VariableID>,
    defer_blocks_in_scope: Vec<hir::Block<'hir>>,
}

#[derive(Copy, Clone)]
pub enum LocalVariableID {
    Param(hir::ParamID),
    Variable(hir::VariableID),
}

#[derive(Copy, Clone)]
struct BlockData {
    var_count: u32,
    defer_count: u32,
    status: BlockStatus,
    diverges: Diverges,
}

#[derive(Copy, Clone)]
pub enum BlockStatus {
    None,
    Loop,
    Defer(TextRange),
}

#[derive(Copy, Clone)]
pub enum Diverges {
    Maybe,
    Always(TextRange),
    AlwaysWarned,
}

//==================== POLY SCOPE ====================

pub struct PolyScope {
    poly_def: Option<hir::PolymorphDefID>,
}

//==================== SCOPES IMPL ====================

impl<'hir> Scope<'hir> {
    pub(super) fn new(session: &Session) -> Scope<'hir> {
        Scope {
            origin_id: ModuleID::dummy(),
            global: GlobalScope::new(session),
            local: LocalScope::new(),
            poly: PolyScope::new(),
        }
    }

    #[inline]
    pub fn origin(&self) -> ModuleID {
        self.origin_id
    }
    #[inline]
    pub fn set_origin(&mut self, origin_id: ModuleID) {
        self.origin_id = origin_id;
    }
    #[inline]
    pub fn set_poly(&mut self, poly_def: Option<hir::PolymorphDefID>) {
        self.poly.poly_def = poly_def;
    }

    pub fn check_already_defined(
        &mut self,
        name: ast::Name,
        session: &Session,
        registry: &Registry,
        emit: &mut ErrorWarningBuffer,
    ) -> Result<(), ()> {
        self.check_already_defined_global(name, session, registry, emit)?;
        self.check_already_defined_local(name, session, emit)
    }

    pub fn check_already_defined_global(
        &self,
        name: ast::Name,
        session: &Session,
        registry: &Registry,
        emit: &mut ErrorWarningBuffer,
    ) -> Result<(), ()> {
        let origin = self.global.module(self.origin_id);
        let existing = match origin.symbols.get(&name.id).copied() {
            Some(symbol) => symbol,
            None => return Ok(()),
        };
        let name_src = SourceRange::new(self.origin_id, name.range);
        let existing = self.symbol_src(existing, registry);
        let name = session.intern_name.get(name.id);
        err::scope_name_already_defined(emit, name_src, existing, name);
        Err(())
    }

    fn check_already_defined_local(
        &mut self,
        name: ast::Name,
        session: &Session,
        emit: &mut ErrorWarningBuffer,
    ) -> Result<(), ()> {
        let existing = match self.local.find_variable(name.id, false) {
            Some(var_id) => var_id,
            None => return Ok(()),
        };
        let name_src = SourceRange::new(self.origin_id, name.range);
        let existing = self.var_src(existing);
        let name = session.intern_name.get(name.id);
        err::scope_name_already_defined(emit, name_src, existing, name);
        Err(())
    }

    fn symbol_src(&self, symbol: Symbol, registry: &Registry<'hir, '_>) -> SourceRange {
        match symbol {
            Symbol::Defined(symbol_id) => symbol_id.src(registry),
            Symbol::Imported(_, import) => SourceRange::new(self.origin_id, import),
            Symbol::ImportedModule(_, import) => SourceRange::new(self.origin_id, import),
        }
    }

    pub fn var_src(&self, local_var_id: LocalVariableID) -> SourceRange {
        let range = match local_var_id {
            LocalVariableID::Param(id) => self.local.param(id).name.range,
            LocalVariableID::Variable(id) => self.local.variable(id).name.range,
        };
        SourceRange::new(self.origin_id, range)
    }
}

impl GlobalScope {
    fn new(session: &Session) -> GlobalScope {
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

            modules.push(ModuleScope {
                name_id: module.name(),
                symbols: HashMap::with_capacity(symbol_count),
            });
        }

        GlobalScope { modules }
    }

    pub fn add_symbol(&mut self, origin_id: ModuleID, name_id: NameID, symbol: Symbol) {
        let module = self.module_mut(origin_id);
        let existing = module.symbols.insert(name_id, symbol);
        assert!(existing.is_none());
    }

    pub fn find_symbol(
        &self,
        origin_id: ModuleID,
        target_id: ModuleID,
        name: ast::Name,
        session: &Session,
        registry: &Registry,
        emit: &mut ErrorWarningBuffer,
    ) -> Result<SymbolOrModule, ()> {
        let target = self.module(target_id);

        match target.symbols.get(&name.id).copied() {
            Some(Symbol::Defined(symbol_id)) => {
                if origin_id == target_id {
                    return Ok(SymbolOrModule::Symbol(symbol_id));
                }

                let vis = symbol_id.vis(registry);
                return match vis {
                    hir::Vis::Public => Ok(SymbolOrModule::Symbol(symbol_id)),
                    hir::Vis::Package => {
                        let origin_package_id = session.module.get(origin_id).origin();
                        let target_package_id = session.module.get(target_id).origin();
                        if origin_package_id != target_package_id {
                            let name_src = SourceRange::new(origin_id, name.range);
                            let defined_src = symbol_id.src(registry);
                            let name = session.intern_name.get(name.id);
                            err::scope_symbol_package_vis(
                                emit,
                                name_src,
                                defined_src,
                                name,
                                symbol_id.desc(),
                            );
                            Err(())
                        } else {
                            Ok(SymbolOrModule::Symbol(symbol_id))
                        }
                    }
                    hir::Vis::Private => {
                        let name_src = SourceRange::new(origin_id, name.range);
                        let defined_src = symbol_id.src(registry);
                        let name = session.intern_name.get(name.id);
                        err::scope_symbol_is_private(
                            emit,
                            name_src,
                            defined_src,
                            name,
                            symbol_id.desc(),
                        );
                        Err(())
                    }
                };
            }
            Some(Symbol::Imported(symbol_id, _)) => {
                if origin_id == target_id {
                    return Ok(SymbolOrModule::Symbol(symbol_id));
                }
            }
            Some(Symbol::ImportedModule(module_id, _)) => {
                if origin_id == target_id {
                    return Ok(SymbolOrModule::Module(module_id));
                }
            }
            None => {}
        }

        let name_src = SourceRange::new(origin_id, name.range);
        let name = session.intern_name.get(name.id);
        let from_module = if origin_id != target_id {
            let target_name = session.intern_name.get(target.name_id);
            Some(target_name)
        } else {
            None
        };
        err::scope_symbol_not_found(emit, name_src, name, from_module);
        Err(())
    }

    pub fn find_defined_proc(&self, target_id: ModuleID, name_id: NameID) -> Option<hir::ProcID> {
        let target = self.module(target_id);
        match target.symbols.get(&name_id).copied() {
            Some(Symbol::Defined(SymbolID::Proc(id))) => Some(id),
            _ => None,
        }
    }

    pub fn find_defined_struct(
        &self,
        target_id: ModuleID,
        name_id: NameID,
    ) -> Option<hir::StructID> {
        let target = self.module(target_id);
        match target.symbols.get(&name_id).copied() {
            Some(Symbol::Defined(SymbolID::Struct(id))) => Some(id),
            _ => None,
        }
    }

    #[inline]
    fn module(&self, module_id: ModuleID) -> &ModuleScope {
        &self.modules[module_id.index()]
    }
    #[inline]
    fn module_mut(&mut self, module_id: ModuleID) -> &mut ModuleScope {
        &mut self.modules[module_id.index()]
    }
}

impl<'hir> LocalScope<'hir> {
    fn new() -> LocalScope<'hir> {
        LocalScope {
            proc_id: None,
            params: &[],
            return_expect: Expectation::None,
            blocks: Vec::with_capacity(32),
            variables: Vec::with_capacity(128),
            variables_in_scope: Vec::with_capacity(128),
            defer_blocks_in_scope: Vec::with_capacity(32),
        }
    }

    pub fn reset(&mut self) {
        self.proc_id = None;
        self.params = &[];
        self.return_expect = Expectation::None;
        self.blocks.clear();
        self.variables.clear();
        self.variables_in_scope.clear();
    }

    pub fn set_proc_context(
        &mut self,
        proc_id: Option<hir::ProcID>,
        params: &'hir [hir::Param<'hir>],
        return_expect: Expectation<'hir>,
    ) {
        self.proc_id = proc_id;
        self.params = params;
        self.return_expect = return_expect;
    }

    pub fn finish_proc_context(&self) -> &[hir::Variable<'hir>] {
        &self.variables
    }

    pub fn proc_id(&self) -> Option<hir::ProcID> {
        self.proc_id
    }
    pub fn return_expect(&self) -> Expectation<'hir> {
        self.return_expect
    }

    pub fn start_block(&mut self, status: BlockStatus) {
        let diverges = self.blocks.last().map(|b| b.diverges).unwrap_or(Diverges::Maybe);
        let data = BlockData { var_count: 0, defer_count: 0, status, diverges };
        self.blocks.push(data);
    }

    pub fn exit_block(&mut self) {
        let curr = self.current_block();
        for _ in 0..curr.var_count {
            self.variables_in_scope.pop();
        }
        for _ in 0..curr.defer_count {
            self.defer_blocks_in_scope.pop();
        }
        self.blocks.pop();
    }

    #[must_use]
    pub fn add_variable(&mut self, var: hir::Variable<'hir>) -> hir::VariableID {
        let var_id = hir::VariableID::new(self.variables.len());
        self.variables.push(var);
        if var.name.id.raw() != 0 {
            self.variables_in_scope.push(var_id);
            self.current_block_mut().var_count += 1;
        }
        var_id
    }

    pub fn add_defer_block(&mut self, block: hir::Block<'hir>) {
        self.defer_blocks_in_scope.push(block);
        self.current_block_mut().defer_count += 1;
    }

    #[must_use]
    pub fn find_variable(
        &mut self,
        name_id: NameID,
        set_usage_flag: bool,
    ) -> Option<LocalVariableID> {
        for (idx, param) in self.params.iter().enumerate() {
            if name_id == param.name.id {
                let param_id = hir::ParamID::new(idx);
                return Some(LocalVariableID::Param(param_id));
            }
        }
        for var_id in self.variables_in_scope.iter().copied() {
            let var = &mut self.variables[var_id.index()];
            if name_id == var.name.id {
                if set_usage_flag {
                    var.was_used = true;
                }
                return Some(LocalVariableID::Variable(var_id));
            }
        }
        None
    }

    pub fn find_prev_defer(&self) -> Option<TextRange> {
        for block in self.blocks.iter().rev() {
            match block.status {
                BlockStatus::None => {}
                BlockStatus::Loop => {}
                BlockStatus::Defer(range) => return Some(range),
            }
        }
        None
    }

    pub fn find_prev_loop_before_defer(&self) -> (bool, Option<TextRange>) {
        let mut defer = None;
        for block in self.blocks.iter().rev() {
            match block.status {
                BlockStatus::None => {}
                BlockStatus::Loop => return (true, defer),
                BlockStatus::Defer(range) => defer = Some(range),
            }
        }
        (false, defer)
    }

    #[inline]
    pub fn param(&self, param_id: hir::ParamID) -> &hir::Param<'hir> {
        &self.params[param_id.index()]
    }
    #[inline]
    pub fn variable(&self, var_id: hir::VariableID) -> &hir::Variable<'hir> {
        &self.variables[var_id.index()]
    }
    #[inline]
    pub fn defer_blocks_all(&self) -> &[hir::Block<'hir>] {
        &self.defer_blocks_in_scope
    }
    #[inline]
    pub fn defer_blocks_last(&self) -> &[hir::Block<'hir>] {
        let defer_count = self.current_block().defer_count;
        let total_count = self.defer_blocks_in_scope.len();
        let range = (total_count - defer_count as usize)..total_count;
        &self.defer_blocks_in_scope[range]
    }
    #[inline]
    pub fn defer_blocks_loop(&self) -> &[hir::Block<'hir>] {
        let mut defer_count = 0;
        for block in self.blocks.iter().rev() {
            defer_count += block.defer_count;
            if let BlockStatus::Loop = block.status {
                let total_count = self.defer_blocks_in_scope.len();
                let range = (total_count - defer_count as usize)..total_count;
                return &self.defer_blocks_in_scope[range];
            }
        }
        unreachable!()
    }

    #[inline]
    fn current_block(&self) -> BlockData {
        self.blocks.last().copied().unwrap()
    }
    #[inline]
    fn current_block_mut(&mut self) -> &mut BlockData {
        self.blocks.last_mut().unwrap()
    }
    #[inline]
    pub fn diverges(&mut self) -> Diverges {
        self.current_block_mut().diverges
    }
    #[inline]
    pub fn diverges_set(&mut self, diverges: Diverges) {
        self.current_block_mut().diverges = diverges;
    }
}

impl PolyScope {
    fn new() -> PolyScope {
        PolyScope { poly_def: None }
    }

    pub fn find_poly_param(
        &self,
        name_id: NameID,
        registry: &Registry,
    ) -> Option<(hir::PolymorphDefID, u32)> {
        let poly_def = self.poly_def?;
        let poly_params = match poly_def {
            hir::PolymorphDefID::Proc(id) => registry.proc_data(id).poly_params,
            hir::PolymorphDefID::Enum(id) => registry.enum_data(id).poly_params,
            hir::PolymorphDefID::Struct(id) => registry.struct_data(id).poly_params,
        }?;
        for (idx, param) in poly_params.iter().enumerate() {
            if name_id == param.id {
                return Some((poly_def, idx as u32));
            }
        }
        None
    }
}

impl SymbolID {
    pub fn desc(self) -> &'static str {
        match self {
            SymbolID::Proc(_) => "procedure",
            SymbolID::Enum(_) => "enum",
            SymbolID::Struct(_) => "struct",
            SymbolID::Const(_) => "const",
            SymbolID::Global(_) => "global",
        }
    }

    pub fn src(self, registry: &Registry) -> SourceRange {
        match self {
            SymbolID::Proc(id) => registry.proc_data(id).src(),
            SymbolID::Enum(id) => registry.enum_data(id).src(),
            SymbolID::Struct(id) => registry.struct_data(id).src(),
            SymbolID::Const(id) => registry.const_data(id).src(),
            SymbolID::Global(id) => registry.global_data(id).src(),
        }
    }

    fn vis(self, registry: &Registry) -> hir::Vis {
        match self {
            SymbolID::Proc(id) => registry.proc_data(id).vis,
            SymbolID::Enum(id) => registry.enum_data(id).vis,
            SymbolID::Struct(id) => registry.struct_data(id).vis,
            SymbolID::Const(id) => registry.const_data(id).vis,
            SymbolID::Global(id) => registry.global_data(id).vis,
        }
    }
}

impl LocalVariableID {
    pub fn desc(self) -> &'static str {
        match self {
            LocalVariableID::Param(_) => "parameter",
            LocalVariableID::Variable(_) => "variable",
        }
    }
}

pub fn check_find_enum_variant(
    ctx: &mut HirCtx,
    enum_id: hir::EnumID,
    name: ast::Name,
) -> Option<hir::VariantID> {
    let enum_data = ctx.registry.enum_data(enum_id);
    for (idx, variant) in enum_data.variants.iter().enumerate() {
        if variant.name.id == name.id {
            return Some(hir::VariantID::new(idx));
        }
    }
    let src = ctx.src(name.range);
    let enum_src = enum_data.src();
    let name = ctx.name(name.id);
    let enum_name = ctx.name(enum_data.name.id);
    err::scope_enum_variant_not_found(&mut ctx.emit, src, enum_src, name, enum_name);
    None
}

pub fn check_find_struct_field(
    ctx: &mut HirCtx,
    struct_id: hir::StructID,
    name: ast::Name,
) -> Option<hir::FieldID> {
    let struct_data = ctx.registry.struct_data(struct_id);
    for (idx, field) in struct_data.fields.iter().enumerate() {
        if field.name.id == name.id {
            return Some(hir::FieldID::new(idx));
        }
    }
    let src = ctx.src(name.range);
    let struct_src = struct_data.src();
    let name = ctx.name(name.id);
    let struct_name = ctx.name(struct_data.name.id);
    err::scope_struct_field_not_found(&mut ctx.emit, src, struct_src, name, struct_name);
    None
}
