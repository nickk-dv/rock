use crate::ast;
use crate::config::TargetPtrWidth;
use crate::error::SourceRange;
use crate::intern::{LitID, NameID};
use crate::session::ModuleID;
use crate::support::{Arena, AsStr, BitSet};
use crate::text::{TextOffset, TextRange};

pub struct Hir<'hir> {
    pub arena: Arena<'hir>,
    pub procs: Vec<ProcData<'hir>>,
    pub enums: Vec<EnumData<'hir>>,
    pub structs: Vec<StructData<'hir>>,
    pub globals: Vec<GlobalData<'hir>>,
    pub const_eval_values: Vec<ConstValue<'hir>>,
    pub variant_eval_values: Vec<ConstValue<'hir>>,
    pub core: CoreItems,
}

pub struct CoreItems {
    pub string_equals: ProcID,
    pub cstring_equals: ProcID,
}

pub struct ProcData<'hir> {
    pub origin_id: ModuleID,
    pub flag_set: BitSet<ProcFlag>,
    pub vis: Vis,
    pub name: ast::Name,
    pub poly_params: Option<&'hir [ast::Name]>,
    pub params: &'hir [Param<'hir>],
    pub return_ty: Type<'hir>,
    pub block: Option<Block<'hir>>,
    pub variables: &'hir [Variable<'hir>],
}

#[derive(Copy, Clone)]
pub struct Param<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

#[derive(Copy, Clone)]
pub struct Variable<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub was_used: bool,
}

pub struct EnumData<'hir> {
    pub origin_id: ModuleID,
    pub flag_set: BitSet<EnumFlag>,
    pub vis: Vis,
    pub name: ast::Name,
    pub poly_params: Option<&'hir [ast::Name]>,
    pub variants: &'hir [Variant<'hir>],
    pub tag_ty: Eval<(), IntType>,
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
    pub flag_set: BitSet<StructFlag>,
    pub vis: Vis,
    pub name: ast::Name,
    pub poly_params: Option<&'hir [ast::Name]>,
    pub fields: &'hir [Field<'hir>],
    pub layout: Eval<(), Layout>,
}

#[derive(Copy, Clone)]
pub struct Field<'hir> {
    pub vis: Vis,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub ty_range: TextRange,
}

pub struct ConstData<'hir> {
    pub origin_id: ModuleID,
    pub flag_set: BitSet<ConstFlag>,
    pub vis: Vis,
    pub name: ast::Name,
    pub ty: Option<Type<'hir>>,
    pub value: ConstEvalID,
}

pub struct GlobalData<'hir> {
    pub origin_id: ModuleID,
    pub flag_set: BitSet<GlobalFlag>,
    pub vis: Vis,
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub init: GlobalInit,
}

#[derive(Copy, Clone)]
pub enum GlobalInit {
    Init(ConstEvalID),
    Zeroed,
}

pub struct ImportData {
    pub origin_id: ModuleID,
}

#[derive(Copy, Clone)]
pub struct Layout {
    pub size: u64,
    pub align: u64,
}

#[must_use]
#[derive(Copy, Clone)]
pub enum Type<'hir> {
    Error,
    Char,
    Void,
    Never,
    Rawptr,
    Int(IntType),
    Float(FloatType),
    Bool(BoolType),
    String(StringType),
    InferDef(PolymorphDefID, u32),
    Enum(EnumID, &'hir [Type<'hir>]),
    Struct(StructID, &'hir [Type<'hir>]),
    Reference(ast::Mut, &'hir Type<'hir>),
    MultiReference(ast::Mut, &'hir Type<'hir>),
    Procedure(&'hir ProcType<'hir>),
    ArraySlice(&'hir ArraySlice<'hir>),
    ArrayStatic(&'hir ArrayStatic<'hir>),
}

#[derive(Copy, Clone, PartialEq)]
pub enum PolymorphDefID {
    Proc(ProcID),
    Enum(EnumID),
    Struct(StructID),
}

#[derive(Copy, Clone)]
pub struct ProcType<'hir> {
    pub param_types: &'hir [Type<'hir>],
    pub return_ty: Type<'hir>,
    pub variadic: bool,
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
    Loop(&'hir Block<'hir>),
    Local(&'hir Local<'hir>),
    Discard(Option<&'hir Expr<'hir>>),
    Assign(&'hir Assign<'hir>),
    ExprSemi(&'hir Expr<'hir>),
    ExprTail(&'hir Expr<'hir>),
}

#[derive(Copy, Clone)]
pub struct Local<'hir> {
    pub var_id: VariableID,
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

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Expr<'hir> {
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
    CallerLocation { struct_id: StructID }, //@pass as param, refer to as param, remove.
    ParamVar     { param_id: ParamID },
    Variable     { var_id: VariableID },
    GlobalVar    { global_id: GlobalID },
    Variant      { enum_id: EnumID, variant_id: VariantID, input: &'hir &'hir [&'hir Expr<'hir>] },
    CallDirect   { proc_id: ProcID, input: &'hir [&'hir Expr<'hir>], start: TextOffset }, //@pass location in IR, no need for `start`
    CallIndirect { target: &'hir Expr<'hir>, indirect: &'hir CallIndirect<'hir> },
    StructInit   { struct_id: StructID, input: &'hir [FieldInit<'hir>] },
    ArrayInit    { array: &'hir ArrayInit<'hir> },
    ArrayRepeat  { array: &'hir ArrayRepeat<'hir> },
    Deref        { rhs: &'hir Expr<'hir>,  mutt: ast::Mut, ref_ty: &'hir Type<'hir> },
    Address      { rhs: &'hir Expr<'hir> },
    Unary        { op: UnOp, rhs: &'hir Expr<'hir> },
    Binary       { op: BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir> },
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ConstValue<'hir> {
    Void,
    Null,
    Bool        { val: bool, bool_ty: BoolType },
    Int         { val: u64, neg: bool, int_ty: IntType },
    Float       { val: f64, float_ty: FloatType },
    Char        { val: char },
    String      { val: LitID, string_ty: StringType },
    Procedure   { proc_id: ProcID },
    Variant     { enum_id: EnumID, variant_id: VariantID },
    Struct      { struct_: &'hir ConstStruct<'hir> },
    Array       { array: &'hir ConstArray<'hir> },
    ArrayRepeat { array: &'hir ConstArrayRepeat<'hir> },
    ArrayEmpty  { elem_ty: &'hir Type<'hir> },
}

#[derive(Copy, Clone)]
pub struct ConstStruct<'hir> {
    pub struct_id: StructID,
    pub values: &'hir [ConstValue<'hir>],
}

#[derive(Copy, Clone)]
pub struct ConstArray<'hir> {
    pub values: &'hir [ConstValue<'hir>],
}

#[derive(Copy, Clone)]
pub struct ConstArrayRepeat<'hir> {
    pub len: u64,
    pub value: ConstValue<'hir>,
}

#[derive(Copy, Clone)]
pub struct If<'hir> {
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
    Int { int_ty: IntType },
    Bool { bool_ty: BoolType },
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

#[derive(Copy, Clone)]
pub enum Pat<'hir> {
    Error,
    Wild,
    Lit(ConstValue<'hir>),
    Variant(EnumID, VariantID, &'hir [VariableID]),
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
#[derive(Copy, Clone, PartialEq)]
pub enum CastKind {
    Error,
    Char_NoOp,
    Rawptr_NoOp,

    Int_NoOp,
    Int_Trunc,
    IntS_Extend,
    IntU_Extend,
    IntS_to_Float,
    IntU_to_Float,

    Float_Trunc,
    Float_Extend,
    Float_to_IntS,
    Float_to_IntU,

    Bool_Trunc,
    Bool_Extend,
    Bool_NoOp_to_Int,
    Bool_Trunc_to_Int,
    Bool_Extend_to_Int,

    Enum_NoOp_to_Int,
    Enum_Trunc_to_Int,
    EnumS_Extend_to_Int,
    EnumU_Extend_to_Int,
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

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum Vis {
        Public "public",
        Private "private",
    }
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
    #[derive(Copy, Clone, PartialEq)]
    pub enum IntType {
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
        Untyped "untyped int",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum FloatType {
        F32 "f32",
        F64 "f64",
        Untyped "untyped float",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum BoolType {
        Bool "bool",
        Bool16 "bool16",
        Bool32 "bool32",
        Bool64 "bool64",
        Untyped "untyped bool",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum StringType {
        String "string",
        CString "cstring",
        Untyped "untyped string",
    }
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum UnOp {
    Neg_Int, Neg_Float,
    BitNot,
    LogicNot,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum BinOp {
    Add_Int, Add_Float,
    Sub_Int, Sub_Float,
    Mul_Int, Mul_Float,
    Div_Int(IntType), Div_Float,
    Rem_Int(IntType),
    BitAnd, BitOr, BitXor,
    BitShl, BitShr(IntType),
    Eq_Int_Other(BoolType),
    NotEq_Int_Other(BoolType),
    Cmp_Int(CmpPred, BoolType, IntType),
    Cmp_Float(CmpPred, BoolType, FloatType),
    Cmp_String(CmpPred, BoolType, StringType),
    LogicAnd(BoolType),
    LogicOr(BoolType),
}

#[derive(Copy, Clone, PartialEq)]
pub enum CmpPred {
    Eq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
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
crate::define_id!(pub VariableID);
crate::define_id!(pub VariantID);
crate::define_id!(pub VariantFieldID);
crate::define_id!(pub FieldID);

crate::define_id!(pub ConstEvalID);
crate::define_id!(pub VariantEvalID);

pub type ConstEval<'hir, 'ast> = Eval<ast::ConstExpr<'ast>, ConstValue<'hir>>;
pub type VariantEval<'hir> = Eval<(), ConstValue<'hir>>;

//==================== SIZE LOCK ====================

crate::size_lock!(24, Type);
crate::size_lock!(16, Stmt);
crate::size_lock!(32, Expr);
crate::size_lock!(16, ConstValue);

//==================== ITEM FLAGS ====================

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum ProcFlag {
        // directives
        Inline "inline",
        Builtin "builtin",
        // compile-time flags
        WasUsed "was used",
        External "external",
        Variadic "variadic",
        EntryPoint "entry point",
        CallerLocation "caller location",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum EnumFlag {
        WasUsed "was used",
        WithFields "with fields",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum StructFlag {
        WasUsed "was used",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum ConstFlag {
        WasUsed "was used",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum GlobalFlag {
        WasUsed "was used",
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
impl Into<u32> for ConstFlag {
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
    fn not_compatible(self, other: Self) -> bool;
}

impl ItemFlag for ProcFlag {
    fn not_compatible(self, other: ProcFlag) -> bool {
        use ProcFlag::*;
        match self {
            Inline => false,
            Builtin => matches!(other, Variadic | EntryPoint),
            WasUsed => false,
            External => matches!(other, EntryPoint),
            Variadic => matches!(other, Builtin | EntryPoint),
            EntryPoint => matches!(other, Builtin | External | Variadic),
            CallerLocation => false,
        }
    }
}
impl ItemFlag for EnumFlag {
    fn not_compatible(self, _: EnumFlag) -> bool {
        false
    }
}
impl ItemFlag for StructFlag {
    fn not_compatible(self, _: StructFlag) -> bool {
        false
    }
}
impl ItemFlag for ConstFlag {
    fn not_compatible(self, _: ConstFlag) -> bool {
        false
    }
}
impl ItemFlag for GlobalFlag {
    fn not_compatible(self, _: GlobalFlag) -> bool {
        false
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
    pub fn global_data(&self, id: GlobalID) -> &GlobalData<'hir> {
        &self.globals[id.index()]
    }
}

impl<'hir> ProcData<'hir> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
    pub fn param(&self, id: ParamID) -> &'hir Param<'hir> {
        &self.params[id.index()]
    }
    pub fn variable(&self, id: VariableID) -> &'hir Variable<'hir> {
        &self.variables[id.index()]
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
    pub fn equal(value: u64) -> Layout {
        Layout { size: value, align: value }
    }
}

impl<'hir> Type<'hir> {
    #[inline(always)]
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }
    #[inline(always)]
    pub fn is_void(&self) -> bool {
        matches!(self, Type::Void)
    }
    #[inline(always)]
    pub fn is_never(&self) -> bool {
        matches!(self, Type::Never)
    }
    #[inline(always)]
    pub fn unwrap_int(&self) -> IntType {
        match self {
            Type::Int(int_ty) => *int_ty,
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    pub fn unwrap_float(&self) -> FloatType {
        match self {
            Type::Float(float_ty) => *float_ty,
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    pub fn unwrap_bool(&self) -> BoolType {
        match self {
            Type::Bool(bool_ty) => *bool_ty,
            _ => unreachable!(),
        }
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

impl IntType {
    pub fn from_basic(basic: ast::BasicType) -> Option<IntType> {
        match basic {
            ast::BasicType::S8 => Some(IntType::S8),
            ast::BasicType::S16 => Some(IntType::S16),
            ast::BasicType::S32 => Some(IntType::S32),
            ast::BasicType::S64 => Some(IntType::S64),
            ast::BasicType::Ssize => Some(IntType::Ssize),
            ast::BasicType::U8 => Some(IntType::U8),
            ast::BasicType::U16 => Some(IntType::U16),
            ast::BasicType::U32 => Some(IntType::U32),
            ast::BasicType::U64 => Some(IntType::U64),
            ast::BasicType::Usize => Some(IntType::Usize),
            _ => None,
        }
    }

    pub fn into_basic(self) -> ast::BasicType {
        match self {
            IntType::S8 => ast::BasicType::S8,
            IntType::S16 => ast::BasicType::S16,
            IntType::S32 => ast::BasicType::S32,
            IntType::S64 => ast::BasicType::S64,
            IntType::Ssize => ast::BasicType::Ssize,
            IntType::U8 => ast::BasicType::U8,
            IntType::U16 => ast::BasicType::U16,
            IntType::U32 => ast::BasicType::U32,
            IntType::U64 => ast::BasicType::U64,
            IntType::Usize => ast::BasicType::Usize,
            IntType::Untyped => unreachable!("untyped int to basic type"),
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(self, IntType::S8 | IntType::S16 | IntType::S32 | IntType::S64 | IntType::Ssize)
    }

    pub fn min_128(self, ptr_width: TargetPtrWidth) -> i128 {
        match self {
            IntType::S8 => i8::MIN as i128,
            IntType::S16 => i16::MIN as i128,
            IntType::S32 => i32::MIN as i128,
            IntType::S64 => i64::MIN as i128,
            IntType::Ssize => match ptr_width {
                TargetPtrWidth::Bit_32 => i32::MIN as i128,
                TargetPtrWidth::Bit_64 => i64::MIN as i128,
            },
            IntType::U8 => 0,
            IntType::U16 => 0,
            IntType::U32 => 0,
            IntType::U64 => 0,
            IntType::Usize => 0,
            IntType::Untyped => -(u64::MAX as i128), // u64 magnitude
        }
    }

    pub fn max_128(self, ptr_width: TargetPtrWidth) -> i128 {
        match self {
            IntType::S8 => i8::MAX as i128,
            IntType::S16 => i16::MAX as i128,
            IntType::S32 => i32::MAX as i128,
            IntType::S64 => i64::MAX as i128,
            IntType::Ssize => match ptr_width {
                TargetPtrWidth::Bit_32 => i32::MAX as i128,
                TargetPtrWidth::Bit_64 => i64::MAX as i128,
            },
            IntType::U8 => u8::MAX as i128,
            IntType::U16 => u16::MAX as i128,
            IntType::U32 => u32::MAX as i128,
            IntType::U64 => u64::MAX as i128,
            IntType::Usize => match ptr_width {
                TargetPtrWidth::Bit_32 => u32::MAX as i128,
                TargetPtrWidth::Bit_64 => u64::MAX as i128,
            },
            IntType::Untyped => u64::MAX as i128, // u64 magnitude
        }
    }
}

impl FloatType {
    pub fn from_basic(basic: ast::BasicType) -> Option<FloatType> {
        match basic {
            ast::BasicType::F32 => Some(FloatType::F32),
            ast::BasicType::F64 => Some(FloatType::F64),
            _ => None,
        }
    }
    pub fn into_basic(self) -> ast::BasicType {
        match self {
            FloatType::F32 => ast::BasicType::F32,
            FloatType::F64 => ast::BasicType::F64,
            FloatType::Untyped => unreachable!(),
        }
    }
}

impl BoolType {
    pub fn from_basic(basic: ast::BasicType) -> Option<BoolType> {
        match basic {
            ast::BasicType::Bool => Some(BoolType::Bool),
            ast::BasicType::Bool32 => Some(BoolType::Bool32),
            _ => None,
        }
    }
    pub fn into_basic(self) -> ast::BasicType {
        match self {
            BoolType::Bool => ast::BasicType::Bool,
            BoolType::Bool16 => ast::BasicType::Bool16,
            BoolType::Bool32 => ast::BasicType::Bool32,
            BoolType::Bool64 => ast::BasicType::Bool64,
            BoolType::Untyped => unreachable!("untyped bool to basic type"),
        }
    }
}

impl StringType {
    pub fn from_basic(basic: ast::BasicType) -> Option<StringType> {
        match basic {
            ast::BasicType::String => Some(StringType::String),
            ast::BasicType::CString => Some(StringType::CString),
            _ => None,
        }
    }
    pub fn into_basic(self) -> ast::BasicType {
        match self {
            StringType::String => ast::BasicType::String,
            StringType::CString => ast::BasicType::CString,
            StringType::Untyped => unreachable!(),
        }
    }
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add_Int | BinOp::Add_Float => "+",
            BinOp::Sub_Int | BinOp::Sub_Float => "-",
            BinOp::Mul_Int | BinOp::Mul_Float => "*",
            BinOp::Div_Int(_) | BinOp::Div_Float => "/",
            BinOp::Rem_Int(_) => "%",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::BitShl => "<<",
            BinOp::BitShr(_) => ">>",
            BinOp::Eq_Int_Other(_) => "==",
            BinOp::NotEq_Int_Other(_) => "!=",
            BinOp::Cmp_Int(pred, _, _) => pred.as_str(),
            BinOp::Cmp_Float(pred, _, _) => pred.as_str(),
            BinOp::Cmp_String(pred, _, _) => pred.as_str(),
            BinOp::LogicAnd(_) => "&&",
            BinOp::LogicOr(_) => "||",
        }
    }
}

impl CmpPred {
    pub fn as_str(self) -> &'static str {
        match self {
            CmpPred::Eq => "==",
            CmpPred::NotEq => "!=",
            CmpPred::Less => "<",
            CmpPred::LessEq => "<=",
            CmpPred::Greater => ">",
            CmpPred::GreaterEq => ">=",
        }
    }
}
