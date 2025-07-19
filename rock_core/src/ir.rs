use crate::hir::{BinOp, UnOp};
use std::mem;

struct IR<'ir> {
    procs: Vec<ProcData<'ir>>,
    structs: Vec<StructData<'ir>>,
}
struct ProcData<'ir> {
    params: &'ir [ParamData],
}
struct StructData<'ir> {
    fields: &'ir [FieldData],
}
struct ParamData {}
struct FieldData {}

struct Writer {
    expr_data: Vec<u32>,
}
struct Body<'ir> {
    expr_data: &'ir [u32],
}

crate::define_id!(pub ProcID);
crate::define_id!(pub EnumID);
crate::define_id!(pub StructID);
crate::define_id!(pub ConstID);
crate::define_id!(pub GlobalID);
crate::define_id!(pub ImportID);

crate::define_id!(pub ParamID);
crate::define_id!(pub VariableID);
crate::define_id!(pub ExprID);

crate::define_id!(pub VariantID);

#[repr(u8)]
pub enum ExprKind {
    Param,
    Local,
    Global,
    CallDirect,
    StructInit,
    Unary,
    Binary,
}

#[allow(unsafe_code)]
impl Body<'_> {
    pub fn expr_kind(&self, id: ExprID) -> ExprKind {
        let kind = (self.expr_data[id.index()] & 0xFF) as u8;
        unsafe { mem::transmute(kind) }
    }
    pub fn param(&self, id: ExprID) -> ParamID {
        ParamID(self.expr_data[id.index()] >> 8)
    }
    pub fn local(&self, id: ExprID) -> VariableID {
        VariableID(self.expr_data[id.index()] >> 8)
    }
    pub fn global(&self, id: ExprID) -> GlobalID {
        GlobalID(self.expr_data[id.index()] >> 8)
    }
    pub fn call_direct(&self, id: ExprID, ir: &IR) -> (ProcID, &[ExprID]) {
        let proc_id = ProcID(self.expr_data[id.index() + 1]);
        let start = id.index() + 2;
        let count = ir.procs[proc_id.index()].params.len();
        let input = &self.expr_data[start..start + count];
        (proc_id, unsafe { mem::transmute(input) })
    }
    pub fn struct_init(&self, id: ExprID, ir: &IR) -> (StructID, &[ExprID]) {
        let struct_id = StructID(self.expr_data[id.index() + 1]);
        let start = id.index() + 2;
        let count = ir.structs[struct_id.index()].fields.len();
        let input = &self.expr_data[start..start + count];
        (struct_id, unsafe { mem::transmute(input) })
    }
    pub fn unary(&self, id: ExprID) -> (UnOp, ExprID, Option<u32>) {
        let tag = self.expr_data[id.index()];
        let op: UnOp = unsafe { mem::transmute(((tag >> 16) & 0xFF) as u8) };
        let rhs = ExprID(self.expr_data[id.index() + 1]);
        let array_op = (tag >> 8) & 0xFF != 0;
        let array_len = array_op.then(|| self.expr_data[id.index() + 2]);
        (op, rhs, array_len)
    }
    pub fn binary(&self, id: ExprID) -> (BinOp, ExprID, ExprID, Option<u32>) {
        let tag = self.expr_data[id.index()];
        let op: BinOp = unsafe { mem::transmute(self.expr_data[id.index() + 1]) };
        let lhs = ExprID(self.expr_data[id.index() + 2]);
        let rhs = ExprID(self.expr_data[id.index() + 3]);
        let array_op = (tag >> 8) & 0xFF != 0;
        let array_len = array_op.then(|| self.expr_data[id.index() + 4]);
        (op, lhs, rhs, array_len)
    }
}

#[allow(unsafe_code)]
impl Writer {
    pub fn param(&mut self, param_id: ParamID) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Param as u32 | (param_id.0 << 8));
        id
    }
    pub fn local(&mut self, var_id: VariableID) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Local as u32 | (var_id.0 << 8));
        id
    }
    pub fn global(&mut self, global_id: GlobalID) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Global as u32 | (global_id.0 << 8));
        id
    }
    pub fn call_direct(&mut self, proc_id: ProcID, input: &[ExprID]) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::CallDirect as u32);
        self.expr_data.push(proc_id.0);
        for expr_id in input.iter().copied() {
            self.expr_data.push(expr_id.0);
        }
        id
    }
    pub fn struct_init(&mut self, struct_id: StructID, input: &[ExprID]) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::StructInit as u32);
        self.expr_data.push(struct_id.0);
        for expr_id in input.iter().copied() {
            self.expr_data.push(expr_id.0);
        }
        id
    }
    pub fn unary(&mut self, op: UnOp, rhs: ExprID, arr_len: Option<u32>) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        let array_op = arr_len.is_some();
        self.expr_data.push(ExprKind::Unary as u32 | (array_op as u32) << 8 | (op as u32) << 16);
        self.expr_data.push(rhs.0);
        if let Some(len) = arr_len {
            self.expr_data.push(len);
        }
        id
    }
    pub fn binary(&mut self, op: BinOp, lhs: ExprID, rhs: ExprID, arr_len: Option<u32>) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        let array_op = arr_len.is_some();
        self.expr_data.push(ExprKind::Binary as u32 | (array_op as u32) << 8);
        self.expr_data.push(unsafe { mem::transmute(op) });
        self.expr_data.push(lhs.0);
        self.expr_data.push(rhs.0);
        if let Some(len) = arr_len {
            self.expr_data.push(len);
        }
        id
    }
}
