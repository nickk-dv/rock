use crate::ast::ast::BasicType;
use crate::mem::{Array, InternID};

pub type ProcID = u32;
pub type StructID = u32;
pub type ArrayTypeID = u32; //@might be changed to global type ids
pub type IfID = u32;
pub type ForID = u32;
pub type BlockID = u32;
pub type SwitchID = u32;
pub type ReturnID = u32;
pub type VarDeclID = u32;
pub type VarAssignID = u32;
pub type ProcCallID = u32;

pub struct Hir {
    procs: Vec<ProcDecl>,
    structs: Vec<StructDecl>,
    array_types: Vec<Type>,
    if_stmts: Vec<If>,
    for_stmts: Vec<For>,
    block_stmts: Vec<Block>,
    switch_stmts: Vec<Switch>,
    return_stmts: Vec<Return>,
    var_decl_stmts: Vec<VarDecl>,
    var_assign_stmts: Vec<VarAssign>,
    proc_call_stmts: Vec<ProcCall>,
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
    ArraySlice,
    ArrayStatic(ArrayTypeID),
}

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub name: Ident,
    pub params: Array<ProcParam>,
    pub is_variadic: bool,
    pub return_type: Option<Type>,
    pub block: BlockID,
}

#[derive(Copy, Clone)]
pub struct ProcParam {
    pub name: Ident,
    pub tt: Type,
}

#[derive(Copy, Clone)]
pub struct StructDecl {
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct StructField {
    pub name: Ident,
    pub tt: Type,
    //@default expr, use const value id ?
}

#[derive(Copy, Clone)]
pub enum Stmt {
    If(IfID),
    For(ForID),
    Block(BlockID),
    Defer(BlockID),
    Break,
    Switch(SwitchID),
    Return(ReturnID),
    Continue,
    VarDecl(VarDeclID),
    VarAssign(VarAssignID),
    ProcCall(ProcCallID),
}

#[derive(Copy, Clone)]
pub struct If {
    pub condition: Expr,
    pub block: BlockID,
    pub else_: Option<Else>,
}

#[derive(Copy, Clone)]
pub enum Else {
    If(IfID),
    Block(BlockID),
}

#[derive(Copy, Clone)]
pub struct For {
    pub var_decl: Option<VarDeclID>,
    pub condition: Option<Expr>,
    pub var_assign: Option<VarAssign>,
    pub block: BlockID,
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
    pub block: BlockID,
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
    //@todo var + access
}

#[derive(Copy, Clone)]
pub struct Expr {} //@todo

#[derive(Copy, Clone)]
pub struct ProcCall {
    pub proc: ProcID,
    pub input: Array<Expr>,
    //@todo access
}
