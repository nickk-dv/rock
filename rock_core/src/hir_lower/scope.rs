use super::context::{HirCtx, Registry};
use super::pass_5::Expectation;
use crate::ast;
use crate::error::{ErrorWarningBuffer, SourceRange};
use crate::errors as err;
use crate::hir;
use crate::intern::NameID;
use crate::session::{self, ModuleID, Session};
use crate::text::TextRange;
use rustc_hash::{FxBuildHasher, FxHashMap};

pub struct Scope<'hir> {
    pub origin: ModuleID,
    pub global: GlobalScope,
    pub local: LocalScope<'hir>,
    pub poly: PolyScope,
    pub infer: InferScope<'hir>,
}

//==================== GLOBAL SCOPE ====================

pub struct GlobalScope {
    modules: Vec<ModuleScope>,
}

pub struct ModuleScope {
    name_id: NameID,
    pub symbols: FxHashMap<NameID, Symbol>,
}

#[derive(Copy, Clone)]
pub enum Symbol {
    Defined(SymbolID),
    Imported(SymbolID, TextRange, bool),
    ImportedModule(ModuleID, TextRange, bool),
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
    pub proc_id: Option<hir::ProcID>,
    pub return_expect: Expectation<'hir>,
    params: &'hir [hir::Param<'hir>],
    pub params_was_used: Vec<bool>,
    blocks: Vec<BlockData>,
    variables: Vec<hir::Variable<'hir>>,
    variables_in_scope: Vec<hir::VariableID>,
    defer_blocks_in_scope: Vec<hir::Block<'hir>>,
    discard_id: NameID,
}

#[derive(Copy, Clone)]
pub enum LocalVariableID {
    Param(hir::ParamID),
    Variable(hir::VariableID),
}

#[derive(Copy, Clone)]
pub struct BlockData {
    var_count: u32,
    defer_count: u32,
    status: BlockStatus,
    diverges: Diverges,
    pub for_idx_change: Option<(hir::VariableID, hir::IntType, hir::BinOp)>,
    pub for_value_change: Option<(hir::VariableID, hir::IntType)>,
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

#[derive(Copy, Clone)]
pub enum PolyScope {
    None,
    Proc(hir::ProcID),
    Enum(hir::EnumID),
    Struct(hir::StructID),
}

pub struct InferScope<'hir> {
    types: Vec<hir::Type<'hir>>,
}

#[derive(Copy, Clone)]
pub struct InferContext {
    offset: usize,
    count: usize,
}

//==================== SCOPES IMPL ====================

impl<'hir> Scope<'hir> {
    pub fn new(session: &mut Session) -> Scope<'hir> {
        let discard_id = session.intern_name.intern("_");
        Scope {
            origin: ModuleID::dummy(),
            global: GlobalScope::new(session),
            local: LocalScope::new(discard_id),
            poly: PolyScope::None,
            infer: InferScope::new(),
        }
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
        let origin = self.global.module(self.origin);
        let existing = match origin.symbols.get(&name.id).copied() {
            Some(symbol) => symbol,
            None => return Ok(()),
        };
        let name_src = SourceRange::new(self.origin, name.range);
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
        let name_src = SourceRange::new(self.origin, name.range);
        let existing = self.var_src(existing);
        let name = session.intern_name.get(name.id);
        err::scope_name_already_defined(emit, name_src, existing, name);
        Err(())
    }

    fn symbol_src(&self, symbol: Symbol, registry: &Registry<'hir, '_>) -> SourceRange {
        match symbol {
            Symbol::Defined(symbol_id) => symbol_id.src(registry),
            Symbol::Imported(_, import, _) => SourceRange::new(self.origin, import),
            Symbol::ImportedModule(_, import, _) => SourceRange::new(self.origin, import),
        }
    }

    pub fn var_src(&self, local_var_id: LocalVariableID) -> SourceRange {
        let range = match local_var_id {
            LocalVariableID::Param(id) => self.local.param(id).name.range,
            LocalVariableID::Variable(id) => self.local.variable(id).name.range,
        };
        SourceRange::new(self.origin, range)
    }
}

impl GlobalScope {
    fn new(session: &Session) -> GlobalScope {
        let mut modules = Vec::with_capacity(session.module.ids().count());

        for module_id in session.module.ids() {
            let module = session.module.get(module_id);
            let ast = module.ast.as_ref().unwrap();

            let mut symbol_count = 0;
            for item in ast.items {
                symbol_count += 1;
                if let ast::Item::Import(import) = item {
                    symbol_count += import.symbols.len();
                }
            }

            modules.push(ModuleScope {
                name_id: module.name_id,
                symbols: FxHashMap::with_capacity_and_hasher(symbol_count, FxBuildHasher),
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
        &mut self,
        origin_id: ModuleID,
        target_id: ModuleID,
        name: ast::Name,
        session: &Session,
        registry: &Registry,
        emit: &mut ErrorWarningBuffer,
    ) -> Result<SymbolOrModule, ()> {
        let target = self.module_mut(target_id);

        match target.symbols.get_mut(&name.id) {
            Some(Symbol::Defined(symbol_id)) => {
                return match symbol_id.vis(registry) {
                    hir::Vis::Public => Ok(SymbolOrModule::Symbol(*symbol_id)),
                    hir::Vis::Package => {
                        let origin_package_id = session.module.get(origin_id).origin;
                        let target_package_id = session.module.get(target_id).origin;

                        if origin_package_id == target_package_id {
                            Ok(SymbolOrModule::Symbol(*symbol_id))
                        } else {
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
                        }
                    }
                    hir::Vis::Private => {
                        if origin_id == target_id {
                            Ok(SymbolOrModule::Symbol(*symbol_id))
                        } else {
                            let name_src = SourceRange::new(origin_id, name.range);
                            let defined_src = symbol_id.src(registry);
                            let name = session.intern_name.get(name.id);
                            err::scope_symbol_private_vis(
                                emit,
                                name_src,
                                defined_src,
                                name,
                                symbol_id.desc(),
                            );
                            Err(())
                        }
                    }
                };
            }
            Some(Symbol::Imported(symbol_id, _, used)) => {
                if origin_id == target_id {
                    *used = true;
                    return Ok(SymbolOrModule::Symbol(*symbol_id));
                }
            }
            Some(Symbol::ImportedModule(module_id, _, used)) => {
                if origin_id == target_id {
                    *used = true;
                    return Ok(SymbolOrModule::Module(*module_id));
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

    pub fn find_defined_enum(&self, target_id: ModuleID, name_id: NameID) -> Option<hir::EnumID> {
        let target = self.module(target_id);
        match target.symbols.get(&name_id).copied() {
            Some(Symbol::Defined(SymbolID::Enum(id))) => Some(id),
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
    pub fn module(&self, module_id: ModuleID) -> &ModuleScope {
        &self.modules[module_id.index()]
    }
    #[inline]
    fn module_mut(&mut self, module_id: ModuleID) -> &mut ModuleScope {
        &mut self.modules[module_id.index()]
    }
}

impl<'hir> LocalScope<'hir> {
    fn new(discard_id: NameID) -> LocalScope<'hir> {
        LocalScope {
            proc_id: None,
            return_expect: Expectation::None,
            params: &[],
            params_was_used: Vec::with_capacity(32),
            blocks: Vec::with_capacity(32),
            variables: Vec::with_capacity(128),
            variables_in_scope: Vec::with_capacity(128),
            defer_blocks_in_scope: Vec::with_capacity(32),
            discard_id,
        }
    }

    pub fn reset(&mut self) {
        self.proc_id = None;
        self.return_expect = Expectation::None;
        self.params = &[];
        self.params_was_used.clear();
        self.blocks.clear();
        self.variables.clear();
        self.variables_in_scope.clear();
        self.defer_blocks_in_scope.clear();
    }

    pub fn set_proc_context(
        &mut self,
        proc_id: Option<hir::ProcID>,
        params: &'hir [hir::Param<'hir>],
        return_expect: Expectation<'hir>,
    ) {
        self.proc_id = proc_id;
        self.return_expect = return_expect;
        self.params = params;
        self.params_was_used.resize(params.len(), false);
    }

    pub fn end_proc_context(&self) -> &[hir::Variable<'hir>] {
        &self.variables
    }

    pub fn start_block(&mut self, status: BlockStatus) {
        let diverges = self.blocks.last().map(|b| b.diverges).unwrap_or(Diverges::Maybe);
        let data = BlockData {
            var_count: 0,
            defer_count: 0,
            status,
            diverges,
            for_idx_change: self.blocks.last().map(|b| b.for_idx_change).unwrap_or(None),
            for_value_change: self.blocks.last().map(|b| b.for_value_change).unwrap_or(None),
        };
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

    pub fn add_variable(&mut self, var: hir::Variable<'hir>) -> hir::VariableID {
        let var_id = hir::VariableID::new(self.variables.len());
        self.variables.push(var);
        if var.name.id != self.discard_id {
            self.variables_in_scope.push(var_id);
            self.current_block_mut().var_count += 1;
        }
        var_id
    }

    pub fn add_defer_block(&mut self, block: hir::Block<'hir>) {
        self.defer_blocks_in_scope.push(block);
        self.current_block_mut().defer_count += 1;
    }

    pub fn find_variable(
        &mut self,
        name_id: NameID,
        set_usage_flag: bool,
    ) -> Option<LocalVariableID> {
        for (idx, param) in self.params.iter().enumerate() {
            if name_id == param.name.id {
                if set_usage_flag {
                    self.params_was_used[idx] = true;
                }
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
    pub fn current_block(&self) -> BlockData {
        self.blocks.last().copied().unwrap()
    }
    #[inline]
    pub fn current_block_mut(&mut self) -> &mut BlockData {
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
    pub fn temp_override(&mut self, new: PolyScope) -> PolyScope {
        let prev = *self;
        *self = new;
        prev
    }
    pub fn find_poly_param<'hir>(
        self,
        name_id: NameID,
        registry: &Registry,
    ) -> Option<(hir::Type<'hir>, TextRange)> {
        let poly_params = match self {
            PolyScope::None => return None,
            PolyScope::Proc(id) => registry.proc_data(id).poly_params?,
            PolyScope::Enum(id) => registry.enum_data(id).poly_params?,
            PolyScope::Struct(id) => registry.struct_data(id).poly_params?,
        };
        for (poly_idx, param) in poly_params.iter().enumerate() {
            if name_id == param.id {
                let ty = match self {
                    PolyScope::None => unreachable!(),
                    PolyScope::Proc(id) => hir::Type::PolyProc(id, poly_idx),
                    PolyScope::Enum(id) => hir::Type::PolyEnum(id, poly_idx),
                    PolyScope::Struct(id) => hir::Type::PolyStruct(id, poly_idx),
                };
                return Some((ty, param.range));
            }
        }
        None
    }
}

impl<'hir> InferScope<'hir> {
    fn new() -> InferScope<'hir> {
        InferScope { types: Vec::with_capacity(64) }
    }
    pub fn start_context(&mut self, params: Option<&[ast::Name]>) -> (InferContext, bool) {
        let offset = self.types.len();
        let count = params.map_or(0, |p| p.len());
        for _ in 0..count {
            self.types.push(hir::Type::Unknown);
        }
        (InferContext { offset, count }, count > 0)
    }
    pub fn end_context(&mut self, ctx: InferContext) {
        while self.types.len() > ctx.offset {
            self.types.pop();
        }
    }
    pub fn resolve(&mut self, ctx: InferContext, poly_idx: usize, ty: hir::Type<'hir>) {
        let poly_type = &mut self.types[ctx.offset + poly_idx];
        if poly_type.is_unknown() {
            *poly_type = ty;
        }
    }
    pub fn inferred(&self, ctx: InferContext) -> &[hir::Type<'hir>] {
        &self.types[ctx.offset..ctx.offset + ctx.count]
    }
    pub fn inferred_mut(&mut self, ctx: InferContext) -> &mut [hir::Type<'hir>] {
        &mut self.types[ctx.offset..ctx.offset + ctx.count]
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

pub fn find_core_proc(ctx: &mut HirCtx, module: &str, name: &str) -> Result<hir::ProcID, ()> {
    let module_id = find_core_module(ctx, module).ok_or(())?;
    let name_id = ctx.session.intern_name.intern(name);
    let proc_id = ctx.scope.global.find_defined_proc(module_id, name_id);
    if proc_id.is_none() {
        err::scope_core_symbol_not_found(&mut ctx.emit, name, "procedure");
    }
    proc_id.ok_or(())
}

pub fn find_core_enum(ctx: &mut HirCtx, module: &str, name: &str) -> Result<hir::EnumID, ()> {
    let module_id = find_core_module(ctx, module).ok_or(())?;
    let name_id = ctx.session.intern_name.intern(name);
    let enum_id = ctx.scope.global.find_defined_enum(module_id, name_id);
    if enum_id.is_none() {
        err::scope_core_symbol_not_found(&mut ctx.emit, name, "enum");
    }
    enum_id.ok_or(())
}

pub fn find_core_struct(ctx: &mut HirCtx, module: &str, name: &str) -> Result<hir::StructID, ()> {
    let module_id = find_core_module(ctx, module).ok_or(())?;
    let name_id = ctx.session.intern_name.intern(name);
    let struct_id = ctx.scope.global.find_defined_struct(module_id, name_id);
    if struct_id.is_none() {
        err::scope_core_symbol_not_found(&mut ctx.emit, name, "struct");
    }
    struct_id.ok_or(())
}

fn find_core_module(ctx: &mut HirCtx, name: &str) -> Option<ModuleID> {
    let package = ctx.session.graph.package(session::CORE_PACKAGE_ID);
    let name_id = ctx.session.intern_name.get_id(name)?;

    let module_id = match package.src.find(ctx.session, name_id) {
        session::ModuleOrDirectory::None => None,
        session::ModuleOrDirectory::Module(module_id) => Some(module_id),
        session::ModuleOrDirectory::Directory(_) => None,
    };

    if module_id.is_none() {
        err::scope_core_symbol_not_found(&mut ctx.emit, name, "module");
    }
    module_id
}

pub fn check_find_enum_variant(
    ctx: &mut HirCtx,
    enum_id: hir::EnumID,
    name: ast::Name,
) -> Option<hir::VariantID> {
    let data = ctx.registry.enum_data(enum_id);

    let variant_id = data
        .variants
        .iter()
        .enumerate()
        .find(|(_, v)| v.name.id == name.id)
        .map(|(i, _)| hir::VariantID::new(i));

    if variant_id.is_none() {
        let src = ctx.src(name.range);
        let enum_src = data.src();
        let name = ctx.name(name.id);
        let enum_name = ctx.name(data.name.id);
        err::scope_enum_variant_not_found(&mut ctx.emit, src, enum_src, name, enum_name);
    }
    variant_id
}

pub fn check_find_struct_field(
    ctx: &mut HirCtx,
    struct_id: hir::StructID,
    name: ast::Name,
) -> Option<hir::FieldID> {
    let data = ctx.registry.struct_data(struct_id);

    let field_id = data
        .fields
        .iter()
        .enumerate()
        .find(|(_, f)| f.name.id == name.id)
        .map(|(i, _)| hir::FieldID::new(i));

    if field_id.is_none() {
        let src = ctx.src(name.range);
        let struct_src = data.src();
        let name = ctx.name(name.id);
        let struct_name = ctx.name(data.name.id);
        err::scope_struct_field_not_found(&mut ctx.emit, src, struct_src, name, struct_name);
    }
    let field_id = field_id?;
    let field = data.field(field_id);

    match field.vis {
        hir::Vis::Public => {}
        hir::Vis::Package => {
            let origin_package_id = ctx.session.module.get(ctx.scope.origin).origin;
            let struct_package_id = ctx.session.module.get(data.origin_id).origin;

            if origin_package_id != struct_package_id {
                let name_src = ctx.src(name.range);
                let defined_src = SourceRange::new(data.origin_id, field.name.range);
                let field_name = ctx.name(field.name.id);
                err::scope_symbol_package_vis(
                    &mut ctx.emit,
                    name_src,
                    defined_src,
                    field_name,
                    "field",
                );
            }
        }
        hir::Vis::Private => {
            if ctx.scope.origin != data.origin_id {
                let name_src = ctx.src(name.range);
                let defined_src = SourceRange::new(data.origin_id, field.name.range);
                let field_name = ctx.name(field.name.id);
                err::scope_symbol_private_vis(
                    &mut ctx.emit,
                    name_src,
                    defined_src,
                    field_name,
                    "field",
                );
            }
        }
    }

    Some(field_id)
}
