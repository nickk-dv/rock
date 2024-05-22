pub mod intern;

use crate::arena::Arena;
use crate::ast;
use crate::id_impl;
use crate::intern::{InternID, InternPool};
use crate::session::FileID;
use intern::ConstInternPool;

pub struct Hir<'hir> {
    pub arena: Arena<'hir>,
    pub intern: InternPool<'hir>,
    pub const_intern: ConstInternPool<'hir>,
    pub modules: Vec<ModuleData>,
    pub procs: Vec<ProcData<'hir>>,
    pub enums: Vec<EnumData<'hir>>,
    pub unions: Vec<UnionData<'hir>>,
    pub structs: Vec<StructData<'hir>>,
    pub consts: Vec<ConstData<'hir>>,
    pub globals: Vec<GlobalData<'hir>>,
    pub const_values: Vec<ConstValueID>,
}

id_impl!(ModuleID);
pub struct ModuleData {
    pub file_id: FileID,
}

id_impl!(ProcID);
pub struct ProcData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub params: &'hir [ProcParam<'hir>],
    pub is_variadic: bool,
    pub return_ty: Type<'hir>,
    pub block: Option<Block<'hir>>,
    pub locals: &'hir [&'hir Local<'hir>],
    pub is_test: bool,
    pub is_main: bool,
}

id_impl!(ProcParamID);
#[derive(Copy, Clone)]
pub struct ProcParam<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
}

id_impl!(EnumID);
pub struct EnumData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub basic: ast::BasicType,
    pub variants: &'hir [EnumVariant],
}

id_impl!(EnumVariantID);
#[derive(Copy, Clone)]
pub struct EnumVariant {
    pub name: ast::Name,
    pub value: ConstEvalID,
}

id_impl!(UnionID);
pub struct UnionData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub members: &'hir [UnionMember<'hir>],
    pub size_eval: SizeEval,
}

id_impl!(UnionMemberID);
#[derive(Copy, Clone)]
pub struct UnionMember<'hir> {
    pub name: ast::Name,
    pub ty: Type<'hir>,
}

id_impl!(StructID);
pub struct StructData<'hir> {
    pub origin_id: ModuleID,
    pub vis: ast::Vis,
    pub name: ast::Name,
    pub fields: &'hir [StructField<'hir>],
    pub size_eval: SizeEval,
}

id_impl!(StructFieldID);
#[derive(Copy, Clone)]
pub struct StructField<'hir> {
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
    pub vis: ast::Vis,
    pub mutt: ast::Mut,
    pub name: ast::Name,
    pub ty: Type<'hir>,
    pub value: ConstEvalID,
    pub thread_local: bool,
}

id_impl!(ConstEvalID);
#[derive(Copy, Clone)]
pub enum ConstEval<'ast> {
    Error,
    Unresolved(ast::ConstExpr<'ast>),
    ResolvedValue(ConstValueID),
}

#[derive(Copy, Clone)]
pub enum SizeEval {
    Unresolved,
    ResolvedError,
    Resolved(Size),
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

#[derive(Copy, Clone)]
pub struct Size {
    size: u64,
    align: u64,
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

#[derive(Copy, Clone)]
pub enum Type<'hir> {
    Error,
    Basic(ast::BasicType),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
    Reference(&'hir Type<'hir>, ast::Mut),
    Procedure(&'hir ProcType<'hir>),
    ArraySlice(&'hir ArraySlice<'hir>),
    ArrayStatic(&'hir ArrayStatic<'hir>),
}

impl<'hir> Type<'hir> {
    #[inline]
    pub fn is_error(self) -> bool {
        matches!(self, Type::Error)
    }
    #[inline]
    pub fn is_void(self) -> bool {
        matches!(self, Type::Basic(ast::BasicType::Void))
    }
}

#[derive(Copy, Clone)]
pub struct ProcType<'hir> {
    pub params: &'hir [Type<'hir>],
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
pub enum Stmt<'hir> {
    Break,
    Continue,
    Return(Option<&'hir Expr<'hir>>),
    Defer(&'hir Block<'hir>),
    ForLoop(&'hir For<'hir>),
    Local(LocalID),
    Assign(&'hir Assign<'hir>),
    ExprSemi(&'hir Expr<'hir>),
    ExprTail(&'hir Expr<'hir>),
}

#[derive(Copy, Clone)]
pub struct For<'hir> {
    pub kind: ForKind<'hir>,
    pub block: Block<'hir>,
}

#[derive(Copy, Clone)]
pub enum ForKind<'hir> {
    Loop,
    While {
        cond: &'hir Expr<'hir>,
    },
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
    pub op: ast::AssignOp,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
    pub lhs_ty: Type<'hir>,
    pub lhs_signed_int: bool,
}

#[derive(Copy, Clone)]
pub struct Block<'hir> {
    pub stmts: &'hir [Stmt<'hir>],
}

id_impl!(ConstValueID);
#[rustfmt::skip]
#[derive(Copy, Clone, PartialEq)]
pub enum ConstValue<'hir> {
    Error,
    Null,
    Bool        { val: bool },
    /// Option<BasicType> means untyped float or integer currently
    Int         { val: u64, neg: bool, ty: Option<ast::BasicType> },
    Float       { val: f64, ty: Option<ast::BasicType> },
    Char        { val: char },
    String      { id: InternID, c_string: bool },
    Procedure   { proc_id: ProcID },
    Struct      { struct_: &'hir ConstStruct<'hir> },
    Array       { array: &'hir ConstArray<'hir> },
    ArrayRepeat { value: ConstValueID, len: u64 },
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstStruct<'hir> {
    struct_id: StructID,
    fields: &'hir [ConstValueID],
}

#[derive(Copy, Clone, PartialEq)]
pub struct ConstArray<'hir> {
    len: u64,
    values: &'hir [ConstValueID],
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Expr<'hir> {
    Error,
    LitNull,
    LitBool      { val: bool },
    LitInt       { val: u64, ty: ast::BasicType },
    LitFloat     { val: f64, ty: ast::BasicType },
    LitChar      { val: char },
    LitString    { id: InternID, c_string: bool },
    If           { if_: &'hir If<'hir> },
    Block        { block: Block<'hir> },
    Match        { match_: &'hir Match<'hir> },
    UnionMember  { target: &'hir Expr<'hir>, union_id: UnionID, member_id: UnionMemberID, deref: bool },
    StructField  { target: &'hir Expr<'hir>, struct_id: StructID, field_id: StructFieldID, deref: bool },
    SliceField   { target: &'hir Expr<'hir>, first_ptr: bool, deref: bool },
    Index        { target: &'hir Expr<'hir>, access: &'hir IndexAccess<'hir> },
    Slice        { target: &'hir Expr<'hir>, access: &'hir SliceAccess<'hir> },
    Cast         { target: &'hir Expr<'hir>, into: &'hir Type<'hir>, kind: CastKind },
    LocalVar     { local_id: LocalID },
    ParamVar     { param_id: ProcParamID },
    ConstVar     { const_id: ConstID },
    GlobalVar    { global_id: GlobalID },
    Procedure    { proc_id: ProcID },
    CallDirect   { proc_id: ProcID, input: &'hir [&'hir Expr<'hir>] },
    CallIndirect { target: &'hir Expr<'hir>, indirect: &'hir CallIndirect<'hir> },
    EnumVariant  { enum_id: EnumID, variant_id: EnumVariantID },
    UnionInit    { union_id: UnionID, input: UnionMemberInit<'hir> },
    StructInit   { struct_id: StructID, input: &'hir [StructFieldInit<'hir>] },
    ArrayInit    { array_init: &'hir ArrayInit<'hir> },
    ArrayRepeat  { array_repeat: &'hir ArrayRepeat<'hir> },
    Address      { rhs: &'hir Expr<'hir> },
    Unary        { op: ast::UnOp, rhs: &'hir Expr<'hir> },
    Binary       { op: ast::BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir>, lhs_signed_int: bool },
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
}

//@fallback `pat` could be stored separately, as optional
// similar to structure of `if`
// this removes invariant on resolved match arm list
// comeback to this when proper match validation is done @03.04.24
#[derive(Copy, Clone)]
pub struct MatchArm<'hir> {
    pub pat: Option<&'hir Expr<'hir>>,
    pub expr: &'hir Expr<'hir>,
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
    Integer_Trunc,
    Sint_Sign_Extend,
    Uint_Zero_Extend,
    Float_to_Sint,
    Float_to_Uint,
    Sint_to_Float,
    Uint_to_Float,
    Float_Trunc,
    Float_Extend,
}

#[derive(Copy, Clone)]
pub struct CallIndirect<'hir> {
    pub proc_ty: &'hir ProcType<'hir>,
    pub input: &'hir [&'hir Expr<'hir>],
}

#[derive(Copy, Clone)]
pub struct UnionMemberInit<'hir> {
    pub member_id: UnionMemberID,
    pub expr: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub struct StructFieldInit<'hir> {
    pub field_id: StructFieldID,
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
    pub len: Option<u64>,
}

use crate::size_assert;
size_assert!(16, ConstEval);
size_assert!(16, ConstValue);
size_assert!(16, Type);
size_assert!(16, Stmt);
size_assert!(24, Expr);

impl<'hir> Hir<'hir> {
    pub fn module_data(&self, id: ModuleID) -> &ModuleData {
        &self.modules[id.index()]
    }
    pub fn proc_data(&self, id: ProcID) -> &ProcData<'hir> {
        &self.procs[id.index()]
    }
    pub fn enum_data(&self, id: EnumID) -> &EnumData<'hir> {
        &self.enums[id.index()]
    }
    pub fn union_data(&self, id: UnionID) -> &UnionData<'hir> {
        &self.unions[id.index()]
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
}

impl<'hir> ProcData<'hir> {
    pub fn param(&self, id: ProcParamID) -> &'hir ProcParam<'hir> {
        &self.params[id.index()]
    }
    pub fn local(&self, id: LocalID) -> &'hir Local<'hir> {
        &self.locals[id.index()]
    }
    pub fn find_param(&self, id: InternID) -> Option<(ProcParamID, &'hir ProcParam<'hir>)> {
        for (idx, param) in self.params.iter().enumerate() {
            if param.name.id == id {
                return Some((ProcParamID::new(idx), param));
            }
        }
        None
    }
}

impl<'hir> EnumData<'hir> {
    pub fn variant(&self, id: EnumVariantID) -> &'hir EnumVariant {
        &self.variants[id.index()]
    }
    pub fn find_variant(&self, id: InternID) -> Option<(EnumVariantID, &'hir EnumVariant)> {
        for (idx, variant) in self.variants.iter().enumerate() {
            if variant.name.id == id {
                return Some((EnumVariantID::new(idx), variant));
            }
        }
        None
    }
}

impl<'hir> UnionData<'hir> {
    pub fn member(&self, id: UnionMemberID) -> &'hir UnionMember<'hir> {
        &self.members[id.index()]
    }
    pub fn find_member(&self, id: InternID) -> Option<(UnionMemberID, &'hir UnionMember<'hir>)> {
        for (idx, member) in self.members.iter().enumerate() {
            if member.name.id == id {
                return Some((UnionMemberID::new(idx), member));
            }
        }
        None
    }
}

impl<'hir> StructData<'hir> {
    pub fn field(&self, id: StructFieldID) -> &'hir StructField<'hir> {
        &self.fields[id.index()]
    }
    pub fn find_field(&self, id: InternID) -> Option<(StructFieldID, &'hir StructField<'hir>)> {
        for (idx, field) in self.fields.iter().enumerate() {
            if field.name.id == id {
                return Some((StructFieldID::new(idx), field));
            }
        }
        None
    }
}
