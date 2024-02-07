use super::span::*;
use super::{ast::BasicType, intern::InternID};
use crate::mem::arena_id::{Arena, Id};

/*
possible design:
immutable ast
create scopes      (module tree, maps to decl ids)
checked decl lists (duplicate remove / name resolution, item usage counts)
per function typed ir (const propagation + global resolution?)
llvm ir
*/

#[derive(Copy, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    BitNot,
    LogicNot,
    Addr,
    Deref,
}

#[derive(Copy, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,

    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,

    IsEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,

    LogicAnd,
    LogicOr,
}

#[derive(Copy, Clone)]
pub enum AssignOp {
    Assign,
    Bin(BinOp),
}

#[derive(Clone, Copy)]
pub enum Stmt {}

#[derive(Clone, Copy)]
pub struct MatchArm {
    pat: Id<Expr>,
    expr: Id<Expr>,
}

#[derive(Clone, Copy)]
pub struct Block {
    stmts: List<Stmt>,
}

#[rustfmt::skip]
#[derive(Clone, Copy)]
pub enum Expr {
    Unit,
    Discard,
    LitNull,
    LitBool   { val: bool },
    LitUint   { val: u64, ty: Option<BasicType> },
    LitFloat  { val: f64, ty: Option<BasicType> },
    LitChar   { val: char },
    LitString { id: InternID },
    Block     { block: Id<Block> },
    Match     { expr: Id<Expr>, arms: List<MatchArm> },
    If        { if_: Id<If>, else_: Option<Else> },
    // .name => expr ident
    // cast  => expr as ty
    // item  => path::name
    // call  => ...
    // struct => ...
    ArrayInit   { input: List<Expr> },
    ArrayRepeat { expr: Id<Expr>, size: Id<Expr>, },
    UnExpr      { op: UnOp, rhs: Id<Expr> },
    BinExpr     { op: BinOp, lhs: Id<Expr>, rhs: Id<Expr> },
}

#[derive(Clone, Copy)]
pub struct If {
    cond: Id<Expr>,
    block: Id<Block>,
}

#[derive(Clone, Copy)]
pub enum Else {
    If(Id<If>),
    Block(Id<Block>),
}

#[derive(Clone, Copy)]
pub struct List<T: Copy> {
    first: Id<Node<T>>,
    last: Id<Node<T>>,
}

#[derive(Copy, Clone)]
struct Node<T: Copy> {
    value: T,
    next: Option<Id<T>>,
}
