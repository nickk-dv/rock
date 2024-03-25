use crate::arena::Arena;
use crate::ast;
use crate::intern::InternID;
use crate::vfs;

pub struct Hir<'hir> {
    pub arena: Arena<'hir>,
    pub scopes: Vec<ScopeData>,
    pub procs: Vec<ProcData<'hir>>,
    pub enums: Vec<EnumData<'hir>>,
    pub unions: Vec<UnionData<'hir>>,
    pub structs: Vec<StructData<'hir>>,
    pub consts: Vec<ConstData<'hir>>,
    pub globals: Vec<GlobalData<'hir>>,
    pub const_exprs: Vec<ConstExprData<'hir>>,
}

macro_rules! hir_id_impl {
    ($name:ident) => {
        #[derive(Copy, Clone, PartialEq)]
        pub struct $name(u32);
        impl $name {
            pub const fn new(index: usize) -> $name {
                $name(index as u32)
            }
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

hir_id_impl!(ScopeID);
pub struct ScopeData {
    pub file_id: vfs::FileID,
}

hir_id_impl!(ProcID);
pub struct ProcData<'hir> {
    pub origin_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub params: &'hir [ProcParam<'hir>],
    pub is_variadic: bool,
    pub return_ty: Type<'hir>,
    pub block: Option<&'hir Expr<'hir>>,
    pub body: ProcBody<'hir>,
}

hir_id_impl!(ProcParamID);
#[derive(Copy, Clone)]
pub struct ProcParam<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
}

hir_id_impl!(LocalID);
#[derive(Copy, Clone)]
pub struct ProcBody<'hir> {
    pub locals: &'hir [&'hir Local<'hir>],
}

hir_id_impl!(EnumID);
pub struct EnumData<'hir> {
    pub origin_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub variants: &'hir [EnumVariant],
}

hir_id_impl!(EnumVariantID);
#[derive(Copy, Clone)]
pub struct EnumVariant {
    pub name: ast::Ident,
    pub value: Option<ConstExprID>,
}

hir_id_impl!(UnionID);
pub struct UnionData<'hir> {
    pub origin_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub members: &'hir [UnionMember<'hir>],
}

hir_id_impl!(UnionMemberID);
#[derive(Copy, Clone)]
pub struct UnionMember<'hir> {
    pub name: ast::Ident,
    pub ty: Type<'hir>,
}

hir_id_impl!(StructID);
pub struct StructData<'hir> {
    pub origin_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub fields: &'hir [StructField<'hir>],
}

hir_id_impl!(StructFieldID);
#[derive(Copy, Clone)]
pub struct StructField<'hir> {
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
}

hir_id_impl!(ConstID);
pub struct ConstData<'hir> {
    pub origin_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
    pub value: ConstExprID,
}

hir_id_impl!(GlobalID);
pub struct GlobalData<'hir> {
    pub origin_id: ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
    pub value: ConstExprID,
}

hir_id_impl!(ConstExprID);
pub struct ConstExprData<'hir> {
    pub origin_id: ScopeID,
    pub value: Option<Expr<'hir>>,
}

#[derive(Copy, Clone)]
pub enum Type<'hir> {
    Error,
    Basic(ast::BasicType),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
    Reference(&'hir Type<'hir>, ast::Mut),
    ArraySlice(&'hir ArraySlice<'hir>),
    ArrayStatic(&'hir ArrayStatic<'hir>),
    ArrayStaticDecl(&'hir ArrayStaticDecl<'hir>), //@differs in how const expr size are resolved, remove
}

#[derive(Copy, Clone)]
pub struct ArraySlice<'hir> {
    pub mutt: ast::Mut,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct ArrayStatic<'hir> {
    pub size: &'hir Expr<'hir>,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct ArrayStaticDecl<'hir> {
    pub size: ConstExprID,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub enum Stmt<'hir> {
    Break,
    Continue,
    Return,
    ReturnVal(&'hir Expr<'hir>),
    Defer(&'hir Expr<'hir>),
    ForLoop(&'hir For<'hir>),
    Local(&'hir Local<'hir>),
    Assign(&'hir Assign<'hir>),
    ExprSemi(&'hir Expr<'hir>),
    ExprTail(&'hir Expr<'hir>),
}

#[derive(Copy, Clone)]
pub struct For<'hir> {
    pub kind: ForKind<'hir>,
    pub block: &'hir Expr<'hir>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum ForKind<'hir> {
    Loop,
    While { cond: &'hir Expr<'hir> },
    ForLoop { local: &'hir Local<'hir>, cond: &'hir Expr<'hir>, assign: &'hir Assign<'hir> },
}

#[derive(Copy, Clone)]
pub struct Local<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
    pub expr: Option<&'hir Expr<'hir>>,
}

#[derive(Copy, Clone)]
pub struct Assign<'hir> {
    pub op: ast::AssignOp,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
}

#[rustfmt::skip]
#[derive(Copy, Clone)]
pub enum Expr<'hir> {
    Error,
    Unit,
    LitNull,
    LitBool     { val: bool },
    LitInt      { val: u64, ty: ast::BasicType },
    LitFloat    { val: f64, ty: ast::BasicType },
    LitChar     { val: char },
    LitString   { id: InternID },
    If          { if_: &'hir If<'hir> },
    Block       { stmts: &'hir [Stmt<'hir>] },
    Match       { match_: &'hir Match<'hir> },
    UnionMember { target: &'hir Expr<'hir>, id: UnionMemberID },
    StructField { target: &'hir Expr<'hir>, id: StructFieldID },
    Index       { target: &'hir Expr<'hir>, index: &'hir Expr<'hir> },
    Cast        { target: &'hir Expr<'hir>, ty: &'hir Type<'hir> },
    LocalVar    { local_id: LocalID },
    ParamVar    { param_id: ProcParamID },
    ConstVar    { const_id: ConstID },
    GlobalVar   { global_id: GlobalID },
    EnumVariant { enum_id: EnumID, id: EnumVariantID },
    ProcCall    { proc_id: ProcID, input: &'hir [&'hir Expr<'hir>] },
    UnionInit   { union_id: UnionID, input: UnionMemberInit<'hir> },
    StructInit  { struct_id: StructID, input: &'hir [StructFieldInit<'hir>] },
    ArrayInit   { input: &'hir [&'hir Expr<'hir>] },
    ArrayRepeat { expr: &'hir Expr<'hir>, size: &'hir Expr<'hir> },
    UnaryExpr   { op: ast::UnOp, rhs: &'hir Expr<'hir> },
    BinaryExpr  { op: ast::BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir> },
}

#[derive(Copy, Clone)]
pub struct If<'hir> {
    pub cond: &'hir Expr<'hir>,
    pub block: &'hir Expr<'hir>,
    pub else_: Option<Else<'hir>>,
}

#[derive(Copy, Clone)]
pub enum Else<'hir> {
    If { else_if: &'hir If<'hir> },
    Block { block: &'hir Expr<'hir> },
}

#[derive(Copy, Clone)]
pub struct Match<'hir> {
    pub on_expr: &'hir Expr<'hir>,
    pub arms: &'hir [MatchArm<'hir>],
}

#[derive(Copy, Clone)]
pub struct MatchArm<'hir> {
    pub pat: &'hir Expr<'hir>, //@optional fallback pattern not represented
    pub expr: &'hir Expr<'hir>,
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

impl<'hir> Hir<'hir> {
    pub fn proc_data(&self, id: ProcID) -> &ProcData {
        &self.procs[id.index()]
    }
    pub fn enum_data(&self, id: EnumID) -> &EnumData {
        &self.enums[id.index()]
    }
    pub fn union_data(&self, id: UnionID) -> &UnionData {
        &self.unions[id.index()]
    }
    pub fn struct_data(&self, id: StructID) -> &StructData {
        &self.structs[id.index()]
    }
    pub fn const_data(&self, id: ConstID) -> &ConstData {
        &self.consts[id.index()]
    }
    pub fn global_data(&self, id: GlobalID) -> &GlobalData {
        &self.globals[id.index()]
    }
    pub fn const_expr_data(&self, id: ConstExprID) -> &ConstExprData {
        &self.const_exprs[id.index()]
    }
}
