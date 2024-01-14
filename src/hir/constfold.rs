use super::hir::ConstValue;
use crate::{ast::ast::*, mem::P};

fn is_consteval(expr: Expr) -> bool {
    match expr {
        Expr::Var(..) => false,
        Expr::Enum(..) => false,
        Expr::Cast(..) => false,
        Expr::Sizeof(..) => false,
        Expr::Literal(..) => true,
        Expr::ProcCall(..) => false,
        Expr::ArrayInit(..) => false,
        Expr::StructInit(..) => false,
        Expr::UnaryExpr(un) => is_consteval(un.rhs),
        Expr::BinaryExpr(bin) => is_consteval(bin.lhs) && is_consteval(bin.rhs),
    }
}

pub fn consteval(expr: Expr) -> Option<ConstValue> {
    if !is_consteval(expr) {
        return None;
    }

    match expr {
        Expr::Literal(lit) => match *lit {
            Literal::Null => Some(ConstValue::NullPtr),
            Literal::Bool(v) => Some(ConstValue::Bool(v)),
            Literal::Uint(v, ..) => Some(ConstValue::Uint(v)),
            Literal::Float(v, ..) => Some(ConstValue::Float(v)),
            Literal::Char(v) => Some(ConstValue::Char(v)),
            Literal::String => None,
        },
        Expr::UnaryExpr(un) => consteval_unary(un),
        Expr::BinaryExpr(bin) => consteval_binary(bin),
        _ => None,
    }
}

fn consteval_unary(un: P<UnaryExpr>) -> Option<ConstValue> {
    let rhs_result = consteval(un.rhs);

    let rhs = match rhs_result {
        Some(v) => v,
        None => return None,
    };

    match un.op {
        UnaryOp::Minus => match rhs {
            ConstValue::Bool(..) => todo!("cannot apply unary `-` on bool"),
            ConstValue::Int(int) => {
                if int == i64::MIN {
                    println!("overflow in unary `-` on {}", int);
                    None
                } else {
                    Some(ConstValue::Int(-int))
                }
            }
            ConstValue::Uint(uint) => {
                if uint > (i64::MAX as u64 + 1) {
                    println!("overflow in unary `-` on {}", uint);
                    None
                } else {
                    if uint == (i64::MAX as u64 + 1) {
                        Some(ConstValue::Int(i64::MIN))
                    } else {
                        Some(ConstValue::Int(-(uint as i64)))
                    }
                }
            }
            ConstValue::Float(v) => Some(ConstValue::Float(-v)),
            ConstValue::Char(..) => todo!("cannot apply unary `-` on char"),
            ConstValue::NullPtr => todo!("cannot apply unary `-` on null pointer"),
            ConstValue::Array(..) => todo!("cannot apply unary `-` on array"),
            ConstValue::Struct(..) => todo!("cannot apply unary `-` on struct"),
        },
        UnaryOp::BitNot => match rhs {
            ConstValue::Bool(..) => todo!("cannot apply unary `~` on bool"),
            ConstValue::Int(int) => Some(ConstValue::Int(!int)),
            ConstValue::Uint(uint) => Some(ConstValue::Uint(!uint)),
            ConstValue::Float(..) => todo!("cannot apply unary `~` on float"),
            ConstValue::Char(..) => todo!("cannot apply unary `~` on char"),
            ConstValue::NullPtr => todo!("cannot apply unary `~` on null pointer"),
            ConstValue::Array(..) => todo!("cannot apply unary `~` on array"),
            ConstValue::Struct(..) => todo!("cannot apply unary `~` on struct"),
        },
        UnaryOp::LogicNot => match rhs {
            ConstValue::Bool(v) => Some(ConstValue::Bool(!v)),
            ConstValue::Int(..) => todo!("cannot apply unary `!` on integer"),
            ConstValue::Uint(..) => todo!("cannot apply unary `!` on integer"),
            ConstValue::Float(..) => todo!("cannot apply unary `~` on float"),
            ConstValue::Char(..) => todo!("cannot apply unary `~` on char"),
            ConstValue::NullPtr => todo!("cannot apply unary `~` on null pointer"),
            ConstValue::Array(..) => todo!("cannot apply unary `~` on array"),
            ConstValue::Struct(..) => todo!("cannot apply unary `~` on struct"),
        },
        UnaryOp::AddressOf => todo!("unary `*` is not consteval supported"),
        UnaryOp::Dereference => todo!("unary `<<` is not consteval supported"),
    }
}

fn consteval_binary(bin: P<BinaryExpr>) -> Option<ConstValue> {
    let lhs_result = consteval(bin.lhs);
    let rhs_result = consteval(bin.rhs);

    let lhs = match lhs_result {
        Some(v) => v,
        None => return None,
    };
    let rhs = match rhs_result {
        Some(v) => v,
        None => return None,
    };

    match bin.op {
        BinaryOp::LogicAnd => todo!(),
        BinaryOp::LogicOr => todo!(),
        BinaryOp::Less => todo!(),
        BinaryOp::Greater => todo!(),
        BinaryOp::LessEq => todo!(),
        BinaryOp::GreaterEq => todo!(),
        BinaryOp::IsEq => todo!(),
        BinaryOp::NotEq => todo!(),
        BinaryOp::Plus => todo!(),
        BinaryOp::Minus => todo!(),
        BinaryOp::Times => todo!(),
        BinaryOp::Div => todo!(),
        BinaryOp::Mod => todo!(),
        BinaryOp::BitAnd => todo!(),
        BinaryOp::BitOr => todo!(),
        BinaryOp::BitXor => todo!(),
        BinaryOp::Shl => todo!(),
        BinaryOp::Shr => todo!(),
    }

    None
}
