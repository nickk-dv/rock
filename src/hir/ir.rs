// Prototype for untyped IR
// goals:
// resolve names
// resolve module acceses
// resolve all visibility rules
// transform to basic blocks
// deconstruct for loops + break / continue / defer blocks
// allow for complex semantic analysis
// and control flow analysis

/*
@entry:
%0 icmp 2 3
condbr %0 @cond, @else
@cond:
ret 5
@else:
ret 10
*/

use crate::ast::ast::{BasicType, Ident};

pub enum Inst {
    BB(u32),
    Br(Label),
    CondBr(InstID, Label, Label),
    Ret,
    RetVal(InstID),
    GetFieldPtr(InstID, InstID),
    GetArrayPtr(InstID, InstID),
    NameIdent(Ident),
    Null,
    Bool(bool),
    Int(u64, Option<BasicType>),
    Float(f64, Option<BasicType>),
    Char(char),
    Neg(Un),
    BitNot(Un),
    BoolNot(Un),
    Addr(Un),
    Deref(Un),
    BoolAnd(Bin),
    BoolOr(Bin),
    CmpLT(Bin),
    CmpGT(Bin),
    CmpLEQ(Bin),
    CmpGEQ(Bin),
    CmpEQ(Bin),
    CmpNEQ(Bin),
    Add(Bin),
    Sub(Bin),
    Mul(Bin),
    Div(Bin),
    Rem(Bin),
    BitAnd(Bin),
    BitOr(Bin),
    BitXor(Bin),
    Shl(Bin),
    Shr(Bin),
}

pub type InstID = u32;
pub type Label = InstID;

struct Un {
    rhs: InstID,
}

struct Bin {
    lhs: InstID,
    rhs: InstID,
}

pub fn test() {
    let inst_stream = vec![Inst::BB(0), Inst::Int(5, None), Inst::RetVal(1)];
    pretty_print(inst_stream);
}

pub fn pretty_print(inst_stream: Vec<Inst>) {
    for inst in inst_stream.iter() {
        match inst {
            Inst::BB(id) => println!("@{}:", id),
            Inst::Br(bb) => {
                let bb_id = if let Inst::BB(id) = *inst_stream.get(*bb as usize).unwrap() {
                    id
                } else {
                    0
                };
                println!("br @{}", bb_id);
            }
            Inst::CondBr(id, bb_t, bb_f) => {
                let bb_t_id = if let Inst::BB(id) = *inst_stream.get(*bb_t as usize).unwrap() {
                    id
                } else {
                    0
                };
                let bb_f_id = if let Inst::BB(id) = *inst_stream.get(*bb_f as usize).unwrap() {
                    id
                } else {
                    0
                };
                println!("condbr ? @{} @{}", bb_t_id, bb_f_id);
            }
            Inst::RetVal(id) => {
                let val = inst_stream.get(*id as usize).unwrap();
                match val {
                    Inst::Int(v, _) => println!("ret {}", v),
                    _ => println!("ret ?"),
                }
            }
            Inst::Int(..) => {}
            _ => {}
        }
    }
}
