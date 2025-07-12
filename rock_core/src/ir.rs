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
}

#[allow(unsafe_code)]
impl Body<'_> {
    pub fn expr_kind(&self, id: ExprID) -> ExprKind {
        let kind = self.expr_data[id.index()] as u8;
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
}

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
}
