use super::scope::{ProcID, StructID};
use crate::ast::{ast::BasicType, span::Span};

pub type InstID = u32;

pub enum Inst {
    BB(u32),
    Label(InstID),
    Br,
    CondBr,
    Ret,
    RetVal,
    Value(u32), //@no type information
    Call { argc: u32, id: Option<ProcID> },
    ArrayInit { argc: u32 }, //@no type information
    StructInit { argc: u32, id: Option<StructID> },
    DbgSpan(Span),

    Null,
    Bool(bool),
    UInt(u64, Option<BasicType>),
    Float(f64, Option<BasicType>),
    Char(char),

    Neg,
    BitNot,
    LogicNot,
    Addr,
    Deref,

    LogicAnd,
    LogicOr,
    CmpLT,
    CmpGT,
    CmpLEQ,
    CmpGEQ,
    CmpEQ,
    CmpNEQ,
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
}

pub fn print_inst(inst: &Inst) {
    match inst {
        Inst::BB(v) => {
            print!("@block.{}", v);
        }
        Inst::Label(..) => {}
        Inst::Br => {
            print!("br ");
        }
        Inst::CondBr => {
            print!("if ");
        }
        Inst::Ret => {
            print!("ret");
        }
        Inst::RetVal => {
            print!("ret_val");
        }
        Inst::Value(v) => {
            print!("%{}", v);
        }
        Inst::Call { argc, id } => {
            print!("call.{} ", argc);
            if let Some(id) = id {
                print!(" <{}> ", id);
            } else {
                print!(" <no id> ");
            }
        }
        Inst::ArrayInit { argc } => {
            print!("array_init.{} ", argc);
        }
        Inst::StructInit { argc, id } => {
            print!("struct_init.{}", argc);
            if let Some(id) = id {
                print!(" <{}> ", id);
            } else {
                print!(" <infer> ");
            }
        }
        Inst::DbgSpan(_) => print!("<dbg span>"),
        Inst::Null => print!("null"),
        Inst::Bool(v) => print!("{}", v),
        Inst::UInt(v, _) => print!("{}", v),
        Inst::Float(v, _) => print!("{}", v),
        Inst::Char(v) => print!("'{}'", v),
        Inst::Neg => print!("neg"),
        Inst::BitNot => print!("bitnot"),
        Inst::LogicNot => print!("logicnot"),
        Inst::Addr => print!("addr"),
        Inst::Deref => print!("deref"),
        Inst::LogicAnd => print!("logic-and"),
        Inst::LogicOr => print!("logic-or"),
        Inst::CmpLT => print!("cmplt"),
        Inst::CmpGT => print!("cmpgt"),
        Inst::CmpLEQ => print!("cmpleq"),
        Inst::CmpGEQ => print!("cmpgeq"),
        Inst::CmpEQ => print!("cmpeq"),
        Inst::CmpNEQ => print!("cmpneq"),
        Inst::Add => print!("add"),
        Inst::Sub => print!("sub"),
        Inst::Mul => print!("mul"),
        Inst::Div => print!("div"),
        Inst::Rem => print!("rem"),
        Inst::BitAnd => print!("bit-and"),
        Inst::BitOr => print!("bit-or"),
        Inst::BitXor => print!("bit-xor"),
        Inst::Shl => print!("shl"),
        Inst::Shr => print!("shr"),
    }
}

pub fn pretty_print(ir_buf: &Vec<Inst>) {
    let mut id = 0;
    let len = ir_buf.len();
    while id < len {
        let inst = unsafe { ir_buf.get_unchecked(id) };
        id += 1;
        print_inst(inst);

        match inst {
            Inst::BB(..) | Inst::Ret => {
                println!();
                continue;
            }
            Inst::RetVal => {
                print!(" ");
            }
            Inst::Br => {
                print_label(&ir_buf, id, 0);
                id += 1;
                println!();
                continue;
            }
            Inst::CondBr => {
                let inst = unsafe { ir_buf.get_unchecked(id) };
                id += 1;
                print_inst(inst);
                print!(" ");
                print_label(&ir_buf, id, 0);
                print!(" else ");
                print_label(&ir_buf, id, 1);
                id += 2;
                println!();
                continue;
            }
            Inst::Value(..) => {
                print!(" = ");
            }
            Inst::DbgSpan(..) => {
                continue;
            }
            _ => panic!("invalid starting ir inst"),
        }

        let inst = unsafe { ir_buf.get_unchecked(id) };
        id += 1;
        print_inst(inst);
        match inst {
            Inst::Value(..)
            | Inst::Null
            | Inst::Bool(..)
            | Inst::UInt(.., _)
            | Inst::Float(.., _)
            | Inst::Char(..) => {
                println!();
                continue;
            }
            Inst::Call { argc, .. } => {
                for _ in 0..*argc {
                    let inst = unsafe { ir_buf.get_unchecked(id) };
                    id += 1;
                    print_inst(inst);
                    print!(" ");
                }
                println!();
                continue;
            }
            Inst::ArrayInit { argc } => {
                for _ in 0..*argc {
                    let inst = unsafe { ir_buf.get_unchecked(id) };
                    id += 1;
                    print_inst(inst);
                    print!(" ");
                }
                println!();
                continue;
            }
            Inst::StructInit { argc, .. } => {
                for _ in 0..*argc {
                    let inst = unsafe { ir_buf.get_unchecked(id) };
                    id += 1;
                    print_inst(inst);
                    print!(" ");
                }
                println!();
                continue;
            }
            Inst::Neg | Inst::BitNot | Inst::LogicNot | Inst::Addr | Inst::Deref => {
                print!(" ");
                let inst = unsafe { ir_buf.get_unchecked(id) };
                id += 1;
                print_inst(inst);
                println!();
                continue;
            }
            Inst::LogicAnd
            | Inst::LogicOr
            | Inst::CmpLT
            | Inst::CmpGT
            | Inst::CmpLEQ
            | Inst::CmpGEQ
            | Inst::CmpEQ
            | Inst::CmpNEQ
            | Inst::Add
            | Inst::Sub
            | Inst::Mul
            | Inst::Div
            | Inst::Rem
            | Inst::BitAnd
            | Inst::BitOr
            | Inst::BitXor
            | Inst::Shl
            | Inst::Shr => {
                print!(" ");
                let inst = unsafe { ir_buf.get_unchecked(id) };
                id += 1;
                print_inst(inst);
                print!(" ");
                let inst = unsafe { ir_buf.get_unchecked(id) };
                id += 1;
                print_inst(inst);
                println!();
                continue;
            }
            _ => panic!("invalid second ir inst"),
        }
    }
}

use crate::err::ansi;

fn space() {
    print!(" ");
}

fn print_label(ir_buf: &Vec<Inst>, i: usize, offset: usize) {
    if let Inst::Label(bb_index) = unsafe { ir_buf.get_unchecked(i + offset) } {
        let inst = unsafe { ir_buf.get_unchecked(*bb_index as usize) };
        print!("goto ");
        print_inst(inst);
    }
}

fn print_value(ir_buf: &Vec<Inst>, i: usize, offset: usize) {
    if let Inst::Value(v) = unsafe { ir_buf.get_unchecked(i + offset) } {
        print!("%{}", v);
    }
}
