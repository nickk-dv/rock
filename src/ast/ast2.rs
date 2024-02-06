use super::span::*;
use super::{ast::BasicType, intern::InternID};

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

struct ExprRef(u32);

enum Expr {
    Unit,
    Discard,
    LitNull,
    LitBool {
        v: bool,
    },
    LitUint {
        v: u64,
        ty: Option<BasicType>,
    },
    LitFloat {
        v: f64,
        ty: Option<BasicType>,
    },
    LitChar {
        v: char,
    },
    LitString {
        id: InternID,
    },
    // if    => ...
    // else  => ...
    // block => list<stmt>
    // match => expr list<arm>
    // .name => expr ident
    // cast  => expr as ty
    // item  => path::name
    // call  => ...
    // array => [expr, expr, ...]
    // struct => ...
    ArrayRepeat {
        expr: ExprRef,
        size: ExprRef,
    },
    UnExpr {
        op: UnOp,
        rhs: ExprRef,
    },
    BinExpr {
        op: BinOp,
        lhs: ExprRef,
        rhs: ExprRef,
    },
}
