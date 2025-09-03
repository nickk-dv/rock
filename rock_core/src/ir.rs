#![allow(dead_code)] //@remove when rework is complete
use crate::ast;
use crate::hir::{self, BinOp, UnOp, Vis};
use crate::intern::InternPool;
use crate::session::ModuleID;
use crate::support::{Arena, BitSet};
use crate::text::TextRange;
use std::mem;

pub struct IR<'ir> {
    pub arena: Arena<'ir>,
    pub types: InternPool<TypeID, Type<'ir>>,
    pub procs: Vec<ProcData<'ir>>,
    pub enums: Vec<EnumData<'ir>>,
    pub structs: Vec<StructData<'ir>>,
}

pub struct ProcData<'ir> {
    pub origin: ModuleID,
    pub flags: BitSet<hir::ProcFlag>,
    pub vis: Vis,
    pub name: ast::Name,
    pub poly: Option<&'ir [ast::Name]>,
    pub params: &'ir [Param],
    pub return_ty: TypeID,
    pub variables: &'ir [Variable],
    pub body: Body<'ir>,
}

#[derive(Copy, Clone)]
pub struct Param {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: TypeID,
    pub ty_range: TextRange,
    pub was_used: bool,
    pub kind: hir::ParamKind,
}

#[derive(Copy, Clone)]
pub struct Variable {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: TypeID,
    pub was_used: bool,
}

pub struct EnumData<'ir> {
    pub origin: ModuleID,
    pub flags: BitSet<hir::EnumFlag>,
    pub vis: Vis,
    pub name: ast::Name,
    pub poly: Option<&'ir [ast::Name]>,
    pub variants: &'ir [Variant<'ir>],
}

#[derive(Copy, Clone)]
pub struct Variant<'ir> {
    pub name: ast::Name,
    pub kind: hir::VariantKind,
    pub fields: &'ir [VariantField],
}

#[derive(Copy, Clone)]
pub struct VariantField {
    pub ty: TypeID,
    pub ty_range: TextRange,
}

pub struct StructData<'ir> {
    pub origin: ModuleID,
    pub flags: BitSet<hir::EnumFlag>,
    pub vis: Vis,
    pub name: ast::Name,
    pub poly: Option<&'ir [ast::Name]>,
    pub fields: &'ir [Field],
}

pub struct Field {
    pub vis: Vis,
    pub name: ast::Name,
    pub ty: TypeID,
    pub ty_range: TextRange,
}

#[must_use]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Type<'hir> {
    Error,
    Unknown,
    Char,
    Void,
    Never,
    Rawptr,
    UntypedChar,
    Int(hir::IntType),
    Float(hir::FloatType),
    Bool(hir::BoolType),
    String(hir::StringType),
    PolyProc(ProcID, usize),
    PolyEnum(EnumID, usize),
    PolyStruct(StructID, usize),
    Enum(EnumID, &'hir [TypeID]),
    Struct(StructID, &'hir [TypeID]),
    Reference(ast::Mut, TypeID),
    MultiReference(ast::Mut, TypeID),
    Procedure(&'hir ProcType<'hir>),
    ArraySlice(ast::Mut, TypeID),
    ArrayStatic(ArrayLen, TypeID),
    ArrayEnumerated(EnumID, TypeID),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum ArrayLen {
    Immediate(u64),
    ConstEval(hir::ConstEvalID),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ProcType<'hir> {
    pub flags: BitSet<hir::ProcFlag>,
    pub params: &'hir [ProcTypeParam],
    pub return_ty: TypeID,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ProcTypeParam {
    pub ty: TypeID,
    pub kind: hir::ParamKind,
}

pub struct Writer {
    expr_data: Vec<u32>,
}
pub struct Body<'ir> {
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
crate::define_id!(pub StmtID);
crate::define_id!(pub ExprID);
crate::define_id!(pub TypeID);
crate::define_id!(pub VariantID);

pub enum StmtKind {
    Break,
    Continue,
    Return,
    Loop,
    Local,
    Assign,
    ExprSemi,
    ExprTail,
}

pub enum ExprKind {
    Error,
    Block,
    SliceField,
    Slice,
    Cast,
    Param,
    Local,
    Global,
    CallDirect,
    StructInit,
    ArrayInit,
    ArrayRepeat,
    Deref,
    Address,
    Unary,
    Binary,
}

pub enum LocalInit {
    Init(ExprID),
    Zeroed,
    Undefined,
}

pub enum AssignOp {
    Assign,
    Binary(BinOp, Option<u32>),
}

#[derive(Copy, Clone)]
pub struct SliceFieldAccess {
    pub deref: Option<ast::Mut>,
    pub field: SliceField,
}

#[derive(Copy, Clone)]
pub enum SliceField {
    Ptr,
    Len,
}

#[derive(Copy, Clone)]
pub struct SliceAccess {
    pub deref: Option<ast::Mut>,
    pub kind: SliceKind,
    pub op_call: ExprID,
}

#[derive(Copy, Clone)]
pub enum SliceKind {
    Slice(ast::Mut),
    Array,
}

const LOCAL_INIT_ZEROED: u32 = u32::MAX - 1;
const LOCAL_INIT_UNDEFINED: u32 = u32::MAX;

macro_rules! read_value_bool {
    ($code:expr, $at:expr) => {
        (($code >> $at) & 0b1) == 1
    };
}
macro_rules! read_value_bits {
    ($code:expr, $at:expr, $typ:ty, $repr:ty) => {
        unsafe { mem::transmute::<$repr, $typ>((($code >> $at) & (<$repr>::MAX as u32)) as $repr) }
    };
}

#[allow(unsafe_code)]
impl Body<'_> {
    #[inline(always)]
    fn read_slice<T>(&self, start: usize, count: usize) -> &[T] {
        unsafe { mem::transmute(&self.expr_data[start..start + count]) }
    }
    #[inline(always)]
    fn read_block(&self, offset: usize) -> &[StmtID] {
        self.read_slice(offset + 1, self.expr_data[offset] as usize)
    }

    pub fn stmt_kind(&self, id: StmtID) -> StmtKind {
        let tag = self.expr_data[id.index()];
        read_value_bits!(tag, 0, StmtKind, u8)
    }
    pub fn expr_kind(&self, id: ExprID) -> ExprKind {
        let tag = self.expr_data[id.index()];
        read_value_bits!(tag, 0, ExprKind, u8)
    }

    pub fn stmt_return(&self, id: StmtID) -> Option<ExprID> {
        let tag = self.expr_data[id.index()];
        if read_value_bool!(tag, 8) {
            Some(ExprID(self.expr_data[id.index() + 1]))
        } else {
            None
        }
    }
    pub fn stmt_loop(&self, id: StmtID) -> &[StmtID] {
        self.read_block(id.index() + 1)
    }
    pub fn stmt_local(&self, id: StmtID) -> (VariableID, LocalInit) {
        let var_id = VariableID(self.expr_data[id.index()] >> 8);
        let init = match self.expr_data[id.index() + 1] {
            LOCAL_INIT_ZEROED => LocalInit::Zeroed,
            LOCAL_INIT_UNDEFINED => LocalInit::Undefined,
            value => LocalInit::Init(ExprID(value)),
        };
        (var_id, init)
    }
    pub fn stmt_assign(&mut self, id: StmtID) -> (ExprID, ExprID, AssignOp) {
        let tag = self.expr_data[id.index()];
        let lhs = ExprID(self.expr_data[id.index() + 1]);
        let rhs = ExprID(self.expr_data[id.index() + 2]);
        let op = if read_value_bool!(tag, 8) {
            let op: BinOp = unsafe { mem::transmute(self.expr_data[id.index() + 3]) };
            let arr_len = read_value_bool!(tag, 9).then(|| self.expr_data[id.index() + 4]);
            AssignOp::Binary(op, arr_len)
        } else {
            AssignOp::Assign
        };
        (lhs, rhs, op)
    }
    pub fn stmt_expr_semi(&self, id: StmtID) -> ExprID {
        ExprID(self.expr_data[id.index() + 1])
    }
    pub fn stmt_expr_tail(&self, id: StmtID) -> ExprID {
        ExprID(self.expr_data[id.index() + 1])
    }

    pub fn block(&self, id: ExprID) -> &[StmtID] {
        self.read_block(id.index() + 1)
    }
    pub fn slice_field(&self, id: ExprID) -> (ExprID, SliceFieldAccess) {
        let tag = self.expr_data[id.index()];
        let deref = read_value_bits!(tag, 8, Option<ast::Mut>, u8);
        let field = read_value_bits!(tag, 16, SliceField, u8);
        let target = ExprID(self.expr_data[id.index() + 1]);
        (target, SliceFieldAccess { deref, field })
    }
    pub fn slice(&self, id: ExprID) -> (ExprID, SliceAccess) {
        let tag = self.expr_data[id.index()];
        let deref = read_value_bits!(tag, 8, Option<ast::Mut>, u8);
        let kind = read_value_bits!(tag, 16, SliceKind, u8);
        let target = ExprID(self.expr_data[id.index() + 1]);
        let op_call = ExprID(self.expr_data[id.index() + 2]);
        (target, SliceAccess { deref, kind, op_call })
    }
    pub fn cast(&self, id: ExprID) -> (ExprID, TypeID, hir::CastKind) {
        let tag = self.expr_data[id.index()];
        let kind = read_value_bits!(tag, 8, hir::CastKind, u8);
        let target = ExprID(self.expr_data[id.index() + 1]);
        let into = TypeID(self.expr_data[id.index() + 2]);
        (target, into, kind)
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
    pub fn call_direct(&self, id: ExprID, ir: &IR) -> (ProcID, &[TypeID], &[ExprID]) {
        let proc_id = ProcID(self.expr_data[id.index() + 1]);
        let data = &ir.procs[proc_id.index()];

        let start = id.index() + 2;
        let count = data.poly.map(|p| p.len()).unwrap_or(0);
        let poly: &[TypeID] = self.read_slice(start, count);

        let start = start + count;
        let count = data.params.len();
        let input: &[ExprID] = self.read_slice(start, count);
        (proc_id, poly, input)
    }
    pub fn struct_init(&self, id: ExprID, ir: &IR) -> (StructID, &[TypeID], &[ExprID]) {
        let struct_id = StructID(self.expr_data[id.index() + 1]);
        let data = &ir.structs[struct_id.index()];

        let start = id.index() + 2;
        let count = data.poly.map(|p| p.len()).unwrap_or(0);
        let poly: &[TypeID] = self.read_slice(start, count);

        let start = start + count;
        let count = data.fields.len();
        let input: &[ExprID] = self.read_slice(start, count);
        (struct_id, poly, input)
    }
    pub fn array_init(&self, id: ExprID) -> (TypeID, &[ExprID]) {
        let elem_ty = TypeID(self.expr_data[id.index() + 1]);
        let start = id.index() + 3;
        let count = self.expr_data[id.index() + 2] as usize;
        (elem_ty, self.read_slice(start, count))
    }
    pub fn array_repeat(&self, id: ExprID) -> (TypeID, u32, ExprID) {
        let elem_ty = TypeID(self.expr_data[id.index() + 1]);
        let len = self.expr_data[id.index() + 2];
        let value = ExprID(self.expr_data[id.index() + 3]);
        (elem_ty, len, value)
    }
    pub fn deref(&self, id: ExprID) -> (ExprID, TypeID, ast::Mut) {
        let tag = self.expr_data[id.index()];
        let mutt = read_value_bits!(tag, 8, ast::Mut, u8);
        let target = ExprID(self.expr_data[id.index() + 1]);
        let ref_ty = TypeID(self.expr_data[id.index() + 2]);
        (target, ref_ty, mutt)
    }
    pub fn address(&self, id: ExprID) -> ExprID {
        ExprID(self.expr_data[id.index() + 1])
    }
    pub fn unary(&self, id: ExprID) -> (UnOp, ExprID, Option<u32>) {
        let tag = self.expr_data[id.index()];
        let array_op = read_value_bool!(tag, 8);
        let op = read_value_bits!(tag, 16, UnOp, u16);
        let rhs = ExprID(self.expr_data[id.index() + 1]);
        let array_len = array_op.then(|| self.expr_data[id.index() + 2]);
        (op, rhs, array_len)
    }
    pub fn binary(&self, id: ExprID) -> (BinOp, ExprID, ExprID, Option<u32>) {
        let tag = self.expr_data[id.index()];
        let array_op = read_value_bool!(tag, 8);
        let op: BinOp = unsafe { mem::transmute(self.expr_data[id.index() + 1]) };
        let lhs = ExprID(self.expr_data[id.index() + 2]);
        let rhs = ExprID(self.expr_data[id.index() + 3]);
        let array_len = array_op.then(|| self.expr_data[id.index() + 4]);
        (op, lhs, rhs, array_len)
    }
}

#[allow(unsafe_code)]
impl Writer {
    pub fn alloc_body<'ir>(&mut self, arena: &mut Arena<'ir>) -> Body<'ir> {
        let expr_data = arena.alloc_slice(&self.expr_data);
        self.expr_data.clear();
        Body { expr_data }
    }

    pub fn stmt_break(&mut self) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        self.expr_data.push(StmtKind::Break as u32);
        id
    }
    pub fn stmt_continue(&mut self) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        self.expr_data.push(StmtKind::Continue as u32);
        id
    }
    pub fn stmt_return(&mut self, value: Option<ExprID>) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        self.expr_data.push(StmtKind::Return as u32 | (value.is_some() as u32) << 8);
        if let Some(value) = value {
            self.expr_data.push(value.0);
        }
        id
    }
    pub fn stmt_loop(&mut self, block: &[StmtID]) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        self.expr_data.push(StmtKind::Loop as u32);
        self.expr_data.push(block.len() as u32);
        for stmt in block.iter().copied() {
            self.expr_data.push(stmt.0);
        }
        id
    }
    pub fn stmt_local(&mut self, var_id: VariableID, init: LocalInit) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32 | (var_id.0 << 8));
        self.expr_data.push(StmtKind::Local as u32);
        match init {
            LocalInit::Init(value) => self.expr_data.push(value.0),
            LocalInit::Zeroed => self.expr_data.push(LOCAL_INIT_ZEROED),
            LocalInit::Undefined => self.expr_data.push(LOCAL_INIT_UNDEFINED),
        }
        id
    }
    pub fn stmt_assign(&mut self, lhs: ExprID, rhs: ExprID, op: AssignOp) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        let (binary, array) = match op {
            AssignOp::Assign => (false, false),
            AssignOp::Binary(_, None) => (true, false),
            AssignOp::Binary(_, Some(_)) => (true, true),
        };
        self.expr_data.push(StmtKind::Assign as u32 | (binary as u32) << 8 | (array as u32) << 9);
        self.expr_data.push(lhs.0);
        self.expr_data.push(rhs.0);
        if let AssignOp::Binary(op, arr_len) = op {
            self.expr_data.push(unsafe { mem::transmute(op) });
            if let Some(len) = arr_len {
                self.expr_data.push(len);
            }
        }
        id
    }
    pub fn stmt_expr_semi(&mut self, expr: ExprID) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        self.expr_data.push(StmtKind::ExprSemi as u32);
        self.expr_data.push(expr.0);
        id
    }
    pub fn stmt_expr_tail(&mut self, expr: ExprID) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        self.expr_data.push(StmtKind::ExprTail as u32);
        self.expr_data.push(expr.0);
        id
    }

    pub fn error(&mut self) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Error as u32);
        id
    }
    pub fn block(&mut self, block: &[StmtID]) -> StmtID {
        let id = StmtID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Block as u32);
        self.expr_data.push(block.len() as u32);
        for stmt in block.iter().copied() {
            self.expr_data.push(stmt.0);
        }
        id
    }
    pub fn slice_field(&mut self, target: ExprID, access: SliceFieldAccess) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        let deref: u8 = unsafe { mem::transmute(access.deref) };
        let field: u8 = unsafe { mem::transmute(access.field) };
        let tag = ExprKind::SliceField as u32 | (deref as u32) << 8 | (field as u32) << 16;
        self.expr_data.push(tag);
        self.expr_data.push(target.0);
        id
    }
    pub fn slice(&mut self, target: ExprID, access: SliceAccess) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        let deref: u8 = unsafe { mem::transmute(access.deref) };
        let kind: u8 = unsafe { mem::transmute(access.kind) };
        let tag = ExprKind::Slice as u32 | (deref as u32) << 8 | (kind as u32) << 16;
        self.expr_data.push(tag);
        self.expr_data.push(target.0);
        self.expr_data.push(access.op_call.0);
        id
    }
    pub fn cast(&mut self, target: ExprID, into: TypeID, kind: hir::CastKind) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Cast as u32 | ((kind as u32) << 8));
        self.expr_data.push(target.0);
        self.expr_data.push(into.0);
        id
    }
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
    pub fn call_direct(&mut self, proc_id: ProcID, poly: &[TypeID], input: &[ExprID]) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::CallDirect as u32);
        self.expr_data.push(proc_id.0);
        for type_id in poly.iter().copied() {
            self.expr_data.push(type_id.0);
        }
        for expr_id in input.iter().copied() {
            self.expr_data.push(expr_id.0);
        }
        id
    }
    pub fn struct_init(
        &mut self,
        struct_id: StructID,
        poly: &[TypeID],
        input: &[ExprID],
    ) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::StructInit as u32);
        self.expr_data.push(struct_id.0);
        for type_id in poly.iter().copied() {
            self.expr_data.push(type_id.0);
        }
        for expr_id in input.iter().copied() {
            self.expr_data.push(expr_id.0);
        }
        id
    }
    pub fn array_init(&mut self, elem_ty: TypeID, input: &[ExprID]) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::ArrayInit as u32);
        self.expr_data.push(elem_ty.0);
        self.expr_data.push(input.len() as u32);
        for expr_id in input.iter().copied() {
            self.expr_data.push(expr_id.0);
        }
        id
    }
    pub fn array_repeat(&mut self, elem_ty: TypeID, len: u32, value: ExprID) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::ArrayRepeat as u32);
        self.expr_data.push(elem_ty.0);
        self.expr_data.push(len);
        self.expr_data.push(value.0);
        id
    }
    pub fn deref(&mut self, target: ExprID, ref_ty: TypeID, mutt: ast::Mut) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Deref as u32 | (mutt as u32) << 8);
        self.expr_data.push(target.0);
        self.expr_data.push(ref_ty.0);
        id
    }
    pub fn address(&mut self, target: ExprID) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        self.expr_data.push(ExprKind::Address as u32);
        self.expr_data.push(target.0);
        id
    }
    pub fn unary(&mut self, op: UnOp, rhs: ExprID, arr_len: Option<u32>) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        let array = arr_len.is_some();
        let op: u16 = unsafe { mem::transmute(op) };
        self.expr_data.push(ExprKind::Unary as u32 | (array as u32) << 8 | (op as u32) << 16);
        self.expr_data.push(rhs.0);
        if let Some(len) = arr_len {
            self.expr_data.push(len);
        }
        id
    }
    pub fn binary(&mut self, op: BinOp, lhs: ExprID, rhs: ExprID, arr_len: Option<u32>) -> ExprID {
        let id = ExprID(self.expr_data.len() as u32);
        let array = arr_len.is_some();
        self.expr_data.push(ExprKind::Binary as u32 | (array as u32) << 8);
        self.expr_data.push(unsafe { mem::transmute(op) });
        self.expr_data.push(lhs.0);
        self.expr_data.push(rhs.0);
        if let Some(len) = arr_len {
            self.expr_data.push(len);
        }
        id
    }
}
