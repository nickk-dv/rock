use crate::hir;
use crate::intern::InternID;

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
    pub in_loop: bool,
    pub in_defer: bool,
    local_count: u32,
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
        &self.data.params[id.index()]
    }
    pub fn get_block(&self) -> &BlockData {
        self.blocks.last().expect("block exists")
    }

    pub fn push_block(&mut self) {
        self.blocks.push(BlockData {
            in_loop: false,
            in_defer: false,
            local_count: 0,
        });
    }

    pub fn push_local(&mut self, local: &'hir hir::Local<'hir>) {
        let local_id = hir::LocalID::new(self.locals.len());
        self.locals.push(local);
        self.locals_in_scope.push(local_id);
        self.blocks.last_mut().expect("block exists").local_count += 1;
    }

    pub fn pop_block(&mut self) {
        let block = self.blocks.pop().expect("block exists");
        for _ in 0..block.local_count {
            self.locals_in_scope.pop();
        }
    }

    pub fn find_variable(&self, id: InternID) -> Option<VariableID> {
        for (idx, param) in self.data.params.iter().enumerate() {
            if param.name.id == id {
                let id = hir::ProcParamID::new(idx);
                return Some(VariableID::Param(id));
            }
        }
        for local_id in self.locals_in_scope.iter().cloned() {
            if self.get_local(local_id).name.id == id {
                return Some(VariableID::Local(local_id));
            }
        }
        None
    }
}
