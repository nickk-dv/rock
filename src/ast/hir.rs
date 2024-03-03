use super::ast::{AssignOp, BasicType, BinOp, Mut, UnOp};
use super::intern::*;
use super::span::Span;
use crate::mem::Arena;
use std::collections::HashMap;

pub struct Hir<'hir> {
    arena: Arena<'hir>,
    scopes: Vec<Scope>,
}

#[derive(Copy, Clone)]
pub struct ProcID(u32);
#[derive(Copy, Clone)]
pub struct EnumID(u32);
#[derive(Copy, Clone)]
pub struct UnionID(u32);
#[derive(Copy, Clone)]
pub struct StructID(u32);
#[derive(Copy, Clone)]
pub struct ConstID(u32);
#[derive(Copy, Clone)]
pub struct GlobalID(u32);
#[derive(Copy, Clone)]
pub struct LocalID(u32);
#[derive(Copy, Clone)]
pub struct EnumVariantID(u32);
#[derive(Copy, Clone)]
pub struct UnionMemberID(u32);
#[derive(Copy, Clone)]
pub struct StructFieldID(u32);

pub type Symbol = ();
#[derive(Copy, Clone)]
struct ScopeID(u32);

pub struct Scope {
    parent: Option<ScopeID>,
    symbols: HashMap<InternID, Symbol>,
}

#[derive(Copy, Clone)]
pub struct Ident {
    pub id: InternID,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum Type<'hir> {
    Basic(BasicType),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
    Reference(&'hir Type<'hir>, Mut),
    ArraySlice(&'hir ArraySlice<'hir>),
    ArrayStatic(&'hir ArrayStatic<'hir>),
}

#[derive(Copy, Clone)]
pub struct ArraySlice<'hir> {
    pub mutt: Mut,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct ArrayStatic<'hir> {
    pub size: ConstExpr<'hir>,
    pub ty: Type<'hir>,
}

#[derive(Copy, Clone)]
pub struct Stmt<'hir> {
    pub kind: StmtKind<'hir>,
    pub span: Span,
}

#[derive(Copy, Clone)]
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
    ForLoop { var_decl: &'hir VarDecl<'hir>, cond: &'hir Expr<'hir>, var_assign: &'hir VarAssign<'hir> },
}

#[derive(Copy, Clone)]
pub struct VarDecl<'hir> {
    pub mutt: Mut,
    pub name: Ident,
    pub ty: Option<Type<'hir>>,
    pub expr: Option<&'hir Expr<'hir>>,
}

#[derive(Copy, Clone)]
pub struct VarAssign<'hir> {
    pub op: AssignOp,
    pub lhs: &'hir Expr<'hir>,
    pub rhs: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub struct Expr<'hir> {
    pub kind: ExprKind<'hir>,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct ConstExpr<'hir>(pub &'hir Expr<'hir>);

#[derive(Copy, Clone)]
#[rustfmt::skip]
pub enum ExprKind<'hir> {
    Error,
    Unit,
    LitNull,
    LitBool     { val: bool },
    LitInt      { val: u64, ty: BasicType },
    LitFloat    { val: f64, ty: BasicType },
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
    ConstVar    { const_id: ConstID },
    GlobalVar   { global_id: GlobalID },
    EnumVariant { enum_id: EnumID, id: EnumVariantID },
    ProcCall    { proc_id: ProcID, input: &'hir [&'hir Expr<'hir>] },
    UnionInit   { union_id: UnionID, input: UnionMemberInit<'hir> },
    StructInit  { struct_id: StructID, input: &'hir [StructFieldInit<'hir>] },
    ArrayInit   { input: &'hir [&'hir Expr<'hir>] },
    ArrayRepeat { expr: &'hir Expr<'hir>, size: ConstExpr<'hir> },
    UnaryExpr   { op: UnOp, rhs: &'hir Expr<'hir> },
    BinaryExpr  { op: BinOp, lhs: &'hir Expr<'hir>, rhs: &'hir Expr<'hir> },
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
    pub pat: &'hir Expr<'hir>,
    pub expr: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub struct UnionMemberInit<'hir> {
    pub id: UnionMemberID,
    pub expr: &'hir Expr<'hir>,
}

#[derive(Copy, Clone)]
pub struct StructFieldInit<'hir> {
    pub id: StructFieldID,
    pub expr: &'hir Expr<'hir>,
}
