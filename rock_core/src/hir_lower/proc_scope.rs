use super::pass_5::Expectation;
use crate::hir;
use crate::intern::InternName;
use crate::session::ModuleID;
use crate::support::{IndexID, ID};
use crate::text::TextRange;

//@re-use same proc scope to avoid frequent re-alloc 26.05.24
// lifetime problems, separate vectors into separate re-usable mutable reference?
// not a big deal until perf of check is important
// not re-using will re allocate each of 3 vectors multiple times for EACH procedure being typechecked
pub struct ProcScope<'hir> {
    origin_id: ModuleID,
    params: &'hir [hir::Param<'hir>],
    return_expect: Expectation<'hir>,
    blocks: Vec<BlockData>,
    locals: Vec<hir::Local<'hir>>,
    locals_in_scope: Vec<hir::LocalID<'hir>>,
}

pub struct BlockData {
    local_count: u32,
    diverges: Diverges,
    loop_status: LoopStatus,
    defer_status: DeferStatus,
}

#[derive(Copy, Clone)]
pub enum BlockEnter {
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

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum LoopStatus {
    None,
    Inside,
    Inside_WithDefer,
}

#[derive(Copy, Clone)]
pub enum DeferStatus {
    None,
    Inside(TextRange),
}

impl Diverges {
    pub fn is_always(&self) -> bool {
        matches!(self, Diverges::Always(_) | Diverges::AlwaysWarned)
    }
}

pub enum VariableID<'hir> {
    Local(hir::LocalID<'hir>),
    Param(hir::ParamID<'hir>),
}

impl<'hir> ProcScope<'hir> {
    pub fn dummy() -> ProcScope<'hir> {
        ProcScope {
            origin_id: ModuleID::dummy(),
            params: &[],
            return_expect: Expectation::None,
            blocks: Vec::with_capacity(64),
            locals: Vec::with_capacity(64),
            locals_in_scope: Vec::with_capacity(64),
        }
    }

    pub fn reset_origin(&mut self, origin_id: ModuleID) {
        self.origin_id = origin_id;
    }

    pub fn reset(
        &mut self,
        origin_id: ModuleID,
        params: &'hir [hir::Param<'hir>],
        return_expect: Expectation<'hir>,
    ) {
        self.origin_id = origin_id;
        self.params = params;
        self.return_expect = return_expect;
        self.blocks.clear();
        self.locals.clear();
        self.locals_in_scope.clear();
    }

    pub fn finish_locals(&self) -> &[hir::Local<'hir>] {
        self.locals.as_slice()
    }
    pub fn origin(&self) -> ModuleID {
        self.origin_id
    }
    pub fn return_expect(&self) -> Expectation<'hir> {
        self.return_expect
    }
    pub fn loop_status(&self) -> LoopStatus {
        self.blocks.last().expect("block exists").loop_status
    }
    pub fn defer_status(&self) -> DeferStatus {
        self.blocks.last().expect("block exists").defer_status
    }
    pub fn diverges(&self) -> Diverges {
        self.blocks.last().expect("block exists").diverges
    }
    pub fn get_local(&self, id: hir::LocalID<'hir>) -> &hir::Local<'hir> {
        self.locals.id_get(id)
    }
    pub fn get_param(&self, id: hir::ParamID<'hir>) -> &hir::Param<'hir> {
        self.params.id_get(id)
    }

    pub fn push_block(&mut self, enter: BlockEnter) {
        let block_data = match enter {
            BlockEnter::None => BlockData {
                local_count: 0,
                diverges: self.inherit_diverges(),
                loop_status: self.inherit_loop_status(false, false),
                defer_status: self.inherit_defer_status(None),
            },
            BlockEnter::Loop => BlockData {
                local_count: 0,
                diverges: self.inherit_diverges(),
                loop_status: self.inherit_loop_status(true, false),
                defer_status: self.inherit_defer_status(None),
            },
            BlockEnter::Defer(range) => BlockData {
                local_count: 0,
                diverges: self.inherit_diverges(),
                loop_status: self.inherit_loop_status(false, true),
                defer_status: self.inherit_defer_status(Some(range)),
            },
        };
        self.blocks.push(block_data);
    }

    fn inherit_diverges(&self) -> Diverges {
        if let Some(last) = self.blocks.last() {
            last.diverges
        } else {
            Diverges::Maybe
        }
    }

    fn inherit_loop_status(&self, enter_loop: bool, enter_defer: bool) -> LoopStatus {
        if let Some(last) = self.blocks.last() {
            match last.loop_status {
                LoopStatus::None => {
                    if enter_loop {
                        LoopStatus::Inside
                    } else {
                        last.loop_status
                    }
                }
                LoopStatus::Inside => {
                    if enter_defer {
                        LoopStatus::Inside_WithDefer
                    } else {
                        last.loop_status
                    }
                }
                LoopStatus::Inside_WithDefer => {
                    if enter_loop {
                        LoopStatus::Inside
                    } else {
                        last.loop_status
                    }
                }
            }
        } else {
            LoopStatus::None
        }
    }

    fn inherit_defer_status(&self, enter_defer: Option<TextRange>) -> DeferStatus {
        if let Some(last) = self.blocks.last() {
            if let DeferStatus::Inside(..) = last.defer_status {
                last.defer_status
            } else {
                enter_defer.map_or(DeferStatus::None, DeferStatus::Inside)
            }
        } else {
            DeferStatus::None
        }
    }

    pub fn pop_block(&mut self) {
        let block = self.blocks.pop().expect("block exists");
        for _ in 0..block.local_count {
            self.locals_in_scope.pop();
        }
    }

    pub fn push_local(&mut self, local: hir::Local<'hir>) -> hir::LocalID<'hir> {
        let local_id = hir::LocalID::new(&self.locals);
        self.locals.push(local);
        self.locals_in_scope.push(local_id);
        self.blocks.last_mut().expect("block exists").local_count += 1;
        local_id
    }

    pub fn find_variable(&self, id: ID<InternName>) -> Option<VariableID<'hir>> {
        if let Some(param_id) = self.find_param(id) {
            return Some(VariableID::Param(param_id));
        }
        for local_id in self.locals_in_scope.iter().cloned() {
            if self.get_local(local_id).name.id == id {
                return Some(VariableID::Local(local_id));
            }
        }
        None
    }

    fn find_param(&self, id: ID<InternName>) -> Option<hir::ParamID<'hir>> {
        for (idx, param) in self.params.iter().enumerate() {
            if param.name.id == id {
                return Some(hir::ParamID::new_raw(idx));
            }
        }
        None
    }

    pub fn check_stmt_diverges(&mut self, will_diverge: bool, stmt_range: TextRange) -> Diverges {
        let diverges = &mut self.blocks.last_mut().expect("block exists").diverges;
        match *diverges {
            Diverges::Maybe => {
                if will_diverge {
                    *diverges = Diverges::Always(stmt_range);
                }
            }
            Diverges::Always(_) => {
                *diverges = Diverges::AlwaysWarned;
            }
            Diverges::AlwaysWarned => {}
        }
        *diverges
    }
}
