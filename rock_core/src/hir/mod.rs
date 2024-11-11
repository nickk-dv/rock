use crate::ast;
use crate::config::TargetPtrWidth;
use crate::error::SourceRange;
use crate::intern::NameID;
use crate::session::ModuleID;
use crate::support::{Arena, AsStr, BitSet};
use crate::text::TextRange;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub struct Hir<'hir> {
    pub arena: Arena<'hir>,
    pub const_intern: ConstInternPool<'hir>,
    pub procs: Vec<ProcData<'hir>>,
    pub enums: Vec<EnumData<'hir>>,
    pub structs: Vec<StructData<'hir>>,
    pub consts: Vec<ConstData<'hir>>,
    pub globals: Vec<GlobalData<'hir>>,
    pub const_values: Vec<ConstValueID>,
    pub variant_tag_values: Vec<ConstValue<'hir>>,
}

pub struct ConstInternPool<'hir> {
    arena: Arena<'hir>,
    values: Vec<ConstValue<'hir>>,
    intern_map: HashMap<ConstValue<'hir>, ConstValueID>,
}

pub struct ProcData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet<ProcFlag>,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub params: &'hir [Param<'hir>],
    pub return_ty: Type<'hir>,
    pub block: Option<Block<'hir>>,
    pub locals: &'hir [Local<'hir>],
    pub local_binds: &'hir [LocalBind<'hir>],
}

#[derive(Copy, Clone)]
pub struct Param<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

pub struct EnumData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet<EnumFlag>,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub variants: &'hir [Variant<'hir>],
    pub tag_ty: Eval<(), BasicInt>,
    pub layout: Eval<(), Layout>,
}

#[derive(Copy, Clone)]
pub struct Variant<'hir> {
    pub name: ast::Name,
    pub kind: VariantKind,
    pub fields: &'hir [VariantField<'hir>],
}

#[derive(Copy, Clone)]
pub enum VariantKind {
    Default(VariantEvalID),
    Constant(ConstEvalID),
}

#[derive(Copy, Clone)]
pub struct VariantField<'hir> {
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

pub struct StructData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet<StructFlag>,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub fields: &'hir [Field<'hir>],
    pub layout: Eval<(), Layout>,
}

#[derive(Copy, Clone)]
pub struct Field<'hir> {
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

pub struct ConstData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: ConstEvalID,
}

pub struct GlobalData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet<GlobalFlag>,
    pub vis: ast::Vis,
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: ConstEvalID,
}

pub struct ImportData {
    pub origin_id: ModuleID,
}

#[derive(Copy, Clone)]
pub struct Layout {
    size: u64,
    align: u64,
}

#[derive(Copy, Clone)]
pub enum Type<'hir> {
    Error,
    Basic(ast::BasicType),
    Enum(EnumID),
    Struct(StructID),
    Reference(ast::Mut, &'hir Type<'hir>),
    MultiReference(ast::Mut, &'hir Type<'hir>),
    Procedure(&'hir ProcType<'hir>),
    ArraySlice(&'hir ArraySlice<'hir>),
    ArrayStatic(&'hir ArrayStatic<'hir>),
}

#[derive(Copy, Clone)]
pub struct ProcType<'hir> {
    pub param_types: &'hir [Type<'hir>],
    pub return_ty: Type<'hir>,
    pub is_variadic: bool,
}

#[derive(Copy, Clone)]
pub struct ArraySlice<'hir> {
    pub mutt: ast::Mut,
    pub elem_ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct ArrayStatic<'hir> {
    pub len: ArrayStaticLen,
    pub elem_ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub enum ArrayStaticLen {
    Immediate(u64),
    ConstEval(ConstEvalID),
}

#[derive(Copy, Clone)]
pub struct Block<'hir> {
    pub stmts: &'hir [Stmt<'hir>],
}

#[derive(Copy, Clone)]
pub enum Stmt<'hir> {
    Break,
    Continue,
    Return(Option<&'hir Expr<'hir>>),
    Defer(&'hir Block<'hir>),
    Loop(&'hir Loop<'hir>),
    For(&'hir For<'hir>),
    Local(LocalID),
    Discard(Option<&'hir Expr<'hir>>),
    Assign(&'hir Assign<'hir>),
    ExprSemi(&'hir Expr<'hir>),
    ExprTail(&'hir Expr<'hir>),
}

#[derive(Copy, Clone)]
pub struct Loop<'hir> {
    pub kind: LoopKind<'hir>,
    pub block: Block<'hir>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum LoopKind<'hir> {
    Loop,
    While { cond: &'hir Expr<'hir> },
    ForLoop {
        bind: ForLoopBind<'hir>,
        cond: &'hir Expr<'hir>,
        assign: &'hir Assign<'hir>,
    },
}

#[derive(Copy, Clone)]
pub enum ForLoopBind<'hir> {
    Error,
    Local(LocalID),
    Discard(Option<&'hir Expr<'hir>>),
}

#[derive(Copy, Clone)]
pub struct For<'hir> {
    pub kind: ForKind<'hir>,
    pub block: Block<'hir>,
}

#[derive(Copy, Clone)]
pub enum ForKind<'hir> {
    Loop,
    Cond(&'hir Expr<'hir>),
    Elem(&'hir ForElem<'hir>),
    Pat(&'hir ForPat),
}

#[derive(Copy, Clone)]
pub struct ForElem<'hir> {
    //some local_id for name
    //some opt local_id for index
    pub by_pointer: bool,
    pub deref: bool,
    pub elem_ty: Type<'hir>,
    pub kind: ForElemKind,
    pub expr: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub enum ForElemKind {
    Slice,
    Array(ArrayStaticLen),
}

pub struct ForPat {}

#[derive(Copy, Clone)]
pub struct Local<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub init: LocalInit<'hir>,
}

#[derive(Copy, Clone)]
pub enum LocalInit<'hir> {
    Init(&'hir Expr<'hir>),
    Zeroed,
    Undefined,
}

#[derive(Copy, Clone)]
pub struct Assign<'hir> {
    pub op: AssignOp,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
    pub lhs_ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct Expr<'hir> {
    pub kind: ExprKind<'hir>,
    pub range: TextRange,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ExprKind<'hir> {
    Error,
    Const        { value: ConstValue<'hir> },
    If           { if_: &'hir If<'hir> },
    Block        { block: Block<'hir> },
    Match        { kind: MatchKind, match_: &'hir Match<'hir> },
    StructField  { target: &'hir Expr<'hir>, access: StructFieldAccess },
    SliceField   { target: &'hir Expr<'hir>, access: SliceFieldAccess },
    Index        { target: &'hir Expr<'hir>, access: &'hir IndexAccess<'hir> },
    Slice        { target: &'hir Expr<'hir>, access: &'hir SliceAccess<'hir> },
    Cast         { target: &'hir Expr<'hir>, into: &'hir Type<'hir>, kind: CastKind },
    ParamVar     { param_id: ParamID },
    LocalVar     { local_id: LocalID },
    LocalBind    { local_bind_id: LocalBindID },
    ConstVar     { const_id: ConstID },
    GlobalVar    { global_id: GlobalID },
    Variant      { enum_id: EnumID, variant_id: VariantID, input: &'hir &'hir [&'hir Expr<'hir>] },
    CallDirect   { proc_id: ProcID, input: &'hir [&'hir Expr<'hir>] },
    CallIndirect { target: &'hir Expr<'hir>, indirect: &'hir CallIndirect<'hir> },
    StructInit   { struct_id: StructID, input: &'hir [FieldInit<'hir>] },
    ArrayInit    { array_init: &'hir ArrayInit<'hir> },
    ArrayRepeat  { array_repeat: &'hir ArrayRepeat<'hir> },
    Deref        { rhs: &'hir Expr<'hir>,  mutt: ast::Mut, ref_ty: &'hir Type<'hir> },
    Address      { rhs: &'hir Expr<'hir> },
    Unary        { op: UnOp, rhs: &'hir Expr<'hir> },
    Binary       { op: BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir> },
}

#[rustfmt::skip]
#[derive(Copy, Clone, PartialEq)]
pub enum ConstValue<'hir> {
    Void,
    Null,
    Bool        { val: bool },
    Int         { val: u64, neg: bool, int_ty: BasicInt },
    Float       { val: f64, float_ty: BasicFloat },
    Char        { val: char },
    String      { string_lit: ast::StringLit },
    Procedure   { proc_id: ProcID },
    Variant     { variant: &'hir ConstVariant<'hir> },
    Struct      { struct_: &'hir ConstStruct<'hir> },
    Array       { array: &'hir ConstArray<'hir> },
    ArrayRepeat { value: ConstValueID, len: u64 },
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstVariant<'hir> {
    pub enum_id: EnumID,
    pub variant_id: VariantID,
    pub value_ids: &'hir [ConstValueID],
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstStruct<'hir> {
    pub struct_id: StructID,
    pub value_ids: &'hir [ConstValueID],
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstArray<'hir> {
    pub len: u64,
    pub value_ids: &'hir [ConstValueID],
}

#[derive(Copy, Clone)]
pub struct If<'hir> {
    pub entry: Branch<'hir>,
    pub branches: &'hir [Branch<'hir>],
    pub else_block: Option<Block<'hir>>,
}

#[derive(Copy, Clone)]
pub struct Branch<'hir> {
    pub cond: &'hir Expr<'hir>,
    pub block: Block<'hir>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum MatchKind {
    Int { int_ty: BasicInt },
    Bool,
    Char,
    String,
    Enum { enum_id: EnumID, ref_mut: Option<ast::Mut> },
}

#[derive(Copy, Clone)]
pub struct Match<'hir> {
    pub on_expr: &'hir Expr<'hir>,
    pub arms: &'hir [MatchArm<'hir>],
}

#[derive(Copy, Clone)]
pub struct MatchArm<'hir> {
    pub pat: Pat<'hir>,
    pub block: Block<'hir>,
}

//@lower pat size
#[derive(Copy, Clone)]
pub enum Pat<'hir> {
    Error,
    Wild,
    Lit(ConstValue<'hir>),
    Const(ConstID),
    Variant(EnumID, VariantID, &'hir [LocalBindID]),
    Or(&'hir [Pat<'hir>]),
}

#[derive(Copy, Clone)]
pub struct StructFieldAccess {
    pub deref: Option<ast::Mut>,
    pub struct_id: StructID,
    pub field_id: FieldID,
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
pub struct IndexAccess<'hir> {
    pub deref: Option<ast::Mut>,
    pub elem_ty: Type<'hir>,
    pub kind: IndexKind,
    pub index: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub enum IndexKind {
    Multi(ast::Mut),
    Slice(ast::Mut),
    Array(ArrayStaticLen),
}

#[derive(Copy, Clone)]
pub struct SliceAccess<'hir> {
    pub deref: Option<ast::Mut>,
    pub kind: SliceKind<'hir>,
    pub range: SliceRange<'hir>,
}

#[derive(Copy, Clone)]
pub enum SliceKind<'hir> {
    Slice { elem_size: u64 },
    Array { array: &'hir ArrayStatic<'hir> },
}

#[derive(Copy, Clone)]
pub struct SliceRange<'hir> {
    pub lower: Option<&'hir Expr<'hir>>,
    pub upper: SliceRangeEnd<'hir>,
}

#[derive(Copy, Clone)]
pub enum SliceRangeEnd<'hir> {
    Unbounded,
    Exclusive(&'hir Expr<'hir>),
    Inclusive(&'hir Expr<'hir>),
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum CastKind {
    Error,
    NoOp,
    Int_Trunc,
    IntS_Sign_Extend,
    IntU_Zero_Extend,
    IntS_to_Float,
    IntU_to_Float,
    Float_to_IntS,
    Float_to_IntU,
    Float_Trunc,
    Float_Extend,
    Bool_to_Int,
    Char_to_U32,
}

#[derive(Copy, Clone)]
pub struct LocalBind<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub field_id: Option<VariantFieldID>,
}

#[derive(Copy, Clone)]
pub struct CallIndirect<'hir> {
    pub proc_ty: &'hir ProcType<'hir>,
    pub input: &'hir [&'hir Expr<'hir>],
}

#[derive(Copy, Clone)]
pub struct FieldInit<'hir> {
    pub field_id: FieldID,
    pub expr: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub struct ArrayInit<'hir> {
    pub elem_ty: Type<'hir>,
    pub input: &'hir [&'hir Expr<'hir>],
}

#[derive(Copy, Clone)]
pub struct ArrayRepeat<'hir> {
    pub elem_ty: Type<'hir>,
    pub value: &'hir Expr<'hir>,
    pub len: u64,
}

#[derive(Copy, Clone)]
pub enum Eval<U, R>
where
    U: Copy + Clone,
    R: Copy + Clone,
{
    Unresolved(U),
    Resolved(R),
    ResolvedError,
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq, Hash)]
    pub enum BasicInt {
        S8 "s8",
        S16 "s16",
        S32 "s32",
        S64 "s64",
        Ssize "ssize",
        U8 "u8",
        U16 "u16",
        U32 "u32",
        U64 "u64",
        Usize "usize",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq, Hash)]
    pub enum BasicFloat {
        F32 "f32",
        F64 "f64",
    }
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum UnOp {
    Neg_Int,
    Neg_Float,
    BitNot,
    LogicNot,
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone)]
pub enum BinOp {
    Add_Int,
    Add_Float,
    Sub_Int,
    Sub_Float,
    Mul_Int,
    Mul_Float,
    Div_IntS,
    Div_IntU,
    Div_Float,
    Rem_IntS,
    Rem_IntU,
    BitAnd,
    BitOr,
    BitXor,
    BitShl,
    BitShr_IntS,
    BitShr_IntU,
    IsEq_Int,
    IsEq_Float,
    NotEq_Int,
    NotEq_Float,
    Less_IntS,
    Less_IntU,
    Less_Float,
    LessEq_IntS,
    LessEq_IntU,
    LessEq_Float,
    Greater_IntS,
    Greater_IntU,
    Greater_Float,
    GreaterEq_IntS,
    GreaterEq_IntU,
    GreaterEq_Float,
    LogicAnd,
    LogicOr,
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    Bin(BinOp),
}

//==================== HIR IDS & EVALS ====================

crate::define_id!(pub ProcID);
crate::define_id!(pub EnumID);
crate::define_id!(pub StructID);
crate::define_id!(pub ConstID);
crate::define_id!(pub GlobalID);
crate::define_id!(pub ImportID);

crate::define_id!(pub ParamID);
crate::define_id!(pub LocalID);
crate::define_id!(pub LocalBindID);
crate::define_id!(pub VariantID);
crate::define_id!(pub VariantFieldID);
crate::define_id!(pub FieldID);

crate::define_id!(pub ConstEvalID);
crate::define_id!(pub ConstValueID);
crate::define_id!(pub VariantEvalID);

pub type ConstEval<'hir, 'ast> = Eval<ast::ConstExpr<'ast>, ConstValueID>;
pub type VariantEval<'hir> = Eval<(), ConstValue<'hir>>;

//==================== SIZE LOCK ====================

crate::size_lock!(16, Type);
crate::size_lock!(16, Stmt);
crate::size_lock!(32, Expr);
crate::size_lock!(16, ConstValue);

//==================== ITEM FLAGS ====================

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum ProcFlag {
        External "external",
        Variadic "variadic",
        Main "main",
        Builtin "builtin",
        Inline "inline",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum EnumFlag {
        ReprC "repr_c",
        WithFields "with fields",
        WithTagType "with tag type",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum StructFlag {
        ReprC "repr_c",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum GlobalFlag {
        ThreadLocal "threadlocal",
    }
}

impl Into<u32> for ProcFlag {
    fn into(self) -> u32 {
        self as u32
    }
}
impl Into<u32> for EnumFlag {
    fn into(self) -> u32 {
        self as u32
    }
}
impl Into<u32> for StructFlag {
    fn into(self) -> u32 {
        self as u32
    }
}
impl Into<u32> for GlobalFlag {
    fn into(self) -> u32 {
        self as u32
    }
}

pub trait ItemFlag
where
    Self: PartialEq,
{
    fn compatible(self, other: Self) -> bool;
}

impl ItemFlag for ProcFlag {
    fn compatible(self, other: ProcFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            ProcFlag::External => matches!(other, ProcFlag::Variadic | ProcFlag::Inline),
            ProcFlag::Variadic => matches!(other, ProcFlag::External | ProcFlag::Inline),
            ProcFlag::Main => false,
            ProcFlag::Builtin => matches!(other, ProcFlag::External | ProcFlag::Inline),
            ProcFlag::Inline => !matches!(other, ProcFlag::Main),
        }
    }
}

impl ItemFlag for EnumFlag {
    fn compatible(self, other: EnumFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            EnumFlag::ReprC => false,
            EnumFlag::WithFields => matches!(other, EnumFlag::WithTagType),
            EnumFlag::WithTagType => matches!(other, EnumFlag::WithFields),
        }
    }
}

impl ItemFlag for StructFlag {
    fn compatible(self, other: StructFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            StructFlag::ReprC => false,
        }
    }
}

impl ItemFlag for GlobalFlag {
    fn compatible(self, other: GlobalFlag) -> bool {
        if self == other {
            unreachable!()
        }
        match self {
            GlobalFlag::ThreadLocal => false,
        }
    }
}

//==================== HIR IMPL ====================

impl<'hir> Hir<'hir> {
    pub fn proc_data(&self, id: ProcID) -> &ProcData<'hir> {
        &self.procs[id.index()]
    }
    pub fn enum_data(&self, id: EnumID) -> &EnumData<'hir> {
        &self.enums[id.index()]
    }
    pub fn struct_data(&self, id: StructID) -> &StructData<'hir> {
        &self.structs[id.index()]
    }
    pub fn const_data(&self, id: ConstID) -> &ConstData<'hir> {
        &self.consts[id.index()]
    }
    pub fn global_data(&self, id: GlobalID) -> &GlobalData<'hir> {
        &self.globals[id.index()]
    }
    pub fn const_value(&self, id: ConstValueID) -> ConstValue<'hir> {
        self.const_intern.get(id)
    }
    pub fn const_eval_value(&self, id: ConstEvalID) -> ConstValue<'hir> {
        let value_id = self.const_values[id.index()];
        self.const_intern.get(value_id)
    }
}

impl<'hir> ConstInternPool<'hir> {
    pub fn new() -> ConstInternPool<'hir> {
        ConstInternPool {
            arena: Arena::new(),
            values: Vec::with_capacity(1024),
            intern_map: HashMap::with_capacity(1024),
        }
    }

    pub fn intern(&mut self, value: ConstValue<'hir>) -> ConstValueID {
        if let Some(id) = self.intern_map.get(&value).cloned() {
            return id;
        }
        let id = ConstValueID::new(self.values.len());
        self.values.push(value);
        self.intern_map.insert(value, id);
        id
    }

    pub fn get(&self, id: ConstValueID) -> ConstValue<'hir> {
        self.values[id.index()]
    }
    pub fn arena(&mut self) -> &mut Arena<'hir> {
        &mut self.arena
    }
}

impl<'hir> Eq for ConstValue<'hir> {}

//@perf: test for hash collision rates and how well this performs in terms of speed 09.05.24
impl<'hir> Hash for ConstValue<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match *self {
            ConstValue::Void => 0.hash(state),
            ConstValue::Null => 1.hash(state),
            ConstValue::Bool { val } => val.hash(state),
            ConstValue::Int { val, neg, int_ty } => (val, neg, int_ty).hash(state),
            ConstValue::Float { val, float_ty } => (val.to_bits(), float_ty).hash(state),
            ConstValue::Char { val } => val.hash(state),
            ConstValue::String { string_lit } => string_lit.hash(state),
            ConstValue::Procedure { proc_id } => proc_id.raw().hash(state),
            ConstValue::Variant { variant } => variant.hash(state),
            ConstValue::Struct { struct_ } => struct_.hash(state),
            ConstValue::Array { array } => array.hash(state),
            ConstValue::ArrayRepeat { value, len } => (value.raw(), len).hash(state),
        }
    }
}

impl<'hir> Hash for ConstVariant<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.enum_id.raw(), self.variant_id.raw(), self.value_ids).hash(state);
    }
}

impl<'hir> Hash for ConstStruct<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.struct_id.raw(), self.value_ids).hash(state);
    }
}

impl<'hir> Hash for ConstArray<'hir> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.len, self.value_ids).hash(state);
    }
}

impl<'hir> ProcData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
    pub fn param(&self, id: ParamID) -> &'hir Param<'hir> {
        &self.params[id.index()]
    }
    pub fn local(&self, id: LocalID) -> &'hir Local<'hir> {
        &self.locals[id.index()]
    }
    pub fn local_bind(&self, id: LocalBindID) -> &'hir LocalBind<'hir> {
        &self.local_binds[id.index()]
    }
    pub fn find_param(&self, id: NameID) -> Option<(ParamID, &'hir Param<'hir>)> {
        for (idx, param) in self.params.iter().enumerate() {
            if param.name.id == id {
                return Some((ParamID::new(idx), param));
            }
        }
        None
    }
}

impl<'hir> EnumData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
    pub fn variant(&self, id: VariantID) -> &'hir Variant<'hir> {
        &self.variants[id.index()]
    }
}

impl<'hir> Variant<'hir> {
    pub fn field(&self, id: VariantFieldID) -> &'hir VariantField<'hir> {
        &self.fields[id.index()]
    }
    pub fn field_id(&self, idx: usize) -> Option<VariantFieldID> {
        if idx < self.fields.len() {
            Some(VariantFieldID::new(idx))
        } else {
            None
        }
    }
}

impl<'hir> StructData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
    pub fn field(&self, id: FieldID) -> &'hir Field<'hir> {
        &self.fields[id.index()]
    }
}

impl<'hir> ConstData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
}

impl<'hir> GlobalData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
}

impl Layout {
    pub fn new(size: u64, align: u64) -> Layout {
        Layout { size, align }
    }
    pub fn new_equal(size_align: u64) -> Layout {
        Layout {
            size: size_align,
            align: size_align,
        }
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn align(&self) -> u64 {
        self.align
    }
}

impl<'hir> Type<'hir> {
    pub const USIZE: Type<'static> = Type::Basic(ast::BasicType::Usize);
    pub const BOOL: Type<'static> = Type::Basic(ast::BasicType::Bool);
    pub const VOID: Type<'static> = Type::Basic(ast::BasicType::Void);
    pub const NEVER: Type<'static> = Type::Basic(ast::BasicType::Never);

    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }
    #[inline]
    pub fn is_void(&self) -> bool {
        matches!(self, Type::Basic(ast::BasicType::Void))
    }
    #[inline]
    pub fn is_never(&self) -> bool {
        matches!(self, Type::Basic(ast::BasicType::Never))
    }
}

impl<U, R> Eval<U, R>
where
    U: Copy + Clone,
    R: Copy + Clone,
{
    pub fn from_res(res: Result<R, ()>) -> Eval<U, R> {
        match res {
            Ok(val) => Eval::Resolved(val),
            Err(_) => Eval::ResolvedError,
        }
    }

    pub fn is_unresolved(&self) -> bool {
        matches!(self, Eval::Unresolved(_))
    }
    pub fn is_resolved_ok(&self) -> bool {
        matches!(self, Eval::Resolved(_))
    }

    pub fn resolved(&self) -> Result<R, ()> {
        match self {
            Eval::Unresolved(_) => unreachable!("eval not resolved"),
            Eval::Resolved(val) => Ok(*val),
            Eval::ResolvedError => Err(()),
        }
    }

    pub fn resolved_unwrap(&self) -> R {
        match self {
            Eval::Unresolved(_) => unreachable!("eval unresolved"),
            Eval::Resolved(val) => *val,
            Eval::ResolvedError => unreachable!("eval error"),
        }
    }
}

impl BasicInt {
    pub fn from_str(string: &str) -> Option<BasicInt> {
        match string {
            "s8" => Some(BasicInt::S8),
            "s16" => Some(BasicInt::S16),
            "s32" => Some(BasicInt::S32),
            "s64" => Some(BasicInt::S64),
            "ssize" => Some(BasicInt::Ssize),
            "u8" => Some(BasicInt::U8),
            "u16" => Some(BasicInt::U16),
            "u32" => Some(BasicInt::U32),
            "u64" => Some(BasicInt::U64),
            "usize" => Some(BasicInt::Usize),
            _ => None,
        }
    }

    pub fn from_basic(basic: ast::BasicType) -> Option<BasicInt> {
        match basic {
            ast::BasicType::S8 => Some(BasicInt::S8),
            ast::BasicType::S16 => Some(BasicInt::S16),
            ast::BasicType::S32 => Some(BasicInt::S32),
            ast::BasicType::S64 => Some(BasicInt::S64),
            ast::BasicType::Ssize => Some(BasicInt::Ssize),
            ast::BasicType::U8 => Some(BasicInt::U8),
            ast::BasicType::U16 => Some(BasicInt::U16),
            ast::BasicType::U32 => Some(BasicInt::U32),
            ast::BasicType::U64 => Some(BasicInt::U64),
            ast::BasicType::Usize => Some(BasicInt::Usize),
            _ => None,
        }
    }

    pub fn into_basic(self) -> ast::BasicType {
        match self {
            BasicInt::S8 => ast::BasicType::S8,
            BasicInt::S16 => ast::BasicType::S16,
            BasicInt::S32 => ast::BasicType::S32,
            BasicInt::S64 => ast::BasicType::S64,
            BasicInt::Ssize => ast::BasicType::Ssize,
            BasicInt::U8 => ast::BasicType::U8,
            BasicInt::U16 => ast::BasicType::U16,
            BasicInt::U32 => ast::BasicType::U32,
            BasicInt::U64 => ast::BasicType::U64,
            BasicInt::Usize => ast::BasicType::Usize,
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(
            self,
            BasicInt::S8 | BasicInt::S16 | BasicInt::S32 | BasicInt::S64 | BasicInt::Ssize
        )
    }

    pub fn min_128(self, ptr_width: TargetPtrWidth) -> i128 {
        match self {
            BasicInt::S8 => i8::MIN as i128,
            BasicInt::S16 => i16::MIN as i128,
            BasicInt::S32 => i32::MIN as i128,
            BasicInt::S64 => i64::MIN as i128,
            BasicInt::Ssize => match ptr_width {
                TargetPtrWidth::Bit_32 => i32::MIN as i128,
                TargetPtrWidth::Bit_64 => i64::MIN as i128,
            },
            BasicInt::U8 => 0,
            BasicInt::U16 => 0,
            BasicInt::U32 => 0,
            BasicInt::U64 => 0,
            BasicInt::Usize => 0,
        }
    }

    pub fn max_128(self, ptr_width: TargetPtrWidth) -> i128 {
        match self {
            BasicInt::S8 => i8::MAX as i128,
            BasicInt::S16 => i16::MAX as i128,
            BasicInt::S32 => i32::MAX as i128,
            BasicInt::S64 => i64::MAX as i128,
            BasicInt::Ssize => match ptr_width {
                TargetPtrWidth::Bit_32 => i32::MAX as i128,
                TargetPtrWidth::Bit_64 => i64::MAX as i128,
            },
            BasicInt::U8 => u8::MAX as i128,
            BasicInt::U16 => u16::MAX as i128,
            BasicInt::U32 => u32::MAX as i128,
            BasicInt::U64 => u64::MAX as i128,
            BasicInt::Usize => match ptr_width {
                TargetPtrWidth::Bit_32 => u32::MAX as i128,
                TargetPtrWidth::Bit_64 => u64::MAX as i128,
            },
        }
    }
}

impl BasicFloat {
    pub fn from_basic(basic: ast::BasicType) -> Option<BasicFloat> {
        match basic {
            ast::BasicType::F32 => Some(BasicFloat::F32),
            ast::BasicType::F64 => Some(BasicFloat::F64),
            _ => None,
        }
    }

    pub fn into_basic(self) -> ast::BasicType {
        match self {
            BasicFloat::F32 => ast::BasicType::F32,
            BasicFloat::F64 => ast::BasicType::F64,
        }
    }
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add_Int | BinOp::Add_Float => "+",
            BinOp::Sub_Int | BinOp::Sub_Float => "-",
            BinOp::Mul_Int | BinOp::Mul_Float => "*",
            BinOp::Div_IntS | BinOp::Div_IntU | BinOp::Div_Float => "/",
            BinOp::Rem_IntS | BinOp::Rem_IntU => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::BitShl => "<<",
            BinOp::BitShr_IntS | BinOp::BitShr_IntU => ">>",
            BinOp::IsEq_Int | BinOp::IsEq_Float => "==",
            BinOp::NotEq_Int | BinOp::NotEq_Float => "!=",
            BinOp::Less_IntS | BinOp::Less_IntU | BinOp::Less_Float => "<",
            BinOp::LessEq_IntS | BinOp::LessEq_IntU | BinOp::LessEq_Float => "<=",
            BinOp::Greater_IntS | BinOp::Greater_IntU | BinOp::Greater_Float => ">",
            BinOp::GreaterEq_IntS | BinOp::GreaterEq_IntU | BinOp::GreaterEq_Float => ">=",
            BinOp::LogicAnd => "&&",
            BinOp::LogicOr => "||",
        }
    }
}

//@REMOVE BEFORE PUSH
mod example {
    use crate::ast;
    use crate::hir::*;

    struct VariableID(u32);
    struct Variable<'hir> {
        mutt: ast::Mut,
        name: ast::Name,
        ty: Type<'hir>,
    }

    #[derive(Copy, Clone)]
    pub enum LocalInit<'hir> {
        Init(&'hir Expr<'hir>),
        Zeroed,
        Undefined,
    }

    struct Param {
        pub var_id: VariableID,
        pub ty_range: TextRange,
    }
    struct Local<'hir> {
        pub var_id: VariableID,
        pub init: LocalInit<'hir>,
    }
    struct PatBind {
        pub var_id: VariableID,
        pub field_id: Option<VariantFieldID>,
    }
    struct ForBind {
        pub var_id: VariableID,
    }
}
