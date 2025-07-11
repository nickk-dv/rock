use std::mem;

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
        unsafe { mem::transmute(self.expr_data[id.index()] as u8) }
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
    //@decode poly
    //@need iterator over input exprs, expect known proc param count
    pub fn call_direct(&self, id: ExprID) -> ProcID {
        ProcID(self.expr_data[id.index() + 1])
    }
    //@decode poly
    //@need iterator over input exprs, expect known proc param count
    pub fn struct_init(&self, id: ExprID) -> StructID {
        StructID(self.expr_data[id.index() + 1])
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
    //@encode poly
    pub fn call_direct(&mut self, proc_id: ProcID, input: &[ExprID]) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::CallDirect as u32);
        self.expr_data.push(proc_id.0);
        for expr_id in input.iter().copied() {
            self.expr_data.push(expr_id.0);
        }
        id
    }
    //@encode poly
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
