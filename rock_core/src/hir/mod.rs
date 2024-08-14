pub mod intern;

use crate::arena::Arena;
use crate::ast;
use crate::bitset::BitSet;
use crate::id_impl;
use crate::intern::{InternID, InternPool};
use crate::session::ModuleID;
use intern::ConstInternPool;

pub struct Hir<'hir> {
    pub arena: Arena<'hir>,
    pub intern_name: InternPool<'hir>,
    pub intern_string: InternPool<'hir>,
    pub string_is_cstr: Vec<bool>,
    pub const_intern: ConstInternPool<'hir>,
    pub procs: Vec<ProcData<'hir>>,
    pub enums: Vec<EnumData<'hir>>,
    pub structs: Vec<StructData<'hir>>,
    pub consts: Vec<ConstData<'hir>>,
    pub globals: Vec<GlobalData<'hir>>,
    pub const_values: Vec<ConstValueID>,
}

id_impl!(ProcID);
pub struct ProcData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub params: &'hir [Param<'hir>],
    pub return_ty: Type<'hir>,
    pub block: Option<Block<'hir>>,
    pub locals: &'hir [&'hir Local<'hir>],
}

id_impl!(ParamID);
#[derive(Copy, Clone)]
pub struct Param<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
}

#[repr(u32)]
#[derive(Copy, Clone)]
pub enum ProcFlag {
    External,
    Variadic,
    Main,
    Test,
    Builtin,
    Inline,
}

id_impl!(EnumID);
pub struct EnumData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub int_ty: BasicInt,
    pub variants: &'hir [Variant<'hir>],
    pub size_eval: SizeEval,
}

id_impl!(VariantID);
#[derive(Copy, Clone)]
pub struct Variant<'hir> {
    pub name: ast::Name,
    pub kind: VariantKind<'hir>,
}

#[derive(Copy, Clone)]
pub enum VariantKind<'hir> {
    Default(ConstValue<'hir>),
    Constant(ConstEvalID),
    HasValues(&'hir [Type<'hir>]),
}

id_impl!(StructID);
pub struct StructData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub fields: &'hir [Field<'hir>],
    pub size_eval: SizeEval,
}

id_impl!(FieldID);
#[derive(Copy, Clone)]
pub struct Field<'hir> {
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub ty: Type<'hir>,
}

id_impl!(ConstID);
pub struct ConstData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: ConstEvalID,
}

id_impl!(GlobalID);
pub struct GlobalData<'hir> {
    pub origin_id: ModuleID,
    pub attr_set: BitSet,
    pub vis: ast::Vis,
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: ConstEvalID,
}

#[repr(u32)]
#[derive(Copy, Clone)]
pub enum GlobalFlag {
    ThreadLocal,
}

id_impl!(ConstEvalID);
#[derive(Copy, Clone)]
pub enum ConstEval<'ast> {
    Unresolved(ast::ConstExpr<'ast>),
    ResolvedError,
    ResolvedValue(ConstValueID),
}

#[derive(Copy, Clone)]
pub enum SizeEval {
    Unresolved,
    ResolvedError,
    Resolved(Size),
}

#[derive(Copy, Clone)]
pub struct Size {
    size: u64,
    align: u64,
}

#[derive(Copy, Clone)]
pub enum Type<'hir> {
    Error,
    Basic(ast::BasicType),
    Enum(EnumID),
    Struct(StructID),
    Reference(&'hir Type<'hir>, ast::Mut),
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
    Immediate(Option<u64>),
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
    Local(LocalID),
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
        local_id: LocalID,
        cond: &'hir Expr<'hir>,
        assign: &'hir Assign<'hir>,
    },
}

id_impl!(LocalID);
#[derive(Copy, Clone)]
pub struct Local<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: Option<&'hir Expr<'hir>>,
}

#[derive(Copy, Clone)]
pub struct Assign<'hir> {
    pub op: AssignOp,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
    pub lhs_ty: Type<'hir>,
}

id_impl!(ConstValueID);
#[rustfmt::skip]
#[derive(Copy, Clone, PartialEq)]
pub enum ConstValue<'hir> {
    Error,
    Null,
    Bool        { val: bool },
    Int         { val: u64, neg: bool, int_ty: BasicInt },
    Float       { val: f64, float_ty: BasicFloat },
    Char        { val: char },
    String      { id: InternID, c_string: bool },
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
    pub values: Option<&'hir [ConstValueID]>,
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstStruct<'hir> {
    pub struct_id: StructID,
    pub fields: &'hir [ConstValueID],
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstArray<'hir> {
    pub len: u64,
    pub values: &'hir [ConstValueID],
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Expr<'hir> {
    Error,
    Const        { value: ConstValue<'hir> },
    If           { if_: &'hir If<'hir> },
    Block        { block: Block<'hir> },
    Match        { match_: &'hir Match<'hir> },
    Match2       { match_: &'hir Match2<'hir> },
    StructField  { target: &'hir Expr<'hir>, struct_id: StructID, field_id: FieldID, deref: bool },
    SliceField   { target: &'hir Expr<'hir>, field: SliceField, deref: bool },
    Index        { target: &'hir Expr<'hir>, access: &'hir IndexAccess<'hir> },
    Slice        { target: &'hir Expr<'hir>, access: &'hir SliceAccess<'hir> },
    Cast         { target: &'hir Expr<'hir>, into: &'hir Type<'hir>, kind: CastKind },
    LocalVar     { local_id: LocalID },
    ParamVar     { param_id: ParamID },
    ConstVar     { const_id: ConstID },
    GlobalVar    { global_id: GlobalID },
    Variant      { enum_id: EnumID, variant_id: VariantID, input: Option<&'hir &'hir [&'hir Expr<'hir>]> },
    CallDirect   { proc_id: ProcID, input: &'hir [&'hir Expr<'hir>] },
    CallIndirect { target: &'hir Expr<'hir>, indirect: &'hir CallIndirect<'hir> },
    StructInit   { struct_id: StructID, input: &'hir [FieldInit<'hir>] },
    ArrayInit    { array_init: &'hir ArrayInit<'hir> },
    ArrayRepeat  { array_repeat: &'hir ArrayRepeat<'hir> },
    Deref        { rhs: &'hir Expr<'hir>, ptr_ty: &'hir Type<'hir> },
    Address      { rhs: &'hir Expr<'hir> },
    Unary        { op: UnOp, rhs: &'hir Expr<'hir> },
    Binary       { op: BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir> },
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

#[derive(Copy, Clone)]
pub struct Match<'hir> {
    pub on_expr: &'hir Expr<'hir>,
    pub arms: &'hir [MatchArm<'hir>],
    pub fallback: Option<Block<'hir>>,
}

#[derive(Copy, Clone)]
pub struct MatchArm<'hir> {
    pub pat: ConstValueID,
    pub block: Block<'hir>,
    pub unreachable: bool,
}

#[derive(Copy, Clone)]
pub struct Match2<'hir> {
    pub on_expr: &'hir Expr<'hir>,
    pub arms: &'hir [MatchArm2<'hir>],
}

#[derive(Copy, Clone)]
pub struct MatchArm2<'hir> {
    pub pat: Pat<'hir>,
    pub block: Block<'hir>,
    pub unreachable: bool,
}

#[derive(Copy, Clone)]
pub enum Pat<'hir> {
    Error,
    Wild,
    Lit(ConstValue<'hir>),
    Const(ConstID),
    Variant(EnumID, VariantID), //@binds
    Or(&'hir [Pat<'hir>]),
}

#[derive(Copy, Clone)]
pub enum SliceField {
    Ptr,
    Len,
}

#[derive(Copy, Clone)]
pub struct IndexAccess<'hir> {
    pub deref: bool,
    pub elem_ty: Type<'hir>,
    pub kind: IndexKind<'hir>,
    pub index: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub enum IndexKind<'hir> {
    Slice { elem_size: u64 },
    Array { array: &'hir ArrayStatic<'hir> },
}

#[derive(Copy, Clone)]
pub struct SliceAccess<'hir> {
    pub deref: bool,
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
    pub expr: &'hir Expr<'hir>,
    pub len: u64,
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

#[derive(Copy, Clone)]
pub enum BasicIntSigned {
    S8,
    S16,
    S32,
    S64,
    Ssize,
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

use crate::size_assert;
size_assert!(16, ConstEval);
size_assert!(16, ConstValue);
size_assert!(16, Type);
size_assert!(16, Stmt);
size_assert!(24, Expr);

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

impl<'hir> ProcData<'hir> {
    pub fn param(&self, id: ParamID) -> &'hir Param<'hir> {
        &self.params[id.index()]
    }
    pub fn local(&self, id: LocalID) -> &'hir Local<'hir> {
        self.locals[id.index()]
    }
    pub fn find_param(&self, id: InternID) -> Option<(ParamID, &'hir Param<'hir>)> {
        for (idx, param) in self.params.iter().enumerate() {
            if param.name.id == id {
                return Some((ParamID::new(idx), param));
            }
        }
        None
    }
}

impl<'hir> EnumData<'hir> {
    pub fn variant(&self, id: VariantID) -> &'hir Variant<'hir> {
        &self.variants[id.index()]
    }
    pub fn find_variant(&self, id: InternID) -> Option<(VariantID, &'hir Variant<'hir>)> {
        for (idx, variant) in self.variants.iter().enumerate() {
            if variant.name.id == id {
                return Some((VariantID::new(idx), variant));
            }
        }
        None
    }
}

impl<'hir> StructData<'hir> {
    pub fn field(&self, id: FieldID) -> &'hir Field<'hir> {
        &self.fields[id.index()]
    }
    pub fn find_field(&self, id: InternID) -> Option<(FieldID, &'hir Field<'hir>)> {
        for (idx, field) in self.fields.iter().enumerate() {
            if field.name.id == id {
                return Some((FieldID::new(idx), field));
            }
        }
        None
    }
}

impl SizeEval {
    pub fn get_size(self) -> Option<Size> {
        match self {
            SizeEval::Unresolved => None,
            SizeEval::ResolvedError => None,
            SizeEval::Resolved(size) => Some(size),
        }
    }
}

impl Size {
    pub fn new(size: u64, align: u64) -> Size {
        Size { size, align }
    }
    pub fn new_equal(size_align: u64) -> Size {
        Size {
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
    pub const VOID: Type<'static> = Type::Basic(ast::BasicType::Void);
    pub const BOOL: Type<'static> = Type::Basic(ast::BasicType::Bool);
    pub const USIZE: Type<'static> = Type::Basic(ast::BasicType::Usize);

    pub fn is_error(self) -> bool {
        matches!(self, Type::Error)
    }
    pub fn is_void(self) -> bool {
        matches!(self, Type::Basic(ast::BasicType::Void))
    }
    pub fn is_never(self) -> bool {
        matches!(self, Type::Basic(ast::BasicType::Never))
    }
}

impl BasicInt {
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
}

impl BasicIntSigned {
    pub fn from_basic(basic: ast::BasicType) -> Option<BasicIntSigned> {
        match basic {
            ast::BasicType::S8 => Some(BasicIntSigned::S8),
            ast::BasicType::S16 => Some(BasicIntSigned::S16),
            ast::BasicType::S32 => Some(BasicIntSigned::S32),
            ast::BasicType::S64 => Some(BasicIntSigned::S64),
            ast::BasicType::Ssize => Some(BasicIntSigned::Ssize),
            _ => None,
        }
    }

    pub fn into_basic(self) -> ast::BasicType {
        match self {
            BasicIntSigned::S8 => ast::BasicType::S8,
            BasicIntSigned::S16 => ast::BasicType::S16,
            BasicIntSigned::S32 => ast::BasicType::S32,
            BasicIntSigned::S64 => ast::BasicType::S64,
            BasicIntSigned::Ssize => ast::BasicType::Ssize,
        }
    }

    pub fn into_int(self) -> BasicInt {
        match self {
            BasicIntSigned::S8 => BasicInt::S8,
            BasicIntSigned::S16 => BasicInt::S16,
            BasicIntSigned::S32 => BasicInt::S32,
            BasicIntSigned::S64 => BasicInt::S64,
            BasicIntSigned::Ssize => BasicInt::Ssize,
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
