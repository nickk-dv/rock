use super::hir_build::{HirData, HirEmit};
use super::pass_5::Expectation;
use crate::error::{Info, SourceRange, WarningComp};
use crate::hir;
use crate::intern::InternID;
use crate::session::ModuleID;
use crate::text::TextRange;

//@re-use same proc scope to avoid frequent re-alloc 26.05.24
// lifetime problems, separate vectors into separate re-usable mutable reference?
// not a big deal until perf of check is important
// not re-using will re allocate each of 3 vectors multiple times for EACH procedure being typechecked
pub struct ProcScope<'hir, 'check> {
    data: &'check hir::ProcData<'hir>,
    return_expect: Expectation<'hir>,
    blocks: Vec<BlockData>,
    locals: Vec<&'hir hir::Local<'hir>>,
    locals_in_scope: Vec<hir::LocalID>,
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

pub enum VariableID {
    Local(hir::LocalID),
    Param(hir::ParamID),
}

impl<'hir, 'check> ProcScope<'hir, 'check> {
    pub fn new(data: &'check hir::ProcData<'hir>, return_expect: Expectation<'hir>) -> Self {
        ProcScope {
            data,
            return_expect,
            blocks: Vec::new(),
            locals: Vec::new(),
            locals_in_scope: Vec::new(),
        }
    }

    pub fn finish_locals(&self) -> &[&'hir hir::Local<'hir>] {
        self.locals.as_slice()
    }
    pub fn origin(&self) -> ModuleID {
        self.data.origin_id
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
    pub fn get_local(&self, id: hir::LocalID) -> &hir::Local<'hir> {
        self.locals[id.index()]
    }
    pub fn get_param(&self, id: hir::ParamID) -> &hir::Param<'hir> {
        self.data.param(id)
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

    pub fn push_local(&mut self, local: &'hir hir::Local<'hir>) -> hir::LocalID {
        let local_id = hir::LocalID::new(self.locals.len());
        self.locals.push(local);
        self.locals_in_scope.push(local_id);
        self.blocks.last_mut().expect("block exists").local_count += 1;
        local_id
    }

    pub fn find_variable(&self, id: InternID) -> Option<VariableID> {
        if let Some((param_id, _)) = self.data.find_param(id) {
            return Some(VariableID::Param(param_id));
        }
        for local_id in self.locals_in_scope.iter().cloned() {
            if self.get_local(local_id).name.id == id {
                return Some(VariableID::Local(local_id));
            }
        }
        None
    }

    pub fn check_stmt_diverges(
        &mut self,
        hir: &HirData<'hir, '_>,
        emit: &mut HirEmit<'hir>,
        will_diverge: bool,
        stmt_range: TextRange,
    ) -> bool {
        let diverges = &mut self.blocks.last_mut().expect("block exists").diverges;
        match *diverges {
            Diverges::Maybe => {
                if will_diverge {
                    *diverges = Diverges::Always(stmt_range);
                }
                false
            }
            Diverges::Always(diverge_range) => {
                *diverges = Diverges::AlwaysWarned;

                emit.warning(WarningComp::new(
                    "unreachable statement",
                    SourceRange::new(self.origin(), stmt_range),
                    Info::new(
                        "all statements after this are unreachable",
                        SourceRange::new(self.origin(), diverge_range),
                    ),
                ));
                true
            }
            Diverges::AlwaysWarned => true,
        }
    }
}
