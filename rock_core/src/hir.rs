use crate::ast;
use crate::error::SourceRange;
use crate::intern::{LitID, NameID};
use crate::session::config::TargetPtrWidth;
use crate::session::{ModuleID, Session};
use crate::support::{Arena, AsStr, BitSet};
use crate::text::{self, TextOffset, TextRange};
use rustc_hash::FxHashMap;

pub struct Hir<'hir> {
    pub arena: Arena<'hir>,
    pub procs: Vec<ProcData<'hir>>,
    pub enums: Vec<EnumData<'hir>>,
    pub structs: Vec<StructData<'hir>>,
    pub globals: Vec<GlobalData<'hir>>,
    pub const_eval_values: Vec<ConstValue<'hir>>,
    pub variant_eval_values: Vec<ConstValue<'hir>>,
    pub entry_point: Option<ProcID>,
    pub enum_layout: FxHashMap<EnumKey<'hir>, Layout>,
    pub struct_layout: FxHashMap<StructKey<'hir>, StructLayout<'hir>>,
    pub variant_layout: FxHashMap<VariantKey<'hir>, StructLayout<'hir>>,
    pub core: CoreItems,
}

pub struct CoreItems {
    pub start: ProcID,
    pub index_out_of_bounds: ProcID,
    pub slice_range: ProcID,
    pub string_equals: ProcID,
    pub cstring_equals: ProcID,
    pub from_raw_parts: ProcID,
    pub range_bound: Option<EnumID>,
    pub array: StructID,
    pub values: ProcID,
    pub values_mut: ProcID,
    pub option: EnumID,
    pub result: EnumID,
    pub any: Option<StructID>,
    pub type_info: EnumID,
    pub int_ty: EnumID,
    pub float_ty: EnumID,
    pub bool_ty: EnumID,
    pub string_ty: EnumID,
    pub type_info_enum: StructID,
    pub type_info_variant: StructID,
    pub type_info_variant_field: StructID,
    pub type_info_struct: StructID,
    pub type_info_field: StructID,
    pub type_info_reference: StructID,
    pub type_info_procedure: StructID,
    pub type_info_array_slice: StructID,
    pub type_info_array_static: StructID,
    pub source_loc: Option<StructID>,
    pub exchange_res: StructID,
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
    pub kind: ParamKind,
}

crate::enum_as_str! {
    #[derive(Copy, Clone, Hash, Eq,  Debug, PartialEq)]
    pub enum ParamKind {
        Normal "normal",
        Variadic "variadic",
        CallerLocation "caller_location",
    }
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
    pub type_usage: Eval<(), ()>,
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
    pub type_usage: Eval<(), ()>,
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

#[derive(Copy, Clone)]
pub struct StructLayout<'hir> {
    pub total: Layout,
    pub field_pad: &'hir [u8],
    pub field_offset: &'hir [u64],
}

pub type ProcKey<'hir> = (ProcID, &'hir [Type<'hir>]);
pub type EnumKey<'hir> = (EnumID, &'hir [Type<'hir>]);
pub type StructKey<'hir> = (StructID, &'hir [Type<'hir>]);
pub type VariantKey<'hir> = (EnumID, VariantID, &'hir [Type<'hir>]);

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
    Int(IntType),
    Float(FloatType),
    Bool(BoolType),
    String(StringType),
    PolyProc(ProcID, usize),
    PolyEnum(EnumID, usize),
    PolyStruct(StructID, usize),
    Enum(EnumID, &'hir [Type<'hir>]),
    Struct(StructID, &'hir [Type<'hir>]),
    Reference(ast::Mut, &'hir Type<'hir>),
    MultiReference(ast::Mut, &'hir Type<'hir>),
    Procedure(&'hir ProcType<'hir>),
    ArraySlice(&'hir ArraySlice<'hir>),
    ArrayStatic(&'hir ArrayStatic<'hir>),
    ArrayEnumerated(&'hir ArrayEnumerated<'hir>),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ProcType<'hir> {
    pub flag_set: BitSet<ProcFlag>,
    pub params: &'hir [ProcTypeParam<'hir>],
    pub return_ty: Type<'hir>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ProcTypeParam<'hir> {
    pub ty: Type<'hir>,
    pub kind: ParamKind,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ArraySlice<'hir> {
    pub mutt: ast::Mut,
    pub elem_ty: Type<'hir>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ArrayStatic<'hir> {
    pub len: ArrayStaticLen,
    pub elem_ty: Type<'hir>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum ArrayStaticLen {
    Immediate(u64),
    ConstEval(ConstEvalID),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub struct ArrayEnumerated<'hir> {
    pub enum_id: EnumID,
    pub elem_ty: Type<'hir>,
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
    pub op: AssignOp<'hir>,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
    pub lhs_ty: Type<'hir>, //@can be removed, infer based on binary op
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Expr<'hir> {
    Error,
    Const        (ConstValue<'hir>, ConstID),
    If           { if_: &'hir If<'hir> },
    Block        { block: Block<'hir> },
    Match        { match_: &'hir Match<'hir> },
    StructField  { target: &'hir Expr<'hir>, access: &'hir StructFieldAccess<'hir> },
    SliceField   { target: &'hir Expr<'hir>, access: SliceFieldAccess },
    Index        { target: &'hir Expr<'hir>, access: &'hir IndexAccess<'hir> },
    Slice        { target: &'hir Expr<'hir>, access: &'hir SliceAccess<'hir> },
    Cast         { target: &'hir Expr<'hir>, into: &'hir Type<'hir>, kind: CastKind },
    ParamVar     { param_id: ParamID },
    Variable     { var_id: VariableID },
    GlobalVar    { global_id: GlobalID },
    Variant      { enum_id: EnumID, variant_id: VariantID, input: &'hir (&'hir [&'hir Expr<'hir>], &'hir [Type<'hir>]) },
    CallDirect   { proc_id: ProcID, input: &'hir [&'hir Expr<'hir>] },
    CallDirectPoly { proc_id: ProcID, input: &'hir (&'hir [&'hir Expr<'hir>], &'hir [Type<'hir>]) },
    CallIndirect { target: &'hir Expr<'hir>, indirect: &'hir CallIndirect<'hir> },
    VariadicArg  { arg: &'hir VariadicArg<'hir> },
    StructInit   { struct_id: StructID, input: &'hir [&'hir Expr<'hir>] },
    StructInitPoly { struct_id: StructID, input: &'hir (&'hir [&'hir Expr<'hir>], &'hir [Type<'hir>]) },
    ArrayInit    { array: &'hir ArrayInit<'hir> },
    ArrayRepeat  { array: &'hir ArrayRepeat<'hir> },
    Deref        { rhs: &'hir Expr<'hir>,  mutt: ast::Mut, ref_ty: &'hir Type<'hir> },
    Address      { rhs: &'hir Expr<'hir> },
    Unary        { op: UnOp, rhs: &'hir Expr<'hir> },
    Binary       { op: BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir> },
    ArrayBinary  { op: BinOp, array: &'hir  ArrayBinary<'hir> },
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ConstValue<'hir> {
    Void,
    Null,
    Bool        { val: bool, bool_ty: BoolType },
    Int         { val: u64, neg: bool, int_ty: IntType },
    Float       { val: f64, float_ty: FloatType },
    Char        { val: char, untyped: bool },
    String      { val: LitID, string_ty: StringType },
    Procedure   { proc_id: ProcID, poly_types: Option<&'hir &'hir [Type<'hir>]> },
    Variant     { enum_id: EnumID, variant_id: VariantID },
    VariantPoly { enum_id: EnumID, variant: &'hir ConstVariant<'hir> },
    Struct      { struct_id: StructID, struct_: &'hir ConstStruct<'hir> },
    Array       { array: &'hir ConstArray<'hir> },
    ArrayRepeat { array: &'hir ConstArrayRepeat<'hir> },
    ArrayEmpty  { elem_ty: &'hir Type<'hir> },
    GlobalIndex { global_id: GlobalID, index: u64 },
    GlobalSlice { global_id: GlobalID, index: u32, len: u32 },
}

#[derive(Copy, Clone)]
pub struct ConstVariant<'hir> {
    pub variant_id: VariantID,
    pub values: &'hir [ConstValue<'hir>],
    pub poly_types: &'hir [Type<'hir>],
}

#[derive(Copy, Clone)]
pub struct ConstStruct<'hir> {
    pub values: &'hir [ConstValue<'hir>],
    pub poly_types: &'hir [Type<'hir>],
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

#[derive(Copy, Clone)]
pub struct Match<'hir> {
    pub kind: MatchKind<'hir>,
    pub on_expr: &'hir Expr<'hir>,
    pub arms: &'hir [MatchArm<'hir>],
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum MatchKind<'hir> {
    Int { int_ty: IntType },
    Bool { bool_ty: BoolType },
    Char,
    String,
    Enum { enum_id: EnumID, ref_mut: Option<ast::Mut>, poly_types: Option<&'hir &'hir [Type<'hir>]> },
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
pub struct StructFieldAccess<'hir> {
    pub deref: Option<ast::Mut>,
    pub struct_id: StructID,
    pub field_id: FieldID,
    pub field_ty: Type<'hir>,
    pub poly_types: &'hir [Type<'hir>],
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
    pub offset: TextOffset,
}

#[derive(Copy, Clone)]
pub enum IndexKind {
    Multi(ast::Mut),
    Slice(ast::Mut),
    Array(ArrayStaticLen),
    ArrayEnum(EnumID),
    ArrayCore,
}

#[derive(Copy, Clone)]
pub struct SliceAccess<'hir> {
    pub deref: Option<ast::Mut>,
    pub kind: SliceKind,
    pub op_call: Expr<'hir>,
}

#[derive(Copy, Clone)]
pub enum SliceKind {
    Slice(ast::Mut),
    Array,
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

    StringUntyped_NoOp,

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
pub struct VariadicArg<'hir> {
    pub exprs: &'hir [&'hir Expr<'hir>],
    pub types: &'hir [Type<'hir>],
}

#[derive(Copy, Clone)]
pub struct ArrayInit<'hir> {
    pub elem_ty: Type<'hir>,
    pub input: &'hir [&'hir Expr<'hir>],
}

#[derive(Copy, Clone)]
pub struct ArrayRepeat<'hir> {
    pub len: u64,
    pub elem_ty: Type<'hir>,
    pub value: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub struct ArrayBinary<'hir> {
    pub len: u64,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum Vis {
        Public "public",
        Package "package",
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
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
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
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
    pub enum FloatType {
        F32 "f32",
        F64 "f64",
        Untyped "untyped float",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
    pub enum BoolType {
        Bool "bool",
        Bool16 "bool16",
        Bool32 "bool32",
        Bool64 "bool64",
        Untyped "untyped bool",
    }
}

crate::enum_as_str! {
    #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
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
    Neg_Int(IntType),
    Neg_Float(FloatType),
    BitNot(IntType),
    LogicNot(BoolType),
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
#[allow(non_camel_case_types)]
pub enum BinOp {
    Add_Int(IntType), Add_Float(FloatType),
    Sub_Int(IntType), Sub_Float(FloatType),
    Mul_Int(IntType), Mul_Float(FloatType),
    Div_Int(IntType), Div_Float(FloatType),
    Rem_Int(IntType),
    BitAnd(IntType),
    BitOr(IntType),
    BitXor(IntType),
    BitShl(IntType, CastKind),
    BitShr(IntType, CastKind),
    Eq_Int_Other(BoolType),
    NotEq_Int_Other(BoolType),
    Cmp_Int(CmpPred, BoolType, IntType),
    Cmp_Float(CmpPred, BoolType, FloatType),
    Cmp_String(CmpPred, BoolType, StringType),
    LogicAnd(BoolType),
    LogicOr(BoolType),
}

crate::enum_as_str! {
    #[derive(Copy, Clone, PartialEq)]
    pub enum CmpPred {
        Eq "==",
        NotEq "!=",
        Less "<",
        LessEq "<=",
        Greater ">",
        GreaterEq ">=",
    }
}

//@bad representation, lhs and rhs is duplicated in `ArrayBinary` only len is relevant.
#[derive(Copy, Clone)]
pub enum AssignOp<'hir> {
    Assign,
    Bin(BinOp, Option<&'hir ArrayBinary<'hir>>),
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
crate::size_lock!(24, Expr);
crate::size_lock!(16, ConstValue);

//==================== ITEM FLAGS ====================

crate::enum_as_str! {
    #[derive(Copy, Clone, Hash, Eq,  Debug, PartialEq)]
    pub enum ProcFlag {
        // directives
        InlineNever "inline_never",
        InlineAlways "inline_always",
        Intrinsic "intrinsic",
        // compile-time flags
        WasUsed "was used",
        External "external",
        Variadic "variadic",
        CVariadic "c_variadic",
        EntryPoint "entry point",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone)]
    pub enum EnumFlag {
        WasUsed "was used",
        WithFields "with fields",
        WithConstantInit "with constant initializers",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone)]
    pub enum StructFlag {
        WasUsed "was used",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone)]
    pub enum ConstFlag {
        WasUsed "was used",
    }
}
crate::enum_as_str! {
    #[derive(Copy, Clone)]
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

impl ConstData<'_> {
    pub fn src(&self) -> SourceRange {
        SourceRange::new(self.origin_id, self.name.range)
    }
}

impl GlobalData<'_> {
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

impl Type<'_> {
    #[inline(always)]
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }
    #[inline(always)]
    pub fn is_unknown(&self) -> bool {
        matches!(self, Type::Unknown)
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
            Type::Int(ty) => *ty,
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    pub fn unwrap_float(&self) -> FloatType {
        match self {
            Type::Float(ty) => *ty,
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    pub fn unwrap_bool(&self) -> BoolType {
        match self {
            Type::Bool(ty) => *ty,
            _ => unreachable!(),
        }
    }
    #[inline(always)]
    pub fn unwrap_string(&self) -> StringType {
        match self {
            Type::String(ty) => *ty,
            _ => unreachable!(),
        }
    }
}

impl<'hir> ConstValue<'hir> {
    pub fn from_u64(val: u64, int_ty: IntType) -> ConstValue<'hir> {
        ConstValue::Int { val, neg: false, int_ty }
    }
    pub fn from_i64(val: i64, int_ty: IntType) -> ConstValue<'hir> {
        if val < 0 {
            ConstValue::Int { val: -val as u64, neg: true, int_ty }
        } else {
            ConstValue::Int { val: val as u64, neg: false, int_ty }
        }
    }
    pub fn into_bool(&self) -> bool {
        match *self {
            ConstValue::Bool { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn expect_bool(self) -> (bool, BoolType) {
        match self {
            ConstValue::Bool { val, bool_ty } => (val, bool_ty),
            _ => unreachable!(),
        }
    }
    pub fn into_int(&self) -> i128 {
        match *self {
            ConstValue::Int { val, neg, .. } => {
                if neg {
                    -(val as i128)
                } else {
                    val as i128
                }
            }
            _ => unreachable!(),
        }
    }
    pub fn into_int_u64(&self) -> u64 {
        match *self {
            ConstValue::Int { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_int_i64(&self) -> i64 {
        match *self {
            ConstValue::Int { val, neg, .. } => {
                if neg {
                    -(val as i64)
                } else {
                    val as i64
                }
            }
            _ => unreachable!(),
        }
    }
    pub fn into_int_ty(&self) -> IntType {
        match *self {
            ConstValue::Int { int_ty, .. } => int_ty,
            _ => unreachable!(),
        }
    }
    pub fn into_float(&self) -> f64 {
        match *self {
            ConstValue::Float { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_float_ty(&self) -> FloatType {
        match *self {
            ConstValue::Float { float_ty, .. } => float_ty,
            _ => unreachable!(),
        }
    }
    pub fn into_char(&self) -> char {
        match *self {
            ConstValue::Char { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_string(&self) -> LitID {
        match *self {
            ConstValue::String { val, .. } => val,
            _ => unreachable!(),
        }
    }
    pub fn into_enum(self) -> (EnumID, VariantID) {
        match self {
            ConstValue::Variant { enum_id, variant_id } => (enum_id, variant_id),
            _ => unreachable!(),
        }
    }
}

impl Vis {
    pub fn stronger(self, other: Vis) -> bool {
        match self {
            Vis::Public => false,
            Vis::Package => matches!(other, Vis::Public),
            Vis::Private => matches!(other, Vis::Public | Vis::Package),
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
            Eval::Unresolved(_) => unreachable!("expected resolved"),
            Eval::Resolved(val) => Ok(*val),
            Eval::ResolvedError => Err(()),
        }
    }
    pub fn resolved_unwrap(&self) -> R {
        match self {
            Eval::Resolved(val) => *val,
            Eval::Unresolved(_) | Eval::ResolvedError => unreachable!("expected resolved"),
        }
    }
    pub fn unresolved_unwrap(&self) -> U {
        match self {
            Eval::Unresolved(val) => *val,
            Eval::Resolved(_) | Eval::ResolvedError => unreachable!("expected unresolved"),
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

impl BoolType {
    pub fn int_equivalent(self) -> IntType {
        match self {
            BoolType::Bool => IntType::U8,
            BoolType::Bool16 => IntType::U16,
            BoolType::Bool32 => IntType::U32,
            BoolType::Bool64 => IntType::U64,
            BoolType::Untyped => unreachable!(),
        }
    }
}

impl BinOp {
    pub fn as_str(self) -> &'static str {
        match self {
            BinOp::Add_Int(_) | BinOp::Add_Float(_) => "+",
            BinOp::Sub_Int(_) | BinOp::Sub_Float(_) => "-",
            BinOp::Mul_Int(_) | BinOp::Mul_Float(_) => "*",
            BinOp::Div_Int(_) | BinOp::Div_Float(_) => "/",
            BinOp::Rem_Int(_) => "%",
            BinOp::BitAnd(_) => "&",
            BinOp::BitOr(_) => "|",
            BinOp::BitXor(_) => "^",
            BinOp::BitShl(_, _) => "<<",
            BinOp::BitShr(_, _) => ">>",
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

pub fn source_location<'hir>(
    session: &mut Session,
    origin_id: ModuleID,
    offset: TextOffset,
) -> [ConstValue<'hir>; 3] {
    let module = session.module.get(origin_id);

    let path = if module.origin == session.root_id {
        let package = session.graph.package(module.origin);
        module.file.path.strip_prefix(&package.root_dir).unwrap()
    } else {
        &module.file.path
    };

    // normalize windows and unix style path separators for tests to be uniform
    #[cfg(target_os = "windows")]
    let path_display = path.to_string_lossy().replace('\\', "/");
    #[cfg(not(target_os = "windows"))]
    let path_display = path.to_string_lossy();
    let path_id = session.intern_lit.intern(&path_display);

    let location = text::find_text_location(&module.file.source, offset, &module.file.line_ranges);
    let line = ConstValue::Int { val: location.line() as u64, neg: false, int_ty: IntType::U32 };
    let column = ConstValue::Int { val: location.col() as u64, neg: false, int_ty: IntType::U32 };
    let filename = ConstValue::String { val: path_id, string_ty: StringType::String };
    [line, column, filename]
}
