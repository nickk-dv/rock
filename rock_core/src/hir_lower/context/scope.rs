use super::registry::Registry;
use super::HirCtx;
use crate::ast;
use crate::error::{ErrorWarningBuffer, SourceRange};
use crate::errors as err;
use crate::hir;
use crate::hir_lower::pass_5::Expectation;
use crate::intern::InternName;
use crate::session::{ModuleID, Session};
use crate::support::{IndexID, ID};
use crate::text::TextRange;
use std::collections::HashMap;

pub struct Scope<'hir> {
    origin_id: ModuleID,
    pub global: GlobalScope<'hir>,
    pub local: LocalScope<'hir>,
}

//==================== GLOBAL SCOPE ====================

pub struct GlobalScope<'hir> {
    modules: Vec<ModuleScope<'hir>>,
}

struct ModuleScope<'hir> {
    name_id: ID<InternName>,
    symbols: HashMap<ID<InternName>, Symbol<'hir>>,
}

#[derive(Copy, Clone)]
pub enum Symbol<'hir> {
    Defined(SymbolID<'hir>),
    Imported(SymbolID<'hir>, TextRange),
    ImportedModule(ModuleID, TextRange),
}

#[derive(Copy, Clone)]
pub enum SymbolID<'hir> {
    Proc(hir::ProcID<'hir>),
    Enum(hir::EnumID<'hir>),
    Struct(hir::StructID<'hir>),
    Const(hir::ConstID<'hir>),
    Global(hir::GlobalID<'hir>),
}

#[derive(Copy, Clone)]
pub enum SymbolOrModule<'hir> {
    Symbol(SymbolID<'hir>),
    Module(ModuleID),
}

//==================== LOCAL SCOPE ====================

pub struct LocalScope<'hir> {
    params: &'hir [hir::Param<'hir>],
    return_expect: Expectation<'hir>,
    blocks: Vec<BlockData>,
    locals: Vec<hir::Local<'hir>>,
    binds: Vec<hir::LocalBind<'hir>>,
    locals_in_scope: Vec<hir::LocalID<'hir>>,
    binds_in_scope: Vec<hir::LocalBindID<'hir>>,
}

#[derive(Copy, Clone)]
pub enum VariableID<'hir> {
    Param(hir::ParamID<'hir>),
    Local(hir::LocalID<'hir>),
    Bind(hir::LocalBindID<'hir>),
}

#[derive(Copy, Clone)]
struct BlockData {
    bind_count: u32,
    local_count: u32,
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

//==================== SCOPES IMPL ====================

impl<'hir> Scope<'hir> {
    pub(super) fn new(session: &Session) -> Scope<'hir> {
        Scope {
            origin_id: ModuleID::dummy(),
            global: GlobalScope::new(session),
            local: LocalScope::new(),
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

    pub fn check_already_defined(
        &self,
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
        Ok(())
    }

    fn check_already_defined_local(
        &self,
        name: ast::Name,
        session: &Session,
        emit: &mut ErrorWarningBuffer,
    ) -> Result<(), ()> {
        let existing = match self.local.find_variable(name.id) {
            Some(var_id) => var_id,
            None => return Ok(()),
        };
        let name_src = SourceRange::new(self.origin_id, name.range);
        let existing = self.var_src(existing);
        let name = session.intern_name.get(name.id);
        err::scope_name_already_defined(emit, name_src, existing, name);
        Ok(())
    }

    fn symbol_src(&self, symbol: Symbol<'hir>, registry: &Registry<'hir, '_>) -> SourceRange {
        match symbol {
            Symbol::Defined(symbol_id) => symbol_id.src(registry),
            Symbol::Imported(_, import) => SourceRange::new(self.origin_id, import),
            Symbol::ImportedModule(_, import) => SourceRange::new(self.origin_id, import),
        }
    }

    pub fn var_src(&self, var_id: VariableID<'hir>) -> SourceRange {
        let range = match var_id {
            VariableID::Param(id) => self.local.param(id).name.range,
            VariableID::Local(id) => self.local.local(id).name.range,
            VariableID::Bind(id) => self.local.bind(id).name.range,
        };
        SourceRange::new(self.origin_id, range)
    }
}

impl<'hir> GlobalScope<'hir> {
    fn new(session: &Session) -> GlobalScope<'hir> {
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

    pub fn add_symbol(
        &mut self,
        origin_id: ModuleID,
        name_id: ID<InternName>,
        symbol: Symbol<'hir>,
    ) {
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
    ) -> Result<SymbolOrModule<'hir>, ()> {
        let target = self.module(target_id);

        match target.symbols.get(&name.id).copied() {
            Some(Symbol::Defined(symbol_id)) => {
                if origin_id == target_id {
                    return Ok(SymbolOrModule::Symbol(symbol_id));
                }

                let vis = symbol_id.vis(registry);
                return match vis {
                    ast::Vis::Public => Ok(SymbolOrModule::Symbol(symbol_id)),
                    ast::Vis::Private => {
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

    pub fn find_defined_proc(
        &self,
        target_id: ModuleID,
        name_id: ID<InternName>,
    ) -> Option<hir::ProcID<'hir>> {
        let target = self.module(target_id);
        match target.symbols.get(&name_id).copied() {
            Some(Symbol::Defined(SymbolID::Proc(id))) => Some(id),
            _ => None,
        }
    }

    pub fn find_defined_struct(
        &self,
        target_id: ModuleID,
        name_id: ID<InternName>,
    ) -> Option<hir::StructID<'hir>> {
        let target = self.module(target_id);
        match target.symbols.get(&name_id).copied() {
            Some(Symbol::Defined(SymbolID::Struct(id))) => Some(id),
            _ => None,
        }
    }

    #[inline]
    fn module(&self, module_id: ModuleID) -> &ModuleScope<'hir> {
        &self.modules[module_id.index()]
    }
    #[inline]
    fn module_mut(&mut self, module_id: ModuleID) -> &mut ModuleScope<'hir> {
        &mut self.modules[module_id.index()]
    }
}

impl<'hir> LocalScope<'hir> {
    fn new() -> LocalScope<'hir> {
        LocalScope {
            params: &[],
            return_expect: Expectation::None,
            blocks: Vec::with_capacity(64),
            binds: Vec::with_capacity(64),
            locals: Vec::with_capacity(64),
            binds_in_scope: Vec::with_capacity(64),
            locals_in_scope: Vec::with_capacity(64),
        }
    }

    pub fn reset(&mut self) {
        self.params = &[];
        self.return_expect = Expectation::None;
        self.blocks.clear();
        self.binds.clear();
        self.locals.clear();
        self.binds_in_scope.clear();
        self.locals_in_scope.clear();
    }

    pub fn set_proc_context(
        &mut self,
        params: &'hir [hir::Param<'hir>],
        return_expect: Expectation<'hir>,
    ) {
        self.params = params;
        self.return_expect = return_expect;
    }

    pub fn finish_proc_context(&self) -> (&[hir::Local<'hir>], &[hir::LocalBind<'hir>]) {
        (&self.locals, &self.binds)
    }

    pub fn return_expect(&self) -> Expectation<'hir> {
        self.return_expect
    }

    pub fn start_block(&mut self, status: BlockStatus) {
        let diverges = self
            .blocks
            .last()
            .map(|b| b.diverges)
            .unwrap_or(Diverges::Maybe);
        let data = BlockData {
            local_count: 0,
            bind_count: 0,
            status,
            diverges,
        };
        self.blocks.push(data);
    }

    pub fn exit_block(&mut self) {
        let curr = self.current_block();
        for _ in 0..curr.local_count {
            self.locals_in_scope.pop();
        }
        for _ in 0..curr.bind_count {
            self.binds_in_scope.pop();
        }
        self.blocks.pop();
    }

    #[must_use]
    pub fn add_local(&mut self, local: hir::Local<'hir>) -> hir::LocalID<'hir> {
        let local_id = hir::LocalID::new(&self.locals);
        self.locals.push(local);
        self.locals_in_scope.push(local_id);
        self.current_block_mut().local_count += 1;
        local_id
    }

    #[must_use]
    pub fn add_bind(&mut self, bind: hir::LocalBind<'hir>) -> hir::LocalBindID<'hir> {
        let bind_id = hir::LocalBindID::new(&self.binds);
        self.binds.push(bind);
        self.binds_in_scope.push(bind_id);
        self.current_block_mut().bind_count += 1;
        bind_id
    }

    #[must_use]
    pub fn find_variable(&self, name_id: ID<InternName>) -> Option<VariableID<'hir>> {
        for (idx, param) in self.params.iter().enumerate() {
            if name_id == param.name.id {
                let param_id = hir::ParamID::new_raw(idx);
                return Some(VariableID::Param(param_id));
            }
        }
        for local_id in self.locals_in_scope.iter().copied() {
            if name_id == self.local(local_id).name.id {
                return Some(VariableID::Local(local_id));
            }
        }
        for bind_id in self.binds_in_scope.iter().copied() {
            if name_id == self.bind(bind_id).name.id {
                return Some(VariableID::Bind(bind_id));
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
    pub fn param(&self, param_id: hir::ParamID<'hir>) -> &hir::Param<'hir> {
        self.params.id_get(param_id)
    }
    #[inline]
    pub fn local(&self, local_id: hir::LocalID<'hir>) -> &hir::Local<'hir> {
        self.locals.id_get(local_id)
    }
    #[inline]
    pub fn bind(&self, bind_id: hir::LocalBindID<'hir>) -> &hir::LocalBind<'hir> {
        self.binds.id_get(bind_id)
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

impl<'hir> SymbolID<'hir> {
    pub fn desc(self) -> &'static str {
        match self {
            SymbolID::Proc(_) => "procedure",
            SymbolID::Enum(_) => "enum",
            SymbolID::Struct(_) => "struct",
            SymbolID::Const(_) => "const",
            SymbolID::Global(_) => "global",
        }
    }

    pub fn src(self, registry: &Registry<'hir, '_>) -> SourceRange {
        match self {
            SymbolID::Proc(id) => registry.proc_data(id).src(),
            SymbolID::Enum(id) => registry.enum_data(id).src(),
            SymbolID::Struct(id) => registry.struct_data(id).src(),
            SymbolID::Const(id) => registry.const_data(id).src(),
            SymbolID::Global(id) => registry.global_data(id).src(),
        }
    }

    fn vis(self, registry: &Registry<'hir, '_>) -> ast::Vis {
        match self {
            SymbolID::Proc(id) => registry.proc_data(id).vis,
            SymbolID::Enum(id) => registry.enum_data(id).vis,
            SymbolID::Struct(id) => registry.struct_data(id).vis,
            SymbolID::Const(id) => registry.const_data(id).vis,
            SymbolID::Global(id) => registry.global_data(id).vis,
        }
    }
}

impl<'hir> VariableID<'hir> {
    pub fn desc(self) -> &'static str {
        match self {
            VariableID::Param(_) => "parameter",
            VariableID::Local(_) => "local",
            VariableID::Bind(_) => "binding",
        }
    }
}

pub fn check_find_enum_variant<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    enum_id: hir::EnumID<'hir>,
    name: ast::Name,
) -> Option<ID<hir::Variant<'hir>>> {
    let enum_data = ctx.registry.enum_data(enum_id);
    for (idx, variant) in enum_data.variants.iter().enumerate() {
        if variant.name.id == name.id {
            return Some(hir::VariantID::new_raw(idx));
        }
    }
    let src = ctx.src(name.range);
    let enum_src = enum_data.src();
    let name = ctx.name(name.id);
    let enum_name = ctx.name(enum_data.name.id);
    err::scope_enum_variant_not_found(&mut ctx.emit, src, enum_src, name, enum_name);
    None
}

pub fn check_find_struct_field<'hir>(
    ctx: &mut HirCtx<'hir, '_, '_>,
    struct_id: hir::StructID<'hir>,
    name: ast::Name,
) -> Option<ID<hir::Field<'hir>>> {
    let struct_data = ctx.registry.struct_data(struct_id);
    for (idx, field) in struct_data.fields.iter().enumerate() {
        if field.name.id == name.id {
            return Some(hir::FieldID::new_raw(idx));
        }
    }
    let src = ctx.src(name.range);
    let struct_src = struct_data.src();
    let name = ctx.name(name.id);
    let struct_name = ctx.name(struct_data.name.id);
    err::scope_struct_field_not_found(&mut ctx.emit, src, struct_src, name, struct_name);
    None
}
