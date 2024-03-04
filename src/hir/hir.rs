use super::hir_temp;
use crate::ast::ast;
use crate::ast::intern;
use crate::ast::span::Span;
use crate::mem::Arena;

pub struct Hir<'hir> {
    arena: Arena<'hir>,
    procs: Vec<ProcData<'hir>>,
    enums: Vec<EnumData<'hir>>,
    unions: Vec<UnionData<'hir>>,
    structs: Vec<StructData<'hir>>,
    consts: Vec<ConstData<'hir>>,
    globals: Vec<GlobalData<'hir>>,
    const_exprs: Vec<ConstExpr<'hir>>,
}

// @local storage bodies arent defined yet
// LocalID doesnt have a usage yet
#[derive(Copy, Clone)]
pub struct LocalID(u32);

#[derive(Copy, Clone)]
pub struct ProcID(u32);
pub struct ProcData<'hir> {
    pub from_id: hir_temp::ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub params: &'hir [ProcParam<'hir>],
    pub is_variadic: bool,
    pub return_ty: Type<'hir>,
    pub block: Option<&'hir Expr<'hir>>,
}

#[derive(Copy, Clone)]
pub struct ProcParamID(u32);
pub struct ProcParam<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct EnumID(u32);
pub struct EnumData<'hir> {
    pub from_id: hir_temp::ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub variants: &'hir [EnumVariant],
}

#[derive(Copy, Clone)]
pub struct EnumVariantID(u32);
pub struct EnumVariant {
    pub name: ast::Ident,
    pub value: Option<ConstExprID>, // @we can assign specific numeric value without a span to it
}

#[derive(Copy, Clone)]
pub struct UnionID(u32);
pub struct UnionData<'hir> {
    pub from_id: hir_temp::ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub members: &'hir [UnionMember<'hir>],
}

#[derive(Copy, Clone)]
pub struct UnionMemberID(u32);
pub struct UnionMember<'hir> {
    pub name: ast::Ident,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct StructID(u32);
pub struct StructData<'hir> {
    pub from_id: hir_temp::ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub fields: &'hir [StructField<'hir>],
}

#[derive(Copy, Clone)]
pub struct StructFieldID(u32);
pub struct StructField<'hir> {
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct ConstID(u32);
pub struct ConstData<'hir> {
    pub from_id: hir_temp::ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub ty: Option<Type<'hir>>, // @how to handle type is it required?
    pub value: ConstExprID,
}

#[derive(Copy, Clone)]
pub struct GlobalID(u32);
pub struct GlobalData<'hir> {
    pub from_id: hir_temp::ScopeID,
    pub vis: ast::Vis,
    pub name: ast::Ident,
    pub ty: Option<Type<'hir>>, // @how to handle type is it required?
    pub value: ConstExprID,
}

#[derive(Copy, Clone)]
pub struct ConstExprID(u32);
pub struct ConstExpr<'hir> {
    pub from_id: hir_temp::ScopeID,
    pub value: Option<&'hir Expr<'hir>>,
}

// @should static arrays with ast::ConstExpr that are used in declarations
// be represented diffrently? since they are part of constant dependency
// resolution process, and might get a 'Erorr' value for their size
// in general Type[?] is usefull concept to represent for better typecheking flow
// since we dont need to mark entire type as Error
pub enum Type<'hir> {
    Error,
    Basic(ast::BasicType),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
    Reference(&'hir Type<'hir>, ast::Mut),
    ArraySlice(&'hir ArraySlice<'hir>),
    ArrayStatic(&'hir ArrayStatic<'hir>),
}

pub struct ArraySlice<'hir> {
    pub mutt: ast::Mut,
    pub ty: Type<'hir>,
}

pub struct ArrayStatic<'hir> {
    pub size: &'hir Expr<'hir>,
    pub ty: Type<'hir>,
}

pub struct Stmt<'hir> {
    pub kind: StmtKind<'hir>,
    pub span: Span,
}

pub enum StmtKind<'hir> {
    Break,
    Continue,
    Return,
    ReturnVal(&'hir Expr<'hir>),
    Defer(&'hir Expr<'hir>),
    ForLoop(&'hir For<'hir>),
    VarDecl(&'hir VarDecl<'hir>),
    VarAssign(&'hir VarAssign<'hir>),
    ExprSemi(&'hir Expr<'hir>),
    ExprTail(&'hir Expr<'hir>),
}

pub struct For<'hir> {
    pub kind: ForKind<'hir>,
    pub block: &'hir Expr<'hir>,
}

#[rustfmt::skip]
pub enum ForKind<'hir> {
    Loop,
    While { cond: &'hir Expr<'hir> },
    ForLoop { var_decl: &'hir VarDecl<'hir>, cond: &'hir Expr<'hir>, var_assign: &'hir VarAssign<'hir> },
}

pub struct VarDecl<'hir> {
    pub mutt: ast::Mut,
    pub name: ast::Ident,
    pub ty: Type<'hir>,
    pub expr: Option<&'hir Expr<'hir>>,
}

pub struct VarAssign<'hir> {
    pub op: ast::AssignOp,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
}

pub struct Expr<'hir> {
    pub kind: ExprKind<'hir>,
    pub span: Span,
}

#[rustfmt::skip]
pub enum ExprKind<'hir> {
    Error,
    Unit,
    LitNull,
    LitBool     { val: bool },
    LitInt      { val: u64, ty: ast::BasicType },
    LitFloat    { val: f64, ty: ast::BasicType },
    LitChar     { val: char },
    LitString   { id: intern::InternID },
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

pub struct If<'hir> {
    pub cond: &'hir Expr<'hir>,
    pub block: &'hir Expr<'hir>,
    pub else_: Option<Else<'hir>>,
}

pub enum Else<'hir> {
    If { else_if: &'hir If<'hir> },
    Block { block: &'hir Expr<'hir> },
}

pub struct Match<'hir> {
    pub on_expr: &'hir Expr<'hir>,
    pub arms: &'hir [MatchArm<'hir>],
}

pub struct MatchArm<'hir> {
    pub pat: &'hir Expr<'hir>,
    pub expr: &'hir Expr<'hir>,
}

pub struct UnionMemberInit<'hir> {
    pub member_id: UnionMemberID,
    pub expr: &'hir Expr<'hir>,
}

pub struct StructFieldInit<'hir> {
    pub field_id: StructFieldID,
    pub expr: &'hir Expr<'hir>,
}

impl<'ast> Hir<'ast> {
    pub fn new() -> Self {
        Self {
            arena: Arena::new(),
            procs: Vec::new(),
            enums: Vec::new(),
            unions: Vec::new(),
            structs: Vec::new(),
            consts: Vec::new(),
            globals: Vec::new(),
            const_exprs: Vec::new(),
        }
    }
}