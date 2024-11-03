use crate::ast;
use crate::config::TargetPtrWidth;
use crate::error::SourceRange;
use crate::intern::InternName;
use crate::session::ModuleID;
use crate::support::{Arena, AsStr, BitSet, IndexID, ID};
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
    pub const_values: Vec<ConstValueID<'hir>>,
    pub variant_tag_values: Vec<ConstValue<'hir>>,
}

pub struct ConstInternPool<'hir> {
    arena: Arena<'hir>,
    values: Vec<ConstValue<'hir>>,
    intern_map: HashMap<ConstValue<'hir>, ConstValueID<'hir>>,
}

pub type ProcID<'hir> = ID<ProcData<'hir>>;
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

pub type ParamID<'hir> = ID<Param<'hir>>;
#[derive(Copy, Clone)]
pub struct Param<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

pub type EnumID<'hir> = ID<EnumData<'hir>>;
pub struct EnumData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet<EnumFlag>,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub variants: &'hir [Variant<'hir>],
    pub tag_ty: Result<BasicInt, ()>,
    pub layout: Eval<(), Layout>,
}

pub type VariantID<'hir> = ID<Variant<'hir>>;
pub type VariantFieldID<'hir> = ID<VariantField<'hir>>;
#[derive(Copy, Clone)]
pub struct Variant<'hir> {
    pub name: ast::Name,
    pub kind: VariantKind<'hir>,
    pub fields: &'hir [VariantField<'hir>],
}

#[derive(Copy, Clone)]
pub enum VariantKind<'hir> {
    Default(VariantEvalID<'hir>),
    Constant(ConstEvalID),
}

#[derive(Copy, Clone)]
pub struct VariantField<'hir> {
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

pub type VariantEvalID<'hir> = ID<VariantEval<'hir>>;
pub type VariantEval<'hir> = Eval<(), ConstValue<'hir>>;

pub type StructID<'hir> = ID<StructData<'hir>>;
pub struct StructData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet<StructFlag>,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub fields: &'hir [Field<'hir>],
    pub layout: Eval<(), Layout>,
}

pub type FieldID<'hir> = ID<Field<'hir>>;
#[derive(Copy, Clone)]
pub struct Field<'hir> {
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

pub type ConstID<'hir> = ID<ConstData<'hir>>;
pub struct ConstData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: ConstEvalID,
}

pub type GlobalID<'hir> = ID<GlobalData<'hir>>;
pub struct GlobalData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet<GlobalFlag>,
    pub vis: ast::Vis,
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: ConstEvalID,
}

pub type ImportID = ID<ImportData>;
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
    Enum(EnumID<'hir>),
    Struct(StructID<'hir>),
    Reference(ast::Mut, &'hir Type<'hir>),
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
    Local(LocalID<'hir>),
    Discard(&'hir Expr<'hir>),
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
    Local(LocalID<'hir>),
    Discard(&'hir Expr<'hir>),
}

pub type LocalID<'hir> = ID<Local<'hir>>;
#[derive(Copy, Clone)]
pub struct Local<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub init: &'hir Expr<'hir>,
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
    Match        { kind: MatchKind<'hir>, match_: &'hir Match<'hir> },
    StructField  { target: &'hir Expr<'hir>, access: StructFieldAccess<'hir> },
    SliceField   { target: &'hir Expr<'hir>, access: SliceFieldAccess },
    Index        { target: &'hir Expr<'hir>, access: &'hir IndexAccess<'hir> },
    Slice        { target: &'hir Expr<'hir>, access: &'hir SliceAccess<'hir> },
    Cast         { target: &'hir Expr<'hir>, into: &'hir Type<'hir>, kind: CastKind },
    ParamVar     { param_id: ParamID<'hir> },
    LocalVar     { local_id: LocalID<'hir> },
    LocalBind    { local_bind_id: LocalBindID<'hir> },
    ConstVar     { const_id: ConstID<'hir> },
    GlobalVar    { global_id: GlobalID<'hir> },
    Variant      { enum_id: EnumID<'hir>, variant_id: VariantID<'hir>, input: &'hir &'hir [&'hir Expr<'hir>] },
    CallDirect   { proc_id: ProcID<'hir>, input: &'hir [&'hir Expr<'hir>] },
    CallIndirect { target: &'hir Expr<'hir>, indirect: &'hir CallIndirect<'hir> },
    StructInit   { struct_id: StructID<'hir>, input: &'hir [FieldInit<'hir>] },
    ArrayInit    { array_init: &'hir ArrayInit<'hir> },
    ArrayRepeat  { array_repeat: &'hir ArrayRepeat<'hir> },
    Deref        { rhs: &'hir Expr<'hir>,  mutt: ast::Mut, ref_ty: &'hir Type<'hir> },
    Address      { rhs: &'hir Expr<'hir> },
    Unary        { op: UnOp, rhs: &'hir Expr<'hir> },
    Binary       { op: BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir> },
}

pub type ConstEvalID = ID<()>; // avoiding 'ast lifetime propagation
pub type ConstEval<'hir, 'ast> = Eval<ast::ConstExpr<'ast>, ConstValueID<'hir>>;
pub type ConstValueID<'hir> = ID<ConstValue<'hir>>;

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
    Procedure   { proc_id: ProcID<'hir> },
    Variant     { variant: &'hir ConstVariant<'hir> },
    Struct      { struct_: &'hir ConstStruct<'hir> },
    Array       { array: &'hir ConstArray<'hir> },
    ArrayRepeat { value: ConstValueID<'hir>, len: u64 },
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstVariant<'hir> {
    pub enum_id: EnumID<'hir>,
    pub variant_id: VariantID<'hir>,
    pub value_ids: &'hir [ConstValueID<'hir>],
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstStruct<'hir> {
    pub struct_id: StructID<'hir>,
    pub value_ids: &'hir [ConstValueID<'hir>],
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstArray<'hir> {
    pub len: u64,
    pub value_ids: &'hir [ConstValueID<'hir>],
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
pub enum MatchKind<'hir> {
    Int { int_ty: BasicInt },
    Bool,
    Char,
    String,
    Enum {
        enum_id: EnumID<'hir>,
        ref_mut: Option<ast::Mut> 
    },
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
    Const(ConstID<'hir>),
    Variant(EnumID<'hir>, VariantID<'hir>, &'hir [LocalBindID<'hir>]),
    Or(&'hir [Pat<'hir>]),
}

#[derive(Copy, Clone)]
pub struct StructFieldAccess<'hir> {
    pub deref: Option<ast::Mut>,
    pub struct_id: StructID<'hir>,
    pub field_id: FieldID<'hir>,
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
}

pub type LocalBindID<'hir> = ID<LocalBind<'hir>>;
#[derive(Copy, Clone)]
pub struct LocalBind<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub field_id: Option<VariantFieldID<'hir>>,
}

#[derive(Copy, Clone)]
pub struct CallIndirect<'hir> {
    pub proc_ty: &'hir ProcType<'hir>,
    pub input: &'hir [&'hir Expr<'hir>],
}

#[derive(Copy, Clone)]
pub struct FieldInit<'hir> {
    pub field_id: FieldID<'hir>,
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

#[derive(Copy, Clone, PartialEq, Hash)]
pub enum BasicInt {
    S8,
    S16,
    S32,
    S64,
    Ssize,
    U8,
    U16,
    U32,
    U64,
    Usize,
}

#[derive(Copy, Clone, PartialEq, Hash)]
pub enum BasicFloat {
    F32,
    F64,
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

//==================== SIZE LOCK ====================

use crate::size_lock;
size_lock!(16, Type);
size_lock!(16, Stmt);
size_lock!(32, Expr);
size_lock!(16, ConstValue);

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
        HasRepr "has repr",
        HasFields "has fields"
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum StructFlag {
        ReprC "repr(C)",
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
            EnumFlag::HasRepr => true,
            EnumFlag::HasFields => true,
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
    pub fn proc_data(&self, id: ProcID<'hir>) -> &ProcData<'hir> {
        self.procs.id_get(id)
    }
    pub fn enum_data(&self, id: EnumID<'hir>) -> &EnumData<'hir> {
        self.enums.id_get(id)
    }
    pub fn struct_data(&self, id: StructID<'hir>) -> &StructData<'hir> {
        self.structs.id_get(id)
    }
    pub fn const_data(&self, id: ConstID<'hir>) -> &ConstData<'hir> {
        self.consts.id_get(id)
    }
    pub fn global_data(&self, id: GlobalID<'hir>) -> &GlobalData<'hir> {
        self.globals.id_get(id)
    }
    pub fn const_value(&self, id: ConstValueID<'hir>) -> ConstValue<'hir> {
        self.const_intern.get(id)
    }
    pub fn const_eval_value(&self, id: ConstEvalID) -> ConstValue<'hir> {
        let value_id = self.const_values[id.raw_index()];
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

    pub fn intern(&mut self, value: ConstValue<'hir>) -> ConstValueID<'hir> {
        if let Some(id) = self.intern_map.get(&value).cloned() {
            return id;
        }
        let id = ConstValueID::new(&self.values);
        self.values.push(value);
        self.intern_map.insert(value, id);
        id
    }

    pub fn get(&self, id: ConstValueID<'hir>) -> ConstValue<'hir> {
        *self.values.id_get(id)
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
    pub fn param(&self, id: ParamID<'hir>) -> &'hir Param<'hir> {
        self.params.id_get(id)
    }
    pub fn local(&self, id: LocalID<'hir>) -> &'hir Local<'hir> {
        self.locals.id_get(id)
    }
    pub fn local_bind(&self, id: LocalBindID<'hir>) -> &'hir LocalBind<'hir> {
        self.local_binds.id_get(id)
    }
    pub fn find_param(&self, id: ID<InternName>) -> Option<(ParamID<'hir>, &'hir Param<'hir>)> {
        for (idx, param) in self.params.iter().enumerate() {
            if param.name.id == id {
                return Some((ParamID::new_raw(idx), param));
            }
        }
        None
    }
}

impl<'hir> EnumData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
    pub fn variant(&self, id: VariantID<'hir>) -> &'hir Variant<'hir> {
        self.variants.id_get(id)
    }
}

impl<'hir> Variant<'hir> {
    pub fn field(&self, id: VariantFieldID<'hir>) -> &'hir VariantField<'hir> {
        self.fields.id_get(id)
    }
    pub fn field_id(&self, idx: usize) -> Option<VariantFieldID<'hir>> {
        if idx < self.fields.len() {
            Some(VariantFieldID::new_raw(idx))
        } else {
            None
        }
    }
}

impl<'hir> StructData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
    pub fn field(&self, id: FieldID<'hir>) -> &'hir Field<'hir> {
        self.fields.id_get(id)
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

    pub fn get_resolved(&self) -> Result<R, ()> {
        match self {
            Eval::Unresolved(_) => unreachable!(),
            Eval::Resolved(val) => Ok(*val),
            Eval::ResolvedError => Err(()),
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
