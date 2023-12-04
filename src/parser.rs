use crate::ast::*;
use crate::lexer::*;
use crate::token::*;
use std::io;
use std::path::PathBuf;

pub fn parse() -> io::Result<()> {
    let mut path = PathBuf::new();
    path.push("src");
    path.push("main.txt");
    println!("path: {}\n", path.display());

    let string = std::fs::read_to_string(&path)?;
    println!("File content:\n{}", string);

    let mut lexer = Lexer::new(&string);
    let tokens = lexer.lex();
    for tok in tokens.iter() {
        dbg!(tok.kind); //@temp
    }

    Ok(())
}

impl TokenKind {
    fn as_unary_op(&self) -> Option<UnaryOp> {
        match self {
            TokenKind::Minus => Some(UnaryOp::Minus),
            TokenKind::BitNot => Some(UnaryOp::BitNot),
            TokenKind::LogicNot => Some(UnaryOp::LogicNot),
            TokenKind::Times => Some(UnaryOp::AddressOf),
            TokenKind::Shl => Some(UnaryOp::Dereference),
            _ => None,
        }
    }

    fn as_binary_op(&self) -> Option<BinaryOp> {
        match self {
            TokenKind::LogicAnd => Some(BinaryOp::LogicAnd),
            TokenKind::LogicOr => Some(BinaryOp::LogicOr),
            TokenKind::Less => Some(BinaryOp::Less),
            TokenKind::Greater => Some(BinaryOp::Greater),
            TokenKind::LessEq => Some(BinaryOp::LessEq),
            TokenKind::GreaterEq => Some(BinaryOp::GreaterEq),
            TokenKind::IsEq => Some(BinaryOp::IsEq),
            TokenKind::NotEq => Some(BinaryOp::NotEq),
            TokenKind::Plus => Some(BinaryOp::Plus),
            TokenKind::Minus => Some(BinaryOp::Minus),
            TokenKind::Times => Some(BinaryOp::Times),
            TokenKind::Div => Some(BinaryOp::Div),
            TokenKind::Mod => Some(BinaryOp::Mod),
            TokenKind::BitAnd => Some(BinaryOp::BitAnd),
            TokenKind::BitOr => Some(BinaryOp::BitOr),
            TokenKind::BitXor => Some(BinaryOp::BitXor),
            TokenKind::Shl => Some(BinaryOp::Shl),
            TokenKind::Shr => Some(BinaryOp::Shr),
            _ => None,
        }
    }

    fn as_assign_op(&self) -> Option<AssignOp> {
        match self {
            TokenKind::Assign => Some(AssignOp::Assign),
            TokenKind::PlusEq => Some(AssignOp::Plus),
            TokenKind::MinusEq => Some(AssignOp::Minus),
            TokenKind::TimesEq => Some(AssignOp::Times),
            TokenKind::DivEq => Some(AssignOp::Div),
            TokenKind::ModEq => Some(AssignOp::Mod),
            TokenKind::BitAndEq => Some(AssignOp::BitAnd),
            TokenKind::BitOrEq => Some(AssignOp::BitOr),
            TokenKind::BitXorEq => Some(AssignOp::BitXor),
            TokenKind::ShlEq => Some(AssignOp::Shl),
            TokenKind::ShrEq => Some(AssignOp::Shr),
            _ => None,
        }
    }

    fn as_basic_type(&self) -> Option<BasicType> {
        match self {
            TokenKind::KwS8 => Some(BasicType::S8),
            TokenKind::KwS16 => Some(BasicType::S16),
            TokenKind::KwS32 => Some(BasicType::S32),
            TokenKind::KwS64 => Some(BasicType::S64),
            TokenKind::KwU8 => Some(BasicType::U8),
            TokenKind::KwU16 => Some(BasicType::U16),
            TokenKind::KwU32 => Some(BasicType::U32),
            TokenKind::KwU64 => Some(BasicType::U64),
            TokenKind::KwF32 => Some(BasicType::F32),
            TokenKind::KwF64 => Some(BasicType::F64),
            TokenKind::KwBool => Some(BasicType::Bool),
            TokenKind::KwString => Some(BasicType::String),
            _ => None,
        }
    }
}
