use super::pass_5::Expectation;
use crate::ast;
use crate::hir;
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

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Symbol<'hir> {
    Defined { kind: SymbolID<'hir> },
    Imported { kind: SymbolID<'hir>, import: TextRange },
    Module { module_id: ModuleID, import: TextRange },
}

#[derive(Copy, Clone)]
pub enum SymbolID<'hir> {
    Proc(hir::ProcID<'hir>),
    Enum(hir::EnumID<'hir>),
    Struct(hir::StructID<'hir>),
    Const(hir::ConstID<'hir>),
    Global(hir::GlobalID<'hir>),
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
}

#[derive(Copy, Clone)]
pub enum BlockStatus {
    None,
    Loop(TextRange),
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
    pub fn new(session: &Session) -> Scope<'hir> {
        Scope {
            origin_id: ModuleID::dummy(),
            local: LocalScope::new(),
            global: GlobalScope::new(session),
        }
    }

    pub fn set_origin(&mut self, origin_id: ModuleID) {
        self.origin_id = origin_id;
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

    pub fn start_block(&mut self, status: BlockStatus) {
        let data = BlockData {
            local_count: 0,
            bind_count: 0,
            status,
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
                BlockStatus::Loop(_) => {}
                BlockStatus::Defer(range) => return Some(range),
            }
        }
        None
    }

    pub fn find_prev_loop_before_defer(&self) -> (Option<TextRange>, Option<TextRange>) {
        let mut defer = None;
        for block in self.blocks.iter().rev() {
            match block.status {
                BlockStatus::None => {}
                BlockStatus::Loop(range) => return (Some(range), defer),
                BlockStatus::Defer(range) => defer = Some(range),
            }
        }
        (None, defer)
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
}
