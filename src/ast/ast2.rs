use super::intern::InternID;
use super::parser::FileID;
use super::span::Span;
use crate::mem::arena_id::*;
use crate::mem::list_id::*;

pub struct Ast {
    pub arena: Arena,
    pub modules: Vec<Module>,
}

#[derive(Copy, Clone)]
pub struct Module {
    pub file_id: FileID,
    pub decls: List<Decl>,
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

#[derive(Clone, Copy)]
pub struct Ident {
    pub id: InternID,
    pub index: u32,
}

#[derive(Clone, Copy)]
pub struct Path {
    pub kind: PathKind,
    pub kind_index: u32,
    pub segments: List<Ident>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum PathKind {
    None,
    Super,
    Package,
}

#[derive(Clone, Copy)]
pub struct Type {
    pub ptr: PtrLevel,
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub struct PtrLevel {
    level: u8,
    mut_mask: u8,
}

#[rustfmt::skip] 
#[derive(Clone, Copy)]
pub enum TypeKind {
    Basic  { ty: BasicType },
    Custom { item: Id<ItemName> },
    Slice  { mutt: Mut, ty: Id<Type> },
    Array  { size: ConstExpr, ty: Id<Type> },
}

#[derive(Copy, Clone)]
pub enum Decl {
    Module(Id<ModuleDecl>),
    Import(Id<ImportDecl>),
    Global(Id<GlobalDecl>),
    Proc(Id<ProcDecl>),
    Enum(Id<EnumDecl>),
    Union(Id<UnionDecl>),
    Struct(Id<StructDecl>),
}

#[derive(Copy, Clone)]
pub struct ModuleDecl {
    pub vis: Vis,
    pub name: Ident,
}

#[derive(Copy, Clone)]
pub struct ImportDecl {
    pub path: Option<Id<Path>>,
    pub target: ImportTarget,
    pub span: Span,
}

#[derive(Copy, Clone)]
pub enum ImportTarget {
    GlobAll,
    Symbol(Ident),
    SymbolList(List<Ident>),
}

#[derive(Copy, Clone)]
pub struct GlobalDecl {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Option<Id<Type>>,
    pub value: ConstExpr,
}

#[derive(Copy, Clone)]
pub struct ProcDecl {
    pub vis: Vis,
    pub name: Ident,
    pub params: List<ProcParam>,
    pub is_variadic: bool,
    pub return_ty: Option<Id<Type>>,
    pub block: Option<Id<Block>>,
}

#[derive(Copy, Clone)]
pub struct ProcParam {
    pub mutt: Mut,
    pub name: Ident,
    pub ty: Id<Type>,
}

#[derive(Copy, Clone)]
pub struct EnumDecl {
    pub vis: Vis,
    pub name: Ident,
    pub basic_ty: Option<BasicType>,
    pub variants: List<EnumVariant>,
}

#[derive(Copy, Clone)]
pub struct EnumVariant {
    pub name: Ident,
    pub value: Option<ConstExpr>,
}

#[derive(Copy, Clone)]
pub struct UnionDecl {
    pub vis: Vis,
    pub name: Ident,
    pub members: List<UnionMember>,
}

#[derive(Copy, Clone)]
pub struct UnionMember {
    pub name: Ident,
    pub ty: Id<Type>,
}

#[derive(Copy, Clone)]
pub struct StructDecl {
    pub vis: Vis,
    pub name: Ident,
    pub fields: List<StructField>,
}

#[derive(Copy, Clone)]
pub struct StructField {
    pub vis: Vis,
    pub name: Ident,
    pub ty: Id<Type>,
}

#[derive(Clone, Copy)]
pub struct Stmt {
    span: Span,
    kind: StmtKind,
}

#[derive(Clone, Copy)]
pub enum StmtKind {
    Break,
    Continue,
    For(Id<For>),
    Defer(Id<Block>),
    Return(Option<Id<Expr>>),
    VarDecl(Id<VarDecl>),
    VarAssign(Id<VarAssign>),
    ExprSemi(Id<Expr>),
    ExprTail(Id<Expr>),
}

#[derive(Copy, Clone)]
pub struct For {
    pub var_decl: Option<Id<VarDecl>>,
    pub cond: Option<Id<Expr>>,
    pub var_assign: Option<Id<VarAssign>>,
    pub block: Id<Block>,
}

#[derive(Copy, Clone)]
pub struct VarDecl {
    pub mutt: Mut,
    pub name: Option<Ident>,
    pub ty: Option<Id<Type>>,
    pub expr: Option<Id<Expr>>,
}

#[derive(Copy, Clone)]
pub struct VarAssign {
    pub op: AssignOp,
    pub lhs: Id<Expr>,
    pub rhs: Id<Expr>,
}

#[derive(Clone, Copy)]
pub struct Expr {
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Copy)]
pub struct ConstExpr(pub Id<Expr>);

#[rustfmt::skip] 
#[derive(Clone, Copy)]
pub enum ExprKind {
    Unit,
    Discard,
    LitNull,
    LitBool     { val: bool },
    LitUint     { val: u64, ty: Option<BasicType> },
    LitFloat    { val: f64, ty: Option<BasicType> },
    LitChar     { val: char },
    LitString   { id: InternID },
    If          { if_: Id<If>, else_: Option<Else> },
    Block       { block: Id<Block> },
    Match       { expr: Id<Expr>, arms: List<MatchArm> },
    Field       { target: Id<Expr>, name: Ident },
    Index       { target: Id<Expr>, index: Id<Expr> },
    Cast        { target: Id<Expr>, ty: Id<Type> },
    Sizeof      { ty: Id<Type> },
    Item        { item: Id<ItemName> },
    ProcCall    { item: Id<ItemName>, input: List<Expr> },
    StructInit  { item: Id<ItemName>, input: List<FieldInit> },
    ArrayInit   { input: List<Expr> },
    ArrayRepeat { expr: Id<Expr>, size: ConstExpr, },
    UnaryExpr   { op: UnOp, rhs: Id<Expr> },
    BinaryExpr  { op: BinOp, lhs: Id<Expr>, rhs: Id<Expr> },
}

#[derive(Clone, Copy)]
pub struct If {
    pub cond: Id<Expr>,
    pub block: Id<Block>,
}

#[derive(Clone, Copy)]
pub enum Else {
    If(Id<If>),
    Block(Id<Block>),
}

#[derive(Clone, Copy)]
pub struct Block {
    pub stmts: List<Stmt>,
}

#[derive(Clone, Copy)]
pub struct MatchArm {
    pub pat: Id<Expr>,
    pub expr: Id<Expr>,
}

#[derive(Clone, Copy)]
pub struct ItemName {
    pub path: Option<Id<Path>>,
    pub name: Ident,
}

#[derive(Clone, Copy)]
pub struct FieldInit {
    pub name: Ident,
    pub expr: Id<Expr>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum BasicType {
    Unit,
    Bool,
    S8,
    S16,
    S32,
    S64,
    Ssize,
    U8,
    U16,
    U32,
    U64,
    Usize,
    F32,
    F64,
    Char,
    Rawptr,
}

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

    size_assert!(56, Ast);
    size_assert!(12, Module);

    size_assert!(1, Vis);
    size_assert!(1, Mut);
    size_assert!(8, Ident);
    size_assert!(16, Path);
    size_assert!(1, PathKind);

    size_assert!(24, Type);
    size_assert!(2, PtrLevel);
    size_assert!(12, TypeKind);

    size_assert!(8, Decl);
    size_assert!(12, ModuleDecl);
    size_assert!(28, ImportDecl);
    size_assert!(12, ImportTarget);
    size_assert!(24, GlobalDecl);
    size_assert!(36, ProcDecl);
    size_assert!(16, ProcParam);
    size_assert!(20, EnumDecl);
    size_assert!(16, EnumVariant);
    size_assert!(20, UnionDecl);
    size_assert!(12, UnionMember);
    size_assert!(20, StructDecl);
    size_assert!(16, StructField);

    size_assert!(16, Stmt);
    size_assert!(8, StmtKind);
    size_assert!(28, For);
    size_assert!(32, VarDecl);
    size_assert!(12, VarAssign);

    size_assert!(24, Expr);
    size_assert!(4, ConstExpr);
    size_assert!(16, ExprKind);
    size_assert!(8, If);
    size_assert!(8, Else);
    size_assert!(8, Block);
    size_assert!(8, MatchArm);
    size_assert!(16, ItemName);
    size_assert!(12, FieldInit);

    size_assert!(1, BasicType);
    size_assert!(1, UnOp);
    size_assert!(1, BinOp);
    size_assert!(1, AssignOp);
}
