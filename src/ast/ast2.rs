use super::intern::*;
use super::span::Span;
use super::FileID;
use crate::ast::{AssignOp, BasicType, BinOp, UnOp};
use crate::check::{EnumID, StructID, UnionID};
use crate::mem::*;

pub struct Ast {
    pub arena: Arena,
    pub modules: Vec<Box<Module>>,
}

pub struct Module {
    pub file_id: FileID,
    pub decls: Vec<Decl>,
}

pub enum Decl {
    Use(Box<UseDecl>),
    Mod(Box<ModDecl>),
    Proc(Box<ProcDecl>),
    Enum(Box<EnumDecl>),
    Union(Box<UnionDecl>),
    Struct(Box<StructDecl>),
    Const(Box<ConstDecl>),
    Global(Box<GlobalDecl>),
}

pub struct UseDecl {
    pub path: Box<Path>,
    pub symbols: Vec<UseSymbol>,
}

pub struct UseSymbol {
    pub name: Ident,
    pub alias: Option<Ident>,
}

pub struct ModDecl {
    pub vis: Vis,
    pub name: Ident,
}

pub struct ProcDecl {
    pub vis: Vis,
    pub name: Ident,
    pub params: Vec<ProcParam>,
    pub is_variadic: bool,
    pub return_ty: Option<Type>,
    pub block: Option<Box<Expr>>,
}

pub struct ProcParam {
    pub mutt: Mut,
    pub name: Ident,
    pub ty: Type,
}

pub struct EnumDecl {
    pub vis: Vis,
    pub name: Ident,
    pub variants: Vec<EnumVariant>,
}

pub struct EnumVariant {
    pub name: Ident,
    pub value: Option<ConstExpr>,
}

pub struct UnionDecl {
    pub vis: Vis,
    pub name: Ident,
    pub members: Vec<UnionMember>,
}

pub struct UnionMember {
    pub name: Ident,
    pub ty: Type,
}

pub struct StructDecl {
    pub vis: Vis,
    pub name: Ident,
    pub fields: Vec<StructField>,
}

pub struct StructField {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Type,
}

pub struct ConstDecl {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: ConstExpr,
}

pub struct GlobalDecl {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Option<Type>,
    pub value: ConstExpr,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Vis {
    Public,
    Private,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Mut {
    Mutable,
    Immutable,
}

pub struct Ident {
    pub id: InternID,
    pub span: Span,
}

pub struct Path {
    pub kind: PathKind,
    pub names: Vec<Ident>,
    pub span_start: u32,
}

#[derive(Copy, Clone, PartialEq)]
pub enum PathKind {
    None,
    Super,
    Package,
}

pub struct Type {
    pub ptr: PtrLevel,
    pub kind: TypeKind,
}

pub struct PtrLevel {
    level: u8,
    mut_mask: u8,
}

pub enum TypeKind {
    Basic(BasicType),
    Custom(Box<Path>),
    ArraySlice(Box<ArraySlice>),
    ArrayStatic(Box<ArrayStatic>),
    Enum(EnumID),
    Union(UnionID),
    Struct(StructID),
    Poison,
}

pub struct ArraySlice {
    pub mutt: Mut,
    pub ty: Type,
}

pub struct ArrayStatic {
    pub size: ConstExpr,
    pub ty: Type,
}

pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

pub enum StmtKind {
    Break,
    Continue,
    Return(Option<Box<Expr>>),
    Defer(Box<Expr>),
    ForLoop(Box<For>),
    VarDecl(Box<VarDecl>),
    VarAssign(Box<VarAssign>),
    ExprSemi(Box<Expr>),
    ExprTail(Box<Expr>),
}

pub struct For {
    pub kind: ForKind,
    pub block: Box<Expr>,
}

#[rustfmt::skip]

pub enum ForKind {
    Loop,
    While { cond: Box<Expr> },
    ForLoop { var_decl: Box<VarDecl>, cond: Box<Expr>, var_assign: Box<VarAssign> },
}

pub struct VarDecl {
    pub mutt: Mut,
    pub name: Ident,
    pub ty: Option<Type>,
    pub expr: Option<Box<Expr>>,
}

pub struct VarAssign {
    pub op: AssignOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

pub struct ConstExpr(pub Box<Expr>);

#[rustfmt::skip]
pub enum ExprKind {
    Unit,
    LitNull,
    LitBool     { val: bool },
    LitInt      { val: u64, ty: Option<BasicType> },
    LitFloat    { val: f64, ty: Option<BasicType> },
    LitChar     { val: char },
    LitString   { id: InternID },
    If          { if_: Box<If> },
    Block       { stmts: Vec<Stmt> },
    Match       { on_expr: Box<Expr>, arms: Vec<MatchArm> },
    Field       { target: Box<Expr>, name: Ident },
    Index       { target: Box<Expr>, index: Box<Expr> },
    Cast        { target: Box<Expr>, ty: Box<Type> },
    Sizeof      { ty: Box<Type> },
    Item        { path: Box<Path> },
    ProcCall    { path: Box<Path>, input: Vec<Box<Expr>> },
    StructInit  { path: Box<Path>, input: Vec<FieldInit> },
    ArrayInit   { input: Vec<Box<Expr>> },
    ArrayRepeat { expr: Box<Expr>, size: ConstExpr },
    UnaryExpr   { op: UnOp, rhs: Box<Expr> },
    BinaryExpr  { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr> },
}

pub struct If {
    pub cond: Box<Expr>,
    pub block: Box<Expr>,
    pub else_: Option<Else>,
}

pub enum Else {
    If { else_if: Box<If> },
    Block { block: Box<Expr> },
}

pub struct MatchArm {
    pub pat: Box<Expr>,
    pub expr: Box<Expr>,
}

pub struct FieldInit {
    pub name: Ident,
    pub expr: Option<Box<Expr>>,
}

impl PtrLevel {
    const MAX_LEVEL: u8 = 8;

    pub fn new() -> Self {
        Self {
            level: 0,
            mut_mask: 0,
        }
    }

    pub fn level(&self) -> u8 {
        self.level
    }

    pub fn add_level(&mut self, mutt: Mut) -> Result<(), ()> {
        if self.level >= Self::MAX_LEVEL {
            return Err(());
        }
        let mut_bit = 1u8 << (self.level);
        if mutt == Mut::Mutable {
            self.mut_mask |= mut_bit;
        } else {
            self.mut_mask &= !mut_bit;
        }
        self.level += 1;
        Ok(())
    }
}

#[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
mod size_assert {
    use super::*;
    macro_rules! size_assert {
        ($size:expr, $ty:ty) => {
            const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
        };
    }

    //size_assert!(12, Ident);
    //size_assert!(16, Path);
    //size_assert!(24, Type);
    //size_assert!(16, Decl);
    //size_assert!(24, Stmt);
    //size_assert!(32, Expr);
}
