use crate::ast::ast::{AssignOp, BasicType, BinaryOp, UnaryOp};
use crate::mem::{Array, InternID, P};

pub type ProcID = u32;
pub type StructID = u32;
pub type ConstValueID = u32;

pub struct Hir {
    procs: Vec<ProcDecl>,
    structs: Vec<StructDecl>,
    const_values: Vec<ConstValue>,
}

#[derive(Copy, Clone)]
pub struct Ident {
    pub id: InternID,
}

#[derive(Copy, Clone)]
pub struct Type {
    pub pointer_level: u32,
    pub kind: TypeKind,
}

#[derive(Copy, Clone)]
pub enum TypeKind {
    Basic(BasicType),
    Struct(StructID),
    ArraySlice(P<ArraySliceType>),
    ArrayStatic(P<ArrayStaticType>),
}

#[derive(Copy, Clone)]
pub struct ArraySliceType {
    pub element: Type,
}

#[derive(Copy, Clone)]
pub struct ArrayStaticType {
    pub size: ConstValueID,
    pub element: Type,
}

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub name: Ident,
    pub params: Array<Type>,
    pub is_variadic: bool,
    pub return_type: Option<Type>,
    pub block: Option<P<Block>>,
}

#[derive(Copy, Clone)]
pub struct StructDecl {
    pub name: Ident,
    pub fields: Array<Type>,
    pub default: ConstValueID,
}

#[derive(Copy, Clone)]
pub enum Stmt {
    If(P<If>),
    For(P<For>),
    Block(P<Block>),
    Defer(P<Block>),
    Break,
    Switch(P<Switch>),
    Return(P<Return>),
    Continue,
    VarDecl(P<VarDecl>),
    VarAssign(P<VarAssign>),
    ProcCall(P<ProcCall>),
}

#[derive(Copy, Clone)]
pub struct If {
    pub condition: Expr,
    pub block: P<Block>,
    pub else_: Option<Else>,
}

#[derive(Copy, Clone)]
pub enum Else {
    If(P<If>),
    Block(P<Block>),
}

#[derive(Copy, Clone)]
pub struct For {
    pub var_decl: Option<P<VarDecl>>,
    pub condition: Option<Expr>,
    pub var_assign: Option<P<VarAssign>>,
    pub block: P<Block>,
}

#[derive(Copy, Clone)]
pub struct Block {
    pub stmts: Array<Stmt>,
}

#[derive(Copy, Clone)]
pub struct Switch {
    pub expr: Expr,
    pub cases: Array<SwitchCase>,
}

#[derive(Copy, Clone)]
pub struct SwitchCase {
    pub expr: Expr,
    pub block: P<Block>,
}

#[derive(Copy, Clone)]
pub struct Return {
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct VarDecl {
    pub name: Ident,
    pub tt: Option<Type>,
    pub expr: Option<Expr>,
}

#[derive(Copy, Clone)]
pub struct VarAssign {
    pub var: P<Var>,
    pub op: AssignOp,
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct Expr {
    //@todo flags
    kind: ExprKind,
}

#[derive(Copy, Clone)]
pub enum ExprKind {
    Var(P<Var>),
    Cast(P<Cast>),
    ProcCall(P<ProcCall>),
    ArrayInit(P<ArrayInit>),
    StructInit(P<StructInit>),
    ConstValue(ConstValueID),
    UnaryExpr(P<UnaryExpr>),
    BinaryExpr(P<BinaryExpr>),
}

#[derive(Copy, Clone)]
pub struct Var {
    pub name: Ident,
    pub access: Option<P<Access>>,
}

#[derive(Copy, Clone)]
pub struct Access {
    pub kind: AccessKind,
    pub next: Option<P<Access>>,
}

#[derive(Copy, Clone)]
pub enum AccessKind {
    Field(u32),
    Array(Expr),
}

#[derive(Copy, Clone)]
pub struct Cast {
    pub tt: Type,
    pub expr: Expr,
}

#[derive(Copy, Clone)]
pub struct ProcCall {
    pub proc: ProcID,
    pub input: Array<Expr>,
    pub access: Option<P<Access>>,
}

#[derive(Copy, Clone)]
pub struct ArrayInit {
    pub tt: Type,
    pub input: Array<Expr>,
}

#[derive(Copy, Clone)]
pub struct StructInit {
    pub tt: StructID,
    pub input: Array<Expr>,
}

#[derive(Copy, Clone)]
pub enum ConstValue {
    Bool(bool),
    Int(i64), // @is signed int needed?
    Uint(u64),
    Float(f64),
    Char(char),
    NullPtr,
    Array(P<ConstArray>),
    Struct(P<ConstStruct>),
}

#[derive(Copy, Clone)]
pub struct ConstArray {
    pub tt: Type,
    pub values: Array<ConstValueID>,
}

#[derive(Copy, Clone)]
pub struct ConstStruct {
    pub tt: Type,
    pub values: Array<ConstValueID>,
}

#[derive(Copy, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub rhs: Expr,
}

#[derive(Copy, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: Expr,
    pub rhs: Expr,
}
