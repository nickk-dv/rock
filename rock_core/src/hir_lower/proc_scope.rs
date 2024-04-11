use crate::hir;
use crate::intern::InternID;
use crate::text::TextRange;

//@need some way to recognize if loop was started within in defer
// to allow break / continue to be used there
// thats part of current design, otherwise break and continue cannot be part of defer block

//@re-use same proc scope to avoid frequent re-alloc (not important yet)
pub struct ProcScope<'hir, 'check> {
    data: &'check hir::ProcData<'hir>,
    blocks: Vec<BlockData>,
    locals: Vec<&'hir hir::Local<'hir>>,
    locals_in_scope: Vec<hir::LocalID>,
}

pub struct BlockData {
    local_count: u32,
    loop_status: LoopStatus,
    defer_status: DeferStatus,
}

#[derive(Copy, Clone)]
enum LoopStatus {
    None,
    Inside,
}

#[derive(Copy, Clone)]
enum DeferStatus {
    None,
    Inside(TextRange),
}

pub enum VariableID {
    Local(hir::LocalID),
    Param(hir::ProcParamID),
}

impl<'hir, 'check> ProcScope<'hir, 'check> {
    pub fn new(data: &'check hir::ProcData<'hir>) -> Self {
        ProcScope {
            data,
            blocks: Vec::new(),
            locals: Vec::new(),
            locals_in_scope: Vec::new(),
        }
    }

    pub fn finish(self) -> Vec<&'hir hir::Local<'hir>> {
        self.locals
    }
    pub fn origin(&self) -> hir::ScopeID {
        self.data.origin_id
    }
    pub fn data(&self) -> &hir::ProcData<'hir> {
        self.data
    }
    pub fn get_local(&self, id: hir::LocalID) -> &hir::Local<'hir> {
        self.locals[id.index()]
    }
    pub fn get_param(&self, id: hir::ProcParamID) -> &hir::ProcParam<'hir> {
        &self.data.param(id)
    }

    pub fn push_block(&mut self, enter_loop: bool, enter_defer: Option<TextRange>) {
        let loop_status = if enter_loop {
            LoopStatus::Inside
        } else {
            self.blocks
                .last()
                .map_or(LoopStatus::None, |last| last.loop_status)
        };

        let defer_status = if let Some(last) = self.blocks.last() {
            if let DeferStatus::Inside(..) = last.defer_status {
                last.defer_status
            } else {
                enter_defer.map_or(DeferStatus::None, |range| DeferStatus::Inside(range))
            }
        } else {
            enter_defer.map_or(DeferStatus::None, |range| DeferStatus::Inside(range))
        };

        self.blocks.push(BlockData {
            local_count: 0,
            loop_status,
            defer_status,
        });
    }

    pub fn is_inside_loop(&self) -> bool {
        let status = self.blocks.last().expect("block exists").loop_status;
        matches!(status, LoopStatus::Inside)
    }

    pub fn is_inside_defer(&self) -> Option<TextRange> {
        let status = self.blocks.last().expect("block exists").defer_status;
        match status {
            DeferStatus::None => None,
            DeferStatus::Inside(range) => Some(range),
        }
    }

    pub fn push_local(&mut self, local: &'hir hir::Local<'hir>) -> hir::LocalID {
        let local_id = hir::LocalID::new(self.locals.len());
        self.locals.push(local);
        self.locals_in_scope.push(local_id);
        self.blocks.last_mut().expect("block exists").local_count += 1;
        local_id
    }

    pub fn pop_block(&mut self) {
        let block = self.blocks.pop().expect("block exists");
        for _ in 0..block.local_count {
            self.locals_in_scope.pop();
        }
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
}
